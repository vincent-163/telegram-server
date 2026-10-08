//! TL RPC dispatch: turns serialized `functions::*` bodies into serialized
//! response bodies, with graceful `rpc_error` replies for unsupported calls.

use anyhow::{anyhow, bail, Result};
use grammers_tl_types as tl;
use grammers_tl_types::{Deserializable, Serializable};
use std::sync::Arc;

use crate::compat;
use crate::config::Config;
use crate::mtproto::{msg_container, rpc_error, rpc_result, MsgIdGen, SeqNoGen};
use crate::store::{now, MessageRow, Store, UserRow};

const MSG_CONTAINER: u32 = 0x73f1_f8dc;
const HTTP_WAIT: u32 = 0x929c_9539;

pub struct RpcContext {
    pub store: Store,
    pub cfg: Arc<Config>,
    pub auth_key_id: i64,
    pub user_id: i64,
    pub layer: i32,
}

/// One reply paired with the id of the client message it answers.
pub struct RpcReply {
    pub req_msg_id: i64,
    pub body: Vec<u8>,
}

impl RpcContext {
    fn me(&self) -> Result<UserRow> {
        self.store
            .get_user(self.user_id)?
            .ok_or_else(|| anyhow!("no authenticated user"))
    }
}

/// Dispatch one TL function body. Returns the raw serialized response (which
/// the caller wraps in `rpc_result`).
pub fn dispatch(ctx: &mut RpcContext, body: &[u8]) -> Result<Vec<u8>> {
    if body.len() < 4 {
        bail!("function body too short");
    }
    let ctor = u32::from_le_bytes(body[0..4].try_into().unwrap());

    // Container functions unwrap to their inner query.
    match ctor {
        0xda9b0d0d => {
            // invokeWithLayer#da9b0d0d layer:int query:!X = X
            if body.len() < 8 {
                bail!("invokeWithLayer too short");
            }
            ctx.layer = i32::from_le_bytes(body[4..8].try_into().unwrap());
            ctx.store.set_session_layer(ctx.auth_key_id, ctx.layer)?;
            return dispatch(ctx, &body[8..]);
        }
        0xbf9459b7 => {
            // invokeWithoutUpdates#bf9459b7 query:!X = X
            return dispatch(ctx, &body[4..]);
        }
        0xc1cd5ea9 => {
            // initConnection#c1cd5ea9 flags:# api_id:int device_model:string
            // system_version:string app_version:string system_lang_code:string
            // lang_pack:string lang_code:string proxy? params? query:!X = X
            let mut off = 4usize;
            let flags = read_i32(body, &mut off)?;
            let api_id = read_i32(body, &mut off)?;
            let device_model = read_string(body, &mut off)?;
            let system_version = read_string(body, &mut off)?;
            let app_version = read_string(body, &mut off)?;
            let _system_lang = read_string(body, &mut off)?;
            let _lang_pack = read_string(body, &mut off)?;
            let _lang_code = read_string(body, &mut off)?;
            if flags & 1 != 0 {
                skip_value(body, &mut off)?;
            }
            if flags & 2 != 0 {
                skip_value(body, &mut off)?;
            }
            let _ = (api_id, device_model, system_version, app_version);
            ctx.store.save_session(
                &format!("init-{}", ctx.auth_key_id),
                ctx.user_id,
                ctx.auth_key_id,
                "",
                "",
                0,
                0,
            )?;
            return dispatch(ctx, &body[off..]);
        }
        _ => {}
    }

    let result = dispatch_inner(ctx, body, ctor);
    // Third-party clients (Telethon, TDLib-based apps) announce older layers
    // and cannot parse the few objects whose constructor id changed since.
    if let Ok(mut reply) = result {
        restamp_response_for_layer(&mut reply, ctx.layer, ctor);
        return Ok(reply);
    }
    result
}

/// TL methods a client may call before it has an authorized session.
fn is_pre_auth_method(ctor: u32) -> bool {
    use grammers_tl_types::functions as f;
    use grammers_tl_types::Identifiable;
    matches!(
        ctor,
        f::Ping::CONSTRUCTOR_ID
            | f::PingDelayDisconnect::CONSTRUCTOR_ID
            | f::DestroySession::CONSTRUCTOR_ID
            | f::help::GetConfig::CONSTRUCTOR_ID
            | f::help::GetNearestDc::CONSTRUCTOR_ID
            | f::help::GetAppConfig::CONSTRUCTOR_ID
            | f::help::GetAppUpdate::CONSTRUCTOR_ID
            | f::help::GetSupport::CONSTRUCTOR_ID
            | f::help::GetTermsOfServiceUpdate::CONSTRUCTOR_ID
            | f::help::GetCountriesList::CONSTRUCTOR_ID
            | f::help::GetInviteText::CONSTRUCTOR_ID
            | f::langpack::GetLanguages::CONSTRUCTOR_ID
            | f::langpack::GetLanguage::CONSTRUCTOR_ID
            | f::langpack::GetDifference::CONSTRUCTOR_ID
            | f::auth::SendCode::CONSTRUCTOR_ID
            | f::auth::SignIn::CONSTRUCTOR_ID
            | f::auth::SignUp::CONSTRUCTOR_ID
            | f::auth::ResendCode::CONSTRUCTOR_ID
            | f::auth::CancelCode::CONSTRUCTOR_ID
            | f::auth::LogOut::CONSTRUCTOR_ID
            | f::auth::ResetAuthorizations::CONSTRUCTOR_ID
            | f::auth::ExportAuthorization::CONSTRUCTOR_ID
            | f::auth::ImportAuthorization::CONSTRUCTOR_ID
            | f::auth::CheckPassword::CONSTRUCTOR_ID
            | f::auth::RequestPasswordRecovery::CONSTRUCTOR_ID
    )
}

fn dispatch_inner(ctx: &mut RpcContext, body: &[u8], ctor: u32) -> Result<Vec<u8>> {
    use tl::Serializable as _;

    // Every arm below deserializes the *arguments*, i.e. the body without its
    // leading constructor id. Keep the two in sync rather than silently
    // parsing the wrong bytes.
    debug_assert_eq!(
        u32::from_le_bytes(body[0..4].try_into().unwrap()),
        ctor,
        "dispatch_inner called with a body that does not start with ctor"
    );
    let args = &body[4..];
    let store = &ctx.store;

    // Methods that must work before (or without) a signed-in session. Anything
    // else is rejected the way the real servers do, so clients trigger their
    // login flow instead of receiving empty result lists.
    if ctx.user_id == 0 && !is_pre_auth_method(ctor) {
        return bail_rpc(401, "AUTH_KEY_UNREGISTERED");
    }

    match ctor {
        // ---------------------------------------------------------------- ping
        0x7abe77ec => {
            let f = tl::functions::Ping::deserialize(&mut tl::Cursor::from_slice(args))?;
            let pong = tl::types::Pong {
                msg_id: 0,
                ping_id: f.ping_id,
            };
            return Ok(tl::enums::Pong::Pong(pong).to_bytes());
        }

        // ------------------------------------------------------------- help
        0xc4f9186b => {
            let _ = tl::functions::help::GetConfig::deserialize(&mut tl::Cursor::from_slice(args))?;
            return Ok(build_config(ctx)?.to_bytes());
        }
        0x1fb33026 => {
            let _ =
                tl::functions::help::GetNearestDc::deserialize(&mut tl::Cursor::from_slice(args))?;
            let nd = tl::types::NearestDc {
                country: "XX".into(),
                this_dc: ctx.cfg.dc_id,
                nearest_dc: ctx.cfg.dc_id,
            };
            return Ok(tl::enums::NearestDc::Dc(nd).to_bytes());
        }

        // ------------------------------------------------------------- auth
        0xa677244f => {
            let f = tl::functions::auth::SendCode::deserialize(&mut tl::Cursor::from_slice(args))?;
            return handle_send_code(ctx, &f);
        }
        0x8d52a951 => {
            let f = tl::functions::auth::SignIn::deserialize(&mut tl::Cursor::from_slice(args))?;
            return handle_sign_in(ctx, &f);
        }
        0xaac7b717 => {
            // Self-registration is disabled: accounts are created by an admin.
            return bail_rpc(403, "SIGNUP_DISABLED");
        }

        // ------------------------------------------------------------ users
        0xd91a548 => {
            let f = tl::functions::users::GetUsers::deserialize(&mut tl::Cursor::from_slice(args))?;
            let mut out = Vec::new();
            for input in &f.id {
                if let Some(u) = resolve_input_user(store, input, ctx.user_id)? {
                    out.push(build_user(ctx, &u)?);
                }
            }
            return Ok(out.to_bytes());
        }
        0xb60f5918 => {
            let f =
                tl::functions::users::GetFullUser::deserialize(&mut tl::Cursor::from_slice(args))?;
            return handle_get_full_user(ctx, &f);
        }

        // ---------------------------------------------------------- account
        0x78515775 => {
            let f = tl::functions::account::UpdateProfile::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            store.update_profile(
                ctx.user_id,
                f.first_name.as_deref(),
                f.last_name.as_deref(),
                f.about.as_deref(),
            )?;
            let u = ctx.me()?;
            return Ok(build_user(ctx, &u)?.to_bytes());
        }

        // ---------------------------------------------------------- messages
        0xa0f4cb4f => {
            let f = tl::functions::messages::GetDialogs::deserialize(&mut tl::Cursor::from_slice(
                args,
            ))?;
            return handle_get_dialogs(ctx, &f);
        }
        0x4423e6c5 => {
            let f = tl::functions::messages::GetHistory::deserialize(&mut tl::Cursor::from_slice(
                args,
            ))?;
            return handle_get_history(ctx, &f);
        }
        0xfef48f62 | 0x545cd15a => {
            // `messages.sendMessage`: `#fef48f62` at the current layer,
            // `#545cd15a` from layer <= 224 clients such as Telethon.
            return handle_send_message(ctx, &parse_send_message(args)?);
        }
        0xb106e66c | 0x51e842e1 => {
            // `messages.editMessage`: `#b106e66c` at the current layer,
            // `#51e842e1` from layer <= 224 clients such as Telethon.
            return handle_edit_message(ctx, &parse_edit_message(args)?);
        }
        0xe58e95d2 => {
            let f = tl::functions::messages::DeleteMessages::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_delete_messages(ctx, &f);
        }
        0xe306d3a => {
            let f = tl::functions::messages::ReadHistory::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_read_history(ctx, &f);
        }
        0x63c66506 => {
            // messages.getMessages
            let f = tl::functions::messages::GetMessages::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_get_messages(ctx, &f);
        }
        0xaeb00b34 => {
            // messages.getFullChat
            let f = tl::functions::messages::GetFullChat::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_get_full_chat_real(ctx, &f);
        }
        // ------------------------------------------------- real handlers
        0xe470bcfd => {
            let f = tl::functions::messages::GetPeerDialogs::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_get_peer_dialogs(ctx, &f);
        }
        0xd6b94df2 => return handle_get_pinned_dialogs(ctx),
        0x29ee847a => {
            let f =
                tl::functions::messages::Search::deserialize(&mut tl::Cursor::from_slice(args))?;
            return handle_search(ctx, &f);
        }
        0xefd9a6a2 => {
            let f = tl::functions::messages::GetPeerSettings::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_get_peer_settings(ctx, &f);
        }
        0x725afbbc => {
            let f = tl::functions::contacts::ResolveUsername::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_resolve_username(ctx, &f);
        }
        0x11f812d8 => {
            // Legacy `contacts.search#11f812d8 q:string limit:int`, still sent
            // by clients pinned to older layers (e.g. Telethon).
            let mut cur = tl::Cursor::from_slice(args);
            let q = String::deserialize(&mut cur)?;
            let limit = i32::deserialize(&mut cur)?;
            return handle_contacts_search_query(ctx, &q, limit);
        }
        0x05f58d0f => {
            // `contacts.search#05f58d0f flags:# q:string limit:int` (current
            // layer, with `broadcasts`/`bots` flags).
            let f =
                tl::functions::contacts::Search::deserialize(&mut tl::Cursor::from_slice(args))?;
            return handle_contacts_search_query(ctx, &f.q, f.limit);
        }
        0xe40ca104 => {
            let f = tl::functions::messages::GetCommonChats::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_get_common_chats(ctx, &f);
        }
        0x49e9528f => {
            let f =
                tl::functions::messages::GetChats::deserialize(&mut tl::Cursor::from_slice(args))?;
            return handle_get_chats(ctx, &f);
        }
        0xa7f6bbb => {
            let f = tl::functions::channels::GetChannels::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_get_channels(ctx, &f);
        }
        0x77ced9d0 => {
            let f = tl::functions::channels::GetParticipants::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_get_participants(ctx, &f);
        }
        0x91cd32a8 => {
            let f = tl::functions::photos::GetUserPhotos::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_get_user_photos(ctx, &f);
        }
        0x12b3ad31 => {
            let f = tl::functions::account::GetNotifySettings::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_get_notify_settings(ctx, &f);
        }
        0x84be5b93 => {
            let f = tl::functions::account::UpdateNotifySettings::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_update_notify_settings(ctx, &f);
        }
        0xe320c158 => return handle_get_authorizations(ctx),
        0xbe5335be => {
            let f = tl::functions::upload::GetFile::deserialize(&mut tl::Cursor::from_slice(args))?;
            return handle_upload_get_file(ctx, &f);
        }
        0x61e3f854 => return handle_get_app_config(ctx),
        0x9cdf08cd => return handle_get_support(ctx),
        0xdf77f3bc => {
            let f = tl::functions::account::ResetAuthorization::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return handle_reset_authorization(ctx, f.hash);
        }
        0x9fab0d1a => {
            let keep = ctx
                .store
                .sessions_for_user(ctx.user_id)?
                .first()
                .map(|(id, _, _, _, _, _)| id.clone());
            ctx.store.drop_all_sessions(ctx.user_id, keep.as_deref())?;
            return Ok(true.to_bytes());
        }
        0x92ceddd4 => {
            // messages.createChat
            let f = tl::functions::messages::CreateChat::deserialize(&mut tl::Cursor::from_slice(
                args,
            ))?;
            let chat = store.create_chat(&f.title, ctx.user_id)?;
            for input in &f.users {
                if let Some(u) = resolve_input_user(store, input, ctx.user_id)? {
                    store.add_chat_member(chat.id, u.id, "member")?;
                }
            }
            let mut updates = Vec::new();
            updates.push(tl::enums::Update::ChatParticipants(
                tl::types::UpdateChatParticipants {
                    participants: tl::enums::ChatParticipants::Participants(
                        tl::types::ChatParticipants {
                            chat_id: chat.id,
                            participants: Vec::new(),
                            version: 1,
                        },
                    ),
                },
            ));
            let resp = tl::types::Updates {
                updates,
                users: Vec::new(),
                chats: vec![build_chat(&chat, 1)],
                date: now() as i32,
                seq: 1,
            };
            return Ok(tl::enums::Updates::Updates(resp).to_bytes());
        }
        0x58943ee2 => {
            // messages.setTyping -> Bool true
            return Ok(true.to_bytes());
        }

        // ----------------------------------------------------------- updates
        0xedd4882a => {
            let _ =
                tl::functions::updates::GetState::deserialize(&mut tl::Cursor::from_slice(args))?;
            return handle_get_state(ctx);
        }
        0x19c2f763 => {
            let _f = tl::functions::updates::GetDifference::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            let (_, _, seq, date) = store.state_for(ctx.user_id)?;
            let empty = tl::types::updates::DifferenceEmpty {
                date: date as i32,
                seq,
            };
            return Ok(tl::enums::updates::Difference::Empty(empty).to_bytes());
        }

        // ------------------------------------------------------------- upload
        0xb304a621 => {
            let f = tl::functions::upload::SaveFilePart::deserialize(&mut tl::Cursor::from_slice(
                args,
            ))?;
            store.put_file_part(f.file_id, f.file_part, &f.bytes)?;
            return Ok(true.to_bytes());
        }
        0xde7b673d => {
            let f = tl::functions::upload::SaveBigFilePart::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            store.put_file_part(f.file_id, f.file_part, &f.bytes)?;
            return Ok(true.to_bytes());
        }

        // ---------------------------------------------------------- contacts
        0x5dd69e12 => {
            let _ = tl::functions::contacts::GetContacts::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            let empty = tl::types::contacts::Contacts {
                contacts: Vec::new(),
                saved_count: 0,
                users: Vec::new(),
            };
            return Ok(tl::enums::contacts::Contacts::Contacts(empty).to_bytes());
        }
        0x2c800be5 => {
            let _f = tl::functions::contacts::ImportContacts::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            let imp = tl::types::contacts::ImportedContacts {
                imported: Vec::new(),
                popular_invites: Vec::new(),
                retry_contacts: Vec::new(),
                users: Vec::new(),
            };
            return Ok(tl::enums::contacts::ImportedContacts::Contacts(imp).to_bytes());
        }
        0x879537f1 => {
            // contacts.resetSaved
            return Ok(tl::enums::Updates::TooLong.to_bytes());
        }

        // ------------------------------------------------------------ langpack
        0x42c6978f => {
            let _f = tl::functions::langpack::GetLanguages::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return Ok(Vec::<tl::enums::LangPackLanguage>::new().to_bytes());
        }
        0xcd984aa5 => {
            let f = tl::functions::langpack::GetDifference::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            let diff = tl::types::LangPackDifference {
                lang_code: f.lang_code,
                from_version: f.from_version,
                version: f.from_version,
                strings: Vec::new(),
            };
            return Ok(tl::enums::LangPackDifference::Difference(diff).to_bytes());
        }
        0x6a596502 => {
            let _f = tl::functions::langpack::GetLanguage::deserialize(
                &mut tl::Cursor::from_slice(args),
            )?;
            return bail_rpc(400, "LANG_PACK_LANGUAGE_INVALID");
        }

        // -------------------------------------------------- no-op / benign
        0x62d6b459 => {
            // msgs_ack
            return Ok(Vec::new());
        }
        _ => {
            if let Some(body) = compat::default_response(ctor) {
                return Ok(body);
            }
            let name = tl::name_for_id(ctor);
            return bail_rpc(400, &format!("RPC_ERROR: unsupported method {}", name));
        }
    }
}

// ============================================================ new handlers

/// Resolve an `InputUser` to a concrete `UserRow`, including `inputUserSelf`.
fn resolve_user_any(ctx: &RpcContext, i: &tl::enums::InputUser) -> Result<Option<UserRow>> {
    use tl::enums::InputUser as U;
    match i {
        U::UserSelf => ctx.store.get_user(ctx.user_id),
        other => resolve_input_user(&ctx.store, other, ctx.user_id),
    }
}

/// Build a `messages.PeerDialogs` response from the user's dialog list,
/// optionally restricted to a set of peers.
fn build_peer_dialogs(ctx: &RpcContext, only: Option<&[tl::enums::Peer]>) -> Result<Vec<u8>> {
    let rows = ctx.store.dialogs_for(ctx.user_id)?;
    let mut dialogs = Vec::new();
    let mut messages = Vec::new();
    let mut users = Vec::new();
    let mut chats = Vec::new();
    for row in &rows {
        let peer = dialog_peer(&row.dialog_type, row.dialog_id);
        if let Some(want) = only {
            if !want.contains(&peer) {
                continue;
            }
        }
        let top = if row.top_message > 0 {
            find_message(ctx, &row.dialog_type, row.dialog_id, row.top_message)?
        } else {
            None
        };
        if let Some(m) = top {
            messages.push(build_message(&m, self_peer(&m, ctx.user_id)));
        }
        let dlg = tl::types::Dialog {
            pinned: row.pinned,
            unread_mark: false,
            view_forum_as_messages: false,
            peer: peer.clone(),
            top_message: row.top_message,
            read_inbox_max_id: row.read_max_id,
            read_outbox_max_id: 0,
            unread_count: row.unread_count,
            unread_mentions_count: 0,
            unread_reactions_count: 0,
            unread_poll_votes_count: 0,
            notify_settings: default_notify(),
            pts: None,
            draft: None,
            folder_id: None,
            ttl_period: None,
        };
        dialogs.push(tl::enums::Dialog::Dialog(dlg));
        collect_refs(ctx, &peer, &mut users, &mut chats)?;
    }
    let (pts, qts, seq, date) = ctx.store.state_for(ctx.user_id)?;
    let resp = tl::types::messages::PeerDialogs {
        dialogs,
        messages,
        chats,
        users,
        state: tl::enums::updates::State::State(tl::types::updates::State {
            pts,
            qts,
            date: date as i32,
            seq,
            unread_count: 0,
        }),
    };
    Ok(tl::enums::messages::PeerDialogs::Dialogs(resp).to_bytes())
}

fn handle_get_peer_dialogs(
    ctx: &RpcContext,
    f: &tl::functions::messages::GetPeerDialogs,
) -> Result<Vec<u8>> {
    let mut want = Vec::new();
    for p in &f.peers {
        if let tl::enums::InputDialogPeer::Peer(ip) = p {
            if let Ok((kind, id)) = resolve_peer(&ip.peer) {
                if kind == "self" {
                    want.push(tl::enums::Peer::User(tl::types::PeerUser {
                        user_id: ctx.user_id,
                    }));
                } else {
                    want.push(dialog_peer(&kind, id));
                }
            }
        }
    }
    build_peer_dialogs(ctx, Some(&want))
}

/// `messages.getMessages` — fetch a specific set of message ids.
fn handle_get_messages(
    ctx: &RpcContext,
    f: &tl::functions::messages::GetMessages,
) -> Result<Vec<u8>> {
    let mut messages = Vec::new();
    let mut users = Vec::new();
    let mut chats = Vec::new();
    let dialogs = ctx.store.dialogs_for(ctx.user_id)?;
    let mut wanted: Vec<i32> = Vec::new();
    for i in &f.id {
        if let tl::enums::InputMessage::Id(x) = i {
            wanted.push(x.id);
        }
    }
    for row in &dialogs {
        for id in &wanted {
            if let Some(m) = find_message(ctx, &row.dialog_type, row.dialog_id, *id)? {
                let peer = peer_of_dialog(&row.dialog_type, row.dialog_id, ctx.user_id);
                messages.push(build_message(&m, peer.clone()));
                collect_refs(ctx, &peer, &mut users, &mut chats)?;
            }
        }
    }
    let resp = tl::types::messages::Messages {
        messages,
        topics: Vec::new(),
        chats,
        users,
    };
    Ok(tl::enums::messages::Messages::Messages(resp).to_bytes())
}

/// `messages.search` — substring search over the user's message history.
fn handle_search(ctx: &RpcContext, f: &tl::functions::messages::Search) -> Result<Vec<u8>> {
    let (kind, id) = resolve_peer(&f.peer)?;
    let (kind_opt, id_opt) = if kind == "self" {
        (None, None)
    } else {
        (Some(kind.as_str()), Some(id))
    };
    let from = match &f.from_id {
        Some(p) => match resolve_peer(p) {
            Ok((k, _)) if k == "self" => Some(ctx.user_id),
            Ok((_, v)) => Some(v),
            Err(_) => None,
        },
        None => None,
    };
    let rows = ctx
        .store
        .search_messages(kind_opt, id_opt, from, &f.q, f.limit, f.offset_id)?;
    let mut messages = Vec::new();
    let mut users = Vec::new();
    let mut chats = Vec::new();
    for r in &rows {
        let peer = peer_of_dialog(&r.dialog_type, r.dialog_id, ctx.user_id);
        messages.push(build_message(r, peer.clone()));
        collect_refs(ctx, &peer, &mut users, &mut chats)?;
    }
    let resp = tl::types::messages::Messages {
        messages,
        topics: Vec::new(),
        chats,
        users,
    };
    Ok(tl::enums::messages::Messages::Messages(resp).to_bytes())
}

/// `messages.getPeerSettings` — default settings for any peer.
fn handle_get_peer_settings(
    ctx: &RpcContext,
    f: &tl::functions::messages::GetPeerSettings,
) -> Result<Vec<u8>> {
    let (kind, id) = resolve_peer(&f.peer)?;
    let mut users = Vec::new();
    let mut chats = Vec::new();
    if kind != "self" {
        let peer = dialog_peer(&kind, id);
        collect_refs(ctx, &peer, &mut users, &mut chats)?;
    }
    let settings = tl::types::PeerSettings {
        report_spam: false,
        add_contact: false,
        block_contact: false,
        share_contact: true,
        need_contacts_exception: false,
        report_geo: false,
        autoarchived: false,
        invite_members: false,
        request_chat_broadcast: false,
        business_bot_paused: false,
        business_bot_can_reply: false,
        geo_distance: None,
        request_chat_title: None,
        request_chat_date: None,
        business_bot_id: None,
        business_bot_manage_url: None,
        charge_paid_message_stars: None,
        registration_month: None,
        phone_country: None,
        name_change_date: None,
        photo_change_date: None,
    };
    let resp = tl::types::messages::PeerSettings {
        settings: tl::enums::PeerSettings::Settings(settings),
        chats,
        users,
    };
    Ok(tl::enums::messages::PeerSettings::Settings(resp).to_bytes())
}

/// `users.getFullUser` — the profile card the client opens for a user.
fn handle_get_full_user(
    ctx: &RpcContext,
    f: &tl::functions::users::GetFullUser,
) -> Result<Vec<u8>> {
    let target = match resolve_user_any(ctx, &f.id)? {
        Some(u) => u,
        None => return bail_rpc(400, "USER_ID_INVALID"),
    };
    let mut chats = Vec::new();
    let users = vec![build_user(ctx, &target)?];
    let common = ctx.store.chat_members_shared(ctx.user_id, target.id)?;
    for c in &common {
        let members = ctx.store.chat_members(c.id)?.len() as i32;
        chats.push(build_chat(c, members));
    }
    let notify = ctx
        .store
        .get_notify_settings(ctx.user_id, &format!("user:{}", target.id))?
        .and_then(|j| parse_notify_settings(&j))
        .unwrap_or_else(default_notify);
    let settings = tl::types::PeerSettings {
        report_spam: false,
        add_contact: false,
        block_contact: false,
        share_contact: true,
        need_contacts_exception: false,
        report_geo: false,
        autoarchived: false,
        invite_members: false,
        request_chat_broadcast: false,
        business_bot_paused: false,
        business_bot_can_reply: false,
        geo_distance: None,
        request_chat_title: None,
        request_chat_date: None,
        business_bot_id: None,
        business_bot_manage_url: None,
        charge_paid_message_stars: None,
        registration_month: None,
        phone_country: None,
        name_change_date: None,
        photo_change_date: None,
    };
    let full = tl::types::UserFull {
        blocked: false,
        phone_calls_available: false,
        phone_calls_private: true,
        can_pin_message: false,
        has_scheduled: false,
        video_calls_available: false,
        voice_messages_forbidden: false,
        translations_disabled: true,
        stories_pinned_available: false,
        blocked_my_stories_from: false,
        wallpaper_overridden: false,
        contact_require_premium: false,
        read_dates_private: true,
        sponsored_enabled: false,
        can_view_revenue: false,
        bot_can_manage_emoji_status: false,
        display_gifts_button: false,
        noforwards_my_enabled: false,
        noforwards_peer_enabled: false,
        unofficial_security_risk: false,
        id: target.id,
        about: Some(target.about.clone()).filter(|s| !s.is_empty()),
        settings: tl::enums::PeerSettings::Settings(settings),
        personal_photo: None,
        profile_photo: Some(tl::enums::Photo::Empty(tl::types::PhotoEmpty { id: 0 })),
        fallback_photo: None,
        notify_settings: notify,
        bot_info: if target.is_bot {
            Some(tl::enums::BotInfo::Info(tl::types::BotInfo {
                has_preview_medias: false,
                user_id: Some(target.id),
                description: Some(target.about.clone()),
                description_photo: None,
                description_document: None,
                commands: Some(Vec::new()),
                menu_button: None,
                privacy_policy_url: None,
                app_settings: None,
                verifier_settings: None,
            }))
        } else {
            None
        },
        pinned_msg_id: None,
        common_chats_count: common.len() as i32,
        folder_id: None,
        ttl_period: None,
        theme: None,
        private_forward_name: None,
        bot_group_admin_rights: None,
        bot_broadcast_admin_rights: None,
        wallpaper: None,
        stories: None,
        business_work_hours: None,
        business_location: None,
        business_greeting_message: None,
        business_away_message: None,
        business_intro: None,
        birthday: None,
        personal_channel_id: None,
        personal_channel_message: None,
        stargifts_count: None,
        starref_program: None,
        bot_verification: None,
        send_paid_messages_stars: None,
        disallowed_gifts: None,
        stars_rating: None,
        stars_my_pending_rating: None,
        stars_my_pending_rating_date: None,
        main_tab: None,
        saved_music: None,
        note: None,
        bot_manager_id: None,
    };
    let resp = tl::types::users::UserFull {
        full_user: tl::enums::UserFull::Full(full),
        chats,
        users,
    };
    Ok(tl::enums::users::UserFull::Full(resp).to_bytes())
}

/// `contacts.resolveUsername` — username -> peer lookup.
fn handle_resolve_username(
    ctx: &RpcContext,
    f: &tl::functions::contacts::ResolveUsername,
) -> Result<Vec<u8>> {
    let name = f.username.trim_start_matches('@').to_string();
    let mut users = Vec::new();
    let mut chats = Vec::new();
    let peer = if let Some(u) = ctx.store.get_user_by_username(&name)? {
        users.push(build_user(ctx, &u)?);
        tl::enums::Peer::User(tl::types::PeerUser { user_id: u.id })
    } else {
        bail_rpc(400, "USERNAME_NOT_OCCUPIED")?
    };
    let resp = tl::types::contacts::ResolvedPeer {
        peer,
        chats: std::mem::take(&mut chats),
        users,
    };
    Ok(tl::enums::contacts::ResolvedPeer::Peer(resp).to_bytes())
}

/// `contacts.search` — search users and chats by name or username.
fn handle_contacts_search_query(ctx: &RpcContext, query: &str, limit: i32) -> Result<Vec<u8>> {
    let q = query.to_lowercase();
    let limit = limit.clamp(1, 100) as usize;
    let mut users = Vec::new();
    let mut results = Vec::new();
    if !q.is_empty() {
        for u in ctx.store.all_users()? {
            if results.len() >= limit {
                break;
            }
            let hay = format!("{} {} {}", u.first_name, u.last_name, u.username).to_lowercase();
            if hay.contains(&q) {
                results.push(tl::enums::Peer::User(tl::types::PeerUser { user_id: u.id }));
                users.push(build_user(ctx, &u)?);
            }
        }
    }
    let resp = tl::types::contacts::Found {
        my_results: Vec::new(),
        results,
        chats: Vec::new(),
        users,
    };
    Ok(tl::enums::contacts::Found::Found(resp).to_bytes())
}

/// `messages.getCommonChats` — chats both users belong to.
fn handle_get_common_chats(
    ctx: &RpcContext,
    f: &tl::functions::messages::GetCommonChats,
) -> Result<Vec<u8>> {
    let other = match resolve_user_any(ctx, &f.user_id)? {
        Some(u) => u,
        None => return bail_rpc(400, "USER_ID_INVALID"),
    };
    let common = ctx.store.chat_members_shared(ctx.user_id, other.id)?;
    let mut chats = Vec::new();
    for c in &common {
        let members = ctx.store.chat_members(c.id)?.len() as i32;
        chats.push(build_chat(c, members));
    }
    Ok(tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats }).to_bytes())
}

/// `messages.getChats` — resolve a list of chat ids.
fn handle_get_chats(ctx: &RpcContext, f: &tl::functions::messages::GetChats) -> Result<Vec<u8>> {
    let mut chats = Vec::new();
    for id in &f.id {
        if let Some(c) = ctx.store.get_chat(*id)? {
            let members = ctx.store.chat_members(c.id)?.len() as i32;
            chats.push(build_chat(&c, members));
        }
    }
    Ok(tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats }).to_bytes())
}

/// `messages.getFullChat` — basic group profile.
fn handle_get_full_chat_real(
    ctx: &RpcContext,
    f: &tl::functions::messages::GetFullChat,
) -> Result<Vec<u8>> {
    let chat = match ctx.store.get_chat(f.chat_id)? {
        Some(c) => c,
        None => return bail_rpc(400, "CHAT_ID_INVALID"),
    };
    let members = ctx.store.chat_members(chat.id)?;
    let mut participants = Vec::new();
    let mut users = Vec::new();
    for (uid, role) in &members {
        let admin = role == "creator" || role == "admin";
        let creator = role == "creator";
        if let Some(u) = ctx.store.get_user(*uid)? {
            users.push(build_user(ctx, &u)?);
        }
        participants.push(if creator {
            tl::enums::ChatParticipant::Creator(tl::types::ChatParticipantCreator {
                user_id: *uid,
                rank: None,
            })
        } else if admin {
            tl::enums::ChatParticipant::Admin(tl::types::ChatParticipantAdmin {
                user_id: *uid,
                inviter_id: chat.creator_id,
                date: chat.created_at as i32,
                rank: None,
            })
        } else {
            tl::enums::ChatParticipant::Participant(tl::types::ChatParticipant {
                user_id: *uid,
                inviter_id: chat.creator_id,
                date: chat.created_at as i32,
                rank: None,
            })
        });
    }
    let full = tl::types::ChatFull {
        can_set_username: false,
        has_scheduled: false,
        translations_disabled: true,
        id: chat.id,
        about: String::new(),
        participants: tl::enums::ChatParticipants::Participants(tl::types::ChatParticipants {
            chat_id: chat.id,
            participants,
            version: 1,
        }),
        chat_photo: Some(tl::enums::Photo::Empty(tl::types::PhotoEmpty { id: 0 })),
        notify_settings: default_notify(),
        exported_invite: None,
        bot_info: None,
        pinned_msg_id: None,
        folder_id: None,
        call: None,
        ttl_period: None,
        groupcall_default_join_as: None,
        theme_emoticon: None,
        requests_pending: None,
        recent_requesters: None,
        available_reactions: None,
        reactions_limit: None,
    };
    let resp = tl::types::messages::ChatFull {
        full_chat: tl::enums::ChatFull::Full(full),
        chats: vec![build_chat(&chat, members.len() as i32)],
        users,
    };
    Ok(tl::enums::messages::ChatFull::Full(resp).to_bytes())
}

/// `messages.getPinnedDialogs` — the user's pinned dialogs.
fn handle_get_pinned_dialogs(ctx: &RpcContext) -> Result<Vec<u8>> {
    let rows = ctx.store.dialogs_for(ctx.user_id)?;
    let pinned: Vec<tl::enums::Peer> = rows
        .iter()
        .filter(|r| r.pinned)
        .map(|r| dialog_peer(&r.dialog_type, r.dialog_id))
        .collect();
    if pinned.is_empty() {
        build_peer_dialogs(ctx, Some(&[]))
    } else {
        build_peer_dialogs(ctx, Some(&pinned))
    }
}

/// `channels.getChannels` — channel peers are not implemented, so this
/// returns an empty list rather than an error.
fn handle_get_channels(
    _ctx: &RpcContext,
    _f: &tl::functions::channels::GetChannels,
) -> Result<Vec<u8>> {
    Ok(
        tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats: Vec::new() })
            .to_bytes(),
    )
}

/// `photos.getUserPhotos` — no photo albums are stored, so return an
/// empty, well-formed list.
fn handle_get_user_photos(
    _ctx: &RpcContext,
    _f: &tl::functions::photos::GetUserPhotos,
) -> Result<Vec<u8>> {
    let resp = tl::types::photos::Photos {
        photos: Vec::new(),
        users: Vec::new(),
    };
    Ok(tl::enums::photos::Photos::Photos(resp).to_bytes())
}

/// `account.getNotifySettings` — read back per-peer notification settings.
fn handle_get_notify_settings(
    ctx: &RpcContext,
    f: &tl::functions::account::GetNotifySettings,
) -> Result<Vec<u8>> {
    let peer = notify_peer_key(&f.peer);
    let stored = ctx.store.get_notify_settings(ctx.user_id, &peer)?;
    match stored.and_then(|j| parse_notify_settings(&j)) {
        Some(s) => Ok(s.to_bytes()),
        None => Ok(default_notify().to_bytes()),
    }
}

/// `account.updateNotifySettings` — persist per-peer notification settings.
fn handle_update_notify_settings(
    ctx: &RpcContext,
    f: &tl::functions::account::UpdateNotifySettings,
) -> Result<Vec<u8>> {
    let peer = notify_peer_key(&f.peer);
    let json = serialize_notify_settings(&f.settings);
    ctx.store.set_notify_settings(ctx.user_id, &peer, &json)?;
    Ok(true.to_bytes())
}

/// `account.getAuthorizations` — the active login sessions of this user.
fn handle_get_authorizations(ctx: &RpcContext) -> Result<Vec<u8>> {
    let sessions = ctx.store.sessions_for_user(ctx.user_id)?;
    let current = ctx.store.device_info(ctx.auth_key_id).ok();
    let mut list = Vec::new();
    for (id, _key_id, created, model, platform, api_id) in sessions {
        let is_current = current
            .as_ref()
            .map(|(m, p, a)| *m == model && *p == platform && *a == api_id)
            .unwrap_or(false);
        let hash = hash_i64(&id);
        list.push(tl::enums::Authorization::Authorization(
            tl::types::Authorization {
                current: is_current,
                official_app: false,
                password_pending: false,
                encrypted_requests_disabled: false,
                call_requests_disabled: true,
                unconfirmed: false,
                hash,
                device_model: model,
                platform,
                system_version: String::new(),
                api_id,
                app_name: "Telegram".into(),
                app_version: String::new(),
                date_created: created as i32,
                date_active: created as i32,
                ip: "127.0.0.1".into(),
                country: String::new(),
                region: String::new(),
            },
        ));
    }
    let resp = tl::types::account::Authorizations {
        authorization_ttl_days: 180,
        authorizations: list,
    };
    Ok(tl::enums::account::Authorizations::Authorizations(resp).to_bytes())
}

/// `account.resetAuthorization` (0xdf77f3bc) and
/// `auth.resetAuthorizations` (0x9fab0d1a).
fn handle_reset_authorization(ctx: &RpcContext, hash: i64) -> Result<Vec<u8>> {
    let sessions = ctx.store.sessions_for_user(ctx.user_id)?;
    let target = sessions
        .iter()
        .find(|(id, _, _, _, _, _)| hash_i64(id) == hash)
        .map(|(id, _, _, _, _, _)| id.clone());
    match target {
        Some(id) => {
            ctx.store.drop_session(ctx.user_id, &id)?;
            Ok(true.to_bytes())
        }
        None => bail_rpc(400, "AUTH_HASH_INVALID"),
    }
}

/// `upload.getFile` — serve a previously uploaded blob, in chunks.
fn handle_upload_get_file(ctx: &RpcContext, f: &tl::functions::upload::GetFile) -> Result<Vec<u8>> {
    let blob_id = file_location_key(&f.location);
    let data = match blob_id
        .as_deref()
        .and_then(|k| ctx.store.get_blob(k).ok().flatten())
    {
        Some(d) => d,
        None => {
            return Ok(tl::enums::upload::File::File(tl::types::upload::File {
                r#type: tl::enums::storage::FileType::FileUnknown,
                mtime: now() as i32,
                bytes: Vec::new(),
            })
            .to_bytes());
        }
    };
    let offset = f.offset.max(0) as usize;
    let limit = f.limit.clamp(1, 1 << 20) as usize;
    let end = (offset + limit).min(data.len());
    let bytes = if offset < data.len() {
        data[offset..end].to_vec()
    } else {
        Vec::new()
    };
    Ok(tl::enums::upload::File::File(tl::types::upload::File {
        r#type: guess_file_type(&data),
        mtime: now() as i32,
        bytes,
    })
    .to_bytes())
}

/// `help.getAppConfig` — minimal, well-formed client configuration.
fn handle_get_app_config(ctx: &RpcContext) -> Result<Vec<u8>> {
    let values: Vec<tl::enums::JsonobjectValue> = Vec::new();
    let json = tl::enums::Jsonvalue::JsonObject(tl::types::JsonObject { value: values });
    let resp = tl::types::help::AppConfig {
        hash: 1,
        config: json,
    };
    let _ = ctx;
    Ok(tl::enums::help::AppConfig::Config(resp).to_bytes())
}

/// `help.getSupport` — the server operator as the support contact.
fn handle_get_support(ctx: &RpcContext) -> Result<Vec<u8>> {
    let admin = ctx
        .store
        .all_users()?
        .into_iter()
        .find(|u| u.admin)
        .or_else(|| ctx.store.get_user(ctx.user_id).ok().flatten())
        .ok_or_else(|| anyhow!("no support user"))?;
    let resp = tl::types::help::Support {
        phone_number: admin.phone.clone(),
        user: build_user(ctx, &admin)?,
    };
    Ok(tl::enums::help::Support::Support(resp).to_bytes())
}

/// `channels.getParticipants` — no channels exist, so report empty.
fn handle_get_participants(
    _ctx: &RpcContext,
    _f: &tl::functions::channels::GetParticipants,
) -> Result<Vec<u8>> {
    let resp = tl::types::channels::ChannelParticipants {
        count: 0,
        participants: Vec::new(),
        chats: Vec::new(),
        users: Vec::new(),
    };
    Ok(tl::enums::channels::ChannelParticipants::Participants(resp).to_bytes())
}

// ------------------------------------------------------------ small helpers

fn hash_i64(s: &str) -> i64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish() as i64
}

fn notify_peer_key(p: &tl::enums::InputNotifyPeer) -> String {
    use tl::enums::InputNotifyPeer as N;
    match p {
        N::Peer(x) => match resolve_peer(&x.peer) {
            Ok((k, _)) if k == "self" => "user:0".into(),
            Ok((k, id)) => format!("{k}:{id}"),
            Err(_) => "unknown".into(),
        },
        N::InputNotifyUsers => "all:users".into(),
        N::InputNotifyChats => "all:chats".into(),
        N::InputNotifyBroadcasts => "all:broadcasts".into(),
        N::InputNotifyForumTopic(_) => "all:forum".into(),
    }
}

fn serialize_notify_settings(s: &tl::enums::InputPeerNotifySettings) -> String {
    let tl::enums::InputPeerNotifySettings::Settings(v) = s;
    let esc = |x: &Option<String>| {
        x.clone()
            .unwrap_or_default()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    };
    format!(
        "{{\"show_previews\":{},\"silent\":{},\"mute_until\":{},\"sound\":\"{}\",\"stories_muted\":{}}}",
        v.show_previews.unwrap_or(true),
        v.silent.unwrap_or(false),
        v.mute_until.unwrap_or(0),
        esc(&None),
        v.stories_muted.unwrap_or(false),
    )
}

/// Parse the small JSON subset written by `serialize_notify_settings`.
fn parse_notify_settings(j: &str) -> Option<tl::enums::PeerNotifySettings> {
    let get_bool = |key: &str| -> Option<bool> {
        let pat = format!("\"{key}\":");
        let i = j.find(&pat)? + pat.len();
        let rest = j[i..].trim_start();
        Some(rest.starts_with("true"))
    };
    let get_i32 = |key: &str| -> Option<i32> {
        let pat = format!("\"{key}\":");
        let i = j.find(&pat)? + pat.len();
        let rest = j[i..].trim_start();
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        rest[..end].trim().parse().ok()
    };
    Some(tl::enums::PeerNotifySettings::Settings(
        tl::types::PeerNotifySettings {
            show_previews: Some(get_bool("show_previews").unwrap_or(true)),
            silent: Some(get_bool("silent").unwrap_or(false)),
            mute_until: Some(get_i32("mute_until").unwrap_or(0)),
            ios_sound: None,
            android_sound: None,
            other_sound: None,
            stories_muted: Some(get_bool("stories_muted").unwrap_or(false)),
            stories_hide_sender: None,
            stories_ios_sound: None,
            stories_android_sound: None,
            stories_other_sound: None,
        },
    ))
}

fn file_location_key(l: &tl::enums::InputFileLocation) -> Option<String> {
    use tl::enums::InputFileLocation as L;
    match l {
        L::Location(x) => Some(format!("blob:{}", x.volume_id)),
        L::InputDocumentFileLocation(x) => Some(format!("doc:{}", x.id)),
        L::InputPhotoFileLocation(x) => Some(format!("photo:{}", x.id)),
        L::InputPeerPhotoFileLocation(x) => Some(format!("peerphoto:{}", x.photo_id)),
        _ => None,
    }
}

fn guess_file_type(data: &[u8]) -> tl::enums::storage::FileType {
    use tl::enums::storage::FileType as T;
    if data.starts_with(&[0xff, 0xd8, 0xff]) {
        T::FileJpeg
    } else if data.starts_with(b"\x89PNG") {
        T::FilePng
    } else if data.starts_with(b"GIF8") {
        T::FileGif
    } else if data.starts_with(b"%PDF") {
        T::FilePdf
    } else if data.starts_with(b"ID3") || data.starts_with(&[0xff, 0xfb]) {
        T::FileMp3
    } else if data.len() > 12 && &data[4..8] == b"ftyp" {
        T::FileMp4
    } else {
        T::FileUnknown
    }
}

fn bail_rpc<T>(code: i32, message: &str) -> Result<T> {
    Err(RpcError {
        code,
        message: message.to_string(),
    }
    .into())
}

#[derive(Debug)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.code, self.message)
    }
}
impl std::error::Error for RpcError {}

/// Returns true when an error should be reported as a TL `rpc_error`.
pub fn as_rpc_error(err: &anyhow::Error) -> Option<&RpcError> {
    err.downcast_ref::<RpcError>()
}

// ---------------------------------------------------------------- helpers

/// Dispatch a request message and frame every reply against the client
/// message id it answers.
///
/// Batched clients put several requests inside one `msg_container`. Answering
/// with a single `rpc_result` keyed by the container id leaves every inner
/// deferred unresolved, so each inner message gets its own reply instead.
/// `http_wait` is deliberately dropped: it never has a TL reply, and the
/// transport response is what settles it.
pub fn dispatch_replies(ctx: &mut RpcContext, body: &[u8], msg_id: i64) -> Vec<RpcReply> {
    if ctor_of(body) != MSG_CONTAINER {
        log_request_constructor(msg_id, body, "single");
        return frame_reply(ctx, msg_id, body).into_iter().collect();
    }
    match parse_container(body) {
        Ok(messages) => {
            log_batched_requests(msg_id, &messages);
            messages
                .into_iter()
                .filter_map(|(inner_msg_id, inner)| {
                    if ctor_of(&inner) == HTTP_WAIT {
                        return None;
                    }
                    frame_reply(ctx, inner_msg_id, &inner)
                })
                .collect()
        }
        Err(error) => vec![RpcReply {
            req_msg_id: msg_id,
            body: framed_error(msg_id, &error, body),
        }],
    }
}

fn log_request_constructor(msg_id: i64, body: &[u8], shape: &'static str) {
    let ctor = request_ctor(body);
    tracing::info!(
        msg_id,
        shape,
        constructor = format_args!("{ctor:#010x}"),
        method = request_method_name(body),
        "MTProto RPC request"
    );
}

fn log_batched_requests(msg_id: i64, messages: &[(i64, Vec<u8>)]) {
    tracing::info!(
        msg_id,
        count = messages.len(),
        methods = ?messages
            .iter()
            .map(|(inner_msg_id, body)| (*inner_msg_id, request_method_name(body)))
            .collect::<Vec<_>>(),
        "MTProto RPC request batch"
    );
}

fn request_method_name(body: &[u8]) -> &'static str {
    let ctor = request_ctor(body);
    match ctor {
        0xc4f9_186b => "help.getConfig",
        0xa677_244f => "auth.sendCode",
        0x8d52_a951 => "auth.signIn",
        HTTP_WAIT => "http.wait",
        MSG_CONTAINER => "msg_container",
        _ => "other",
    }
}

fn request_ctor(body: &[u8]) -> u32 {
    let mut body = body;
    loop {
        let ctor = ctor_of(body);
        match ctor {
            0xda9b_0d0d => {
                if body.len() < 8 {
                    return ctor;
                }
                body = &body[8..];
            }
            0xbf94_59b7 => {
                if body.len() < 4 {
                    return ctor;
                }
                body = &body[4..];
            }
            0xc1cd_5ea9 => match init_connection_query_offset(body) {
                Ok(offset) if offset < body.len() => body = &body[offset..],
                _ => return ctor,
            },
            _ => return ctor,
        }
    }
}

fn init_connection_query_offset(body: &[u8]) -> Result<usize> {
    if body.len() < 8 {
        bail!("initConnection too short");
    }
    let mut off = 4usize;
    let flags = read_i32(body, &mut off)?;
    let _api_id = read_i32(body, &mut off)?;
    let _device_model = read_string(body, &mut off)?;
    let _system_version = read_string(body, &mut off)?;
    let _app_version = read_string(body, &mut off)?;
    let _system_lang = read_string(body, &mut off)?;
    let _lang_pack = read_string(body, &mut off)?;
    let _lang_code = read_string(body, &mut off)?;
    if flags & 1 != 0 {
        skip_value(body, &mut off)?;
    }
    if flags & 2 != 0 {
        skip_value(body, &mut off)?;
    }
    Ok(off)
}

/// Turn dispatched replies into the single message body to put on the wire.
///
/// One reply goes out as-is. Several are packed into a `msg_container` so
/// every inner reply keeps its own `req_msg_id`. Returns `None` when the
/// request produced nothing to send.
pub fn frame_replies(
    replies: Vec<RpcReply>,
    session_id: i64,
    msg_ids: &mut MsgIdGen,
    seq_nos: &mut SeqNoGen,
) -> Option<(Vec<u8>, i32)> {
    if replies.is_empty() {
        return None;
    }
    if replies.len() == 1 {
        let mut replies = replies;
        let reply = replies.pop().unwrap();
        return Some((reply.body, seq_nos.next(session_id, true)));
    }
    let entries: Vec<(i64, i32, Vec<u8>)> = replies
        .into_iter()
        .map(|reply| {
            (
                msg_ids.next(true),
                seq_nos.next(session_id, true),
                reply.body,
            )
        })
        .collect();
    let seq_no = seq_nos.next(session_id, false);
    Some((msg_container(&entries), seq_no))
}

fn frame_reply(ctx: &mut RpcContext, req_msg_id: i64, body: &[u8]) -> Option<RpcReply> {
    match dispatch(ctx, body) {
        Ok(result) if result.is_empty() => None,
        Ok(result) => Some(RpcReply {
            req_msg_id,
            body: rpc_result(req_msg_id, &result),
        }),
        Err(error) => Some(RpcReply {
            req_msg_id,
            body: framed_error(req_msg_id, &error, body),
        }),
    }
}

fn framed_error(req_msg_id: i64, error: &anyhow::Error, body: &[u8]) -> Vec<u8> {
    let detail = match error.downcast_ref::<RpcError>() {
        Some(rpc) => rpc_error(rpc.code, &rpc.message),
        None => {
            tracing::warn!(
                "internal error handling request {:#010x}: {:#}",
                ctor_of(body),
                error
            );
            rpc_error(400, "INTERNAL_ERROR")
        }
    };
    rpc_result(req_msg_id, &detail)
}

fn ctor_of(body: &[u8]) -> u32 {
    if body.len() < 4 {
        return 0;
    }
    u32::from_le_bytes(body[0..4].try_into().unwrap())
}

fn parse_container(body: &[u8]) -> Result<Vec<(i64, Vec<u8>)>> {
    let mut off = 4usize;
    let count = read_i32(body, &mut off)?;
    if count < 0 {
        bail!("negative container count");
    }
    let count = count as usize;
    // Every entry costs at least msg_id + seq_no + length: reject a count that
    // cannot fit before reserving anything.
    if count > body.len().saturating_sub(off) / 20 {
        bail!("container count exceeds remaining bytes");
    }
    let mut messages = Vec::with_capacity(count);
    for _ in 0..count {
        let msg_id = read_i64(body, &mut off)?;
        let _seq_no = read_i32(body, &mut off)?;
        let len = read_i32(body, &mut off)?;
        if len < 0 || (len as usize) > body.len() - off {
            bail!("container overruns buffer");
        }
        let len = len as usize;
        messages.push((msg_id, body[off..off + len].to_vec()));
        off += len;
    }
    Ok(messages)
}

fn handle_send_code(ctx: &mut RpcContext, f: &tl::functions::auth::SendCode) -> Result<Vec<u8>> {
    let phone = normalize_phone(&f.phone_number);
    if !ctx.store.get_user_by_phone(&phone)?.is_some() && !phone.is_empty() {
        // Codes can only be issued for admin-registered accounts.
        tracing::warn!("auth.sendCode rejected unregistered phone: {}", phone);
        return bail_rpc(400, "PHONE_NUMBER_INVALID");
    }
    let code = ctx.cfg.login_code.clone().unwrap_or_else(|| "00000".into());
    ctx.store.issue_login_code(&phone, &code, 300)?;
    tracing::info!(phone, "auth.sendCode accepted");
    let sent = tl::types::auth::SentCode {
        r#type: tl::enums::auth::SentCodeType::App(tl::types::auth::SentCodeTypeApp { length: 5 }),
        phone_code_hash: hash_for(&phone),
        next_type: Some(tl::enums::auth::CodeType::Sms),
        timeout: Some(300),
    };
    Ok(tl::enums::auth::SentCode::Code(sent).to_bytes())
}

fn handle_sign_in(ctx: &mut RpcContext, f: &tl::functions::auth::SignIn) -> Result<Vec<u8>> {
    let phone = normalize_phone(&f.phone_number);
    let user = ctx
        .store
        .get_user_by_phone(&phone)?
        .ok_or_else(|| RpcError {
            code: 400,
            message: "PHONE_NUMBER_INVALID".into(),
        })?;
    let code = f.phone_code.as_deref().unwrap_or("");
    if !ctx.store.check_login_code(&phone, code)? {
        return bail_rpc(400, "PHONE_CODE_INVALID");
    }
    let device = ctx.store.device_info(ctx.auth_key_id)?;
    let created = now();
    ctx.store.save_session(
        &format!("sess-{}", ctx.auth_key_id),
        user.id,
        ctx.auth_key_id,
        &device.0,
        &device.1,
        device.2,
        60 * 60 * 24 * 365,
    )?;
    ctx.store.save_auth_key(ctx.auth_key_id, &[0u8; 256])?;
    let auth = tl::types::auth::Authorization {
        setup_password_required: false,
        otherwise_relogin_days: None,
        tmp_sessions: None,
        future_auth_token: None,
        user: build_user(ctx, &user)?,
    };
    ctx.user_id = user.id;
    Ok(tl::enums::auth::Authorization::Authorization(auth).to_bytes())
}

fn handle_get_dialogs(
    ctx: &RpcContext,
    f: &tl::functions::messages::GetDialogs,
) -> Result<Vec<u8>> {
    let limit = f.limit.clamp(1, 200) as usize;
    let rows = ctx.store.dialogs_for(ctx.user_id)?;
    let mut dialogs = Vec::new();
    let mut messages = Vec::new();
    let mut users = Vec::new();
    let mut chats = Vec::new();
    for row in rows.iter().take(limit) {
        let peer = dialog_peer(row.dialog_type.as_str(), row.dialog_id);
        let top = if row.top_message > 0 {
            find_message(
                ctx,
                row.dialog_type.as_str(),
                row.dialog_id,
                row.top_message,
            )?
        } else {
            None
        };
        let top_id = row.top_message;
        if let Some(m) = top.clone() {
            messages.push(build_message(&m, self_peer(&m, ctx.user_id)));
        }
        let notify = tl::types::PeerNotifySettings {
            show_previews: None,
            silent: None,
            mute_until: None,
            ios_sound: None,
            android_sound: None,
            other_sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_ios_sound: None,
            stories_android_sound: None,
            stories_other_sound: None,
        };
        let dlg = tl::types::Dialog {
            pinned: row.pinned,
            unread_mark: false,
            view_forum_as_messages: false,
            peer: peer.clone(),
            top_message: top_id,
            read_inbox_max_id: row.read_max_id,
            read_outbox_max_id: 0,
            unread_count: row.unread_count,
            unread_mentions_count: 0,
            unread_reactions_count: 0,
            unread_poll_votes_count: 0,
            notify_settings: tl::enums::PeerNotifySettings::Settings(notify),
            pts: None,
            draft: None,
            folder_id: None,
            ttl_period: None,
        };
        dialogs.push(tl::enums::Dialog::Dialog(dlg));
        collect_refs(ctx, &peer, &mut users, &mut chats)?;
    }
    let resp = tl::types::messages::Dialogs {
        dialogs,
        messages,
        chats,
        users,
    };
    Ok(tl::enums::messages::Dialogs::Dialogs(resp).to_bytes())
}

fn handle_get_history(
    ctx: &RpcContext,
    f: &tl::functions::messages::GetHistory,
) -> Result<Vec<u8>> {
    let (kind, id) = resolve_peer(&f.peer)?;
    let rows = ctx.store.history(&kind, id, f.limit, f.offset_id)?;
    let mut messages = Vec::new();
    let mut users = Vec::new();
    let mut chats = Vec::new();
    for r in &rows {
        let peer = peer_of_dialog(&kind, id, ctx.user_id);
        messages.push(build_message(r, peer.clone()));
        collect_refs(ctx, &peer, &mut users, &mut chats)?;
    }
    let resp = tl::types::messages::Messages {
        messages,
        topics: Vec::new(),
        chats,
        users,
    };
    Ok(tl::enums::messages::Messages::Messages(resp).to_bytes())
}

fn handle_send_message(ctx: &mut RpcContext, f: &SendMessageArgs) -> Result<Vec<u8>> {
    let (kind, id) = resolve_peer(&f.peer)?;
    let sender = if kind == "user" && id == ctx.user_id {
        ctx.user_id
    } else {
        ctx.user_id
    };
    let msg_id = ctx.store.insert_message(
        &kind,
        id,
        sender,
        None,
        &f.message,
        "",
        None,
        None,
        f.random_id,
    )?;
    let (pts, pts_count) = bump_pts(ctx, ctx.user_id)?;
    if kind == "user" && id != ctx.user_id {
        ctx.store.push_bot_update(
            id,
            ctx.user_id,
            id,
            "message",
            &serde_json::to_string(&serde_json::json!({
                "message_id": msg_id,
                "chat_id": ctx.user_id,
                "from_id": ctx.user_id,
                "text": f.message,
            }))?,
        )?;
    }
    ctx.store
        .touch_dialog(ctx.user_id, &kind, id, msg_id, id != ctx.user_id)?;
    let resp = tl::types::UpdateShortSentMessage {
        out: true,
        id: msg_id,
        pts,
        pts_count,
        date: now() as i32,
        media: None,
        entities: None,
        ttl_period: None,
    };
    Ok(tl::enums::Updates::UpdateShortSentMessage(resp).to_bytes())
}

fn handle_edit_message(ctx: &mut RpcContext, f: &EditMessageArgs) -> Result<Vec<u8>> {
    let (kind, id) = resolve_peer(&f.peer)?;
    let text = f.message.clone().unwrap_or_default();
    if !ctx.store.edit_message(&kind, id, f.id, &text)? {
        return bail_rpc(400, "MESSAGE_NOT_MODIFIED");
    }
    let (pts, pts_count) = bump_pts(ctx, ctx.user_id)?;
    let row = ctx
        .store
        .get_message(&kind, id, f.id)?
        .ok_or_else(|| RpcError {
            code: 400,
            message: "MESSAGE_ID_INVALID".into(),
        })?;
    let peer = peer_of_dialog(&kind, id, ctx.user_id);
    let msg = build_message(&row, peer);
    let upd = tl::types::UpdateEditMessage {
        message: msg,
        pts,
        pts_count,
    };
    let resp = tl::types::Updates {
        updates: vec![tl::enums::Update::EditMessage(upd)],
        users: Vec::new(),
        chats: Vec::new(),
        date: now() as i32,
        seq: 1,
    };
    Ok(tl::enums::Updates::Updates(resp).to_bytes())
}

fn handle_delete_messages(
    ctx: &mut RpcContext,
    f: &tl::functions::messages::DeleteMessages,
) -> Result<Vec<u8>> {
    let rows = ctx.store.dialogs_for(ctx.user_id)?;
    let mut deleted = 0;
    for row in rows {
        deleted += ctx
            .store
            .delete_messages(&row.dialog_type, row.dialog_id, &f.id)?;
    }
    let (pts, _) = bump_pts(ctx, ctx.user_id)?;
    let resp = tl::types::messages::AffectedMessages {
        pts,
        pts_count: deleted.max(1) as i32,
    };
    Ok(tl::enums::messages::AffectedMessages::Messages(resp).to_bytes())
}

fn handle_read_history(
    ctx: &mut RpcContext,
    f: &tl::functions::messages::ReadHistory,
) -> Result<Vec<u8>> {
    let rows = ctx.store.dialogs_for(ctx.user_id)?;
    for row in rows {
        ctx.store
            .set_dialog_read(ctx.user_id, &row.dialog_type, row.dialog_id, f.max_id)?;
    }
    let (pts, _) = bump_pts(ctx, ctx.user_id)?;
    let resp = tl::types::messages::AffectedHistory {
        pts,
        pts_count: 0,
        offset: 0,
    };
    Ok(tl::enums::messages::AffectedHistory::History(resp).to_bytes())
}

fn handle_get_state(ctx: &RpcContext) -> Result<Vec<u8>> {
    let (pts, qts, seq, date) = ctx.store.state_for(ctx.user_id)?;
    let state = tl::types::updates::State {
        pts,
        qts,
        date: date as i32,
        seq,
        unread_count: 0,
    };
    Ok(tl::enums::updates::State::State(state).to_bytes())
}

fn bump_pts(ctx: &RpcContext, user: i64) -> Result<(i32, i32)> {
    ctx.store.bump_pts(user)
}

// -------------------------------------------------------- TL construction

pub fn build_config(ctx: &RpcContext) -> Result<tl::enums::Config> {
    let ip = ctx
        .cfg
        .public_ip(std::net::SocketAddr::from(([0, 0, 0, 0], 0)));
    let port = ctx.cfg.mtproto_port as i32;
    let dc = ctx.cfg.dc_id;
    let mut dc_options = vec![tl::enums::DcOption::Option(tl::types::DcOption {
        ipv6: false,
        media_only: false,
        tcpo_only: false,
        cdn: false,
        r#static: false,
        this_port_only: false,
        id: dc,
        ip_address: ip.clone(),
        port,
        secret: None,
    })];
    if let Some(v6) = ctx
        .cfg
        .public_ipv6(std::net::SocketAddr::from(([0, 0, 0, 0], 0)))
    {
        dc_options.push(tl::enums::DcOption::Option(tl::types::DcOption {
            ipv6: true,
            media_only: false,
            tcpo_only: false,
            cdn: false,
            r#static: false,
            this_port_only: false,
            id: dc,
            ip_address: v6,
            port,
            secret: None,
        }));
    }
    let now_ts = now() as i32;
    Ok(tl::enums::Config::Config(tl::types::Config {
        default_p2p_contacts: false,
        preload_featured_stickers: false,
        revoke_pm_inbox: true,
        blocked_mode: false,
        force_try_ipv6: false,
        date: now_ts,
        expires: now_ts + 86400,
        test_mode: false,
        this_dc: dc,
        dc_options,
        dc_txt_domain_name: String::new(),
        chat_size_max: 200,
        megagroup_size_max: 200000,
        forwarded_count_max: 100,
        online_update_period_ms: 120000,
        offline_blur_timeout_ms: 5000,
        offline_idle_timeout_ms: 30000,
        online_cloud_timeout_ms: 300000,
        notify_cloud_delay_ms: 30000,
        notify_default_delay_ms: 1500,
        push_chat_period_ms: 60000,
        push_chat_limit: 2,
        edit_time_limit: 48 * 3600,
        revoke_time_limit: 172800,
        revoke_pm_time_limit: 172800,
        rating_e_decay: 24 * 3600,
        stickers_recent_limit: 200,
        channels_read_media_period: 604800,
        tmp_sessions: Some(16),
        call_receive_timeout_ms: 20000,
        call_ring_timeout_ms: 90000,
        call_connect_timeout_ms: 30000,
        call_packet_timeout_ms: 5000,
        me_url_prefix: format!("tg://resolve?domain=@"),
        autoupdate_url_prefix: None,
        gif_search_username: None,
        venue_search_username: None,
        img_search_username: None,
        static_maps_provider: None,
        caption_length_max: 1024,
        message_length_max: 4096,
        webfile_dc_id: dc,
        suggested_lang_code: Some("en".into()),
        // `suggested_lang_code`, `lang_pack_version` and
        // `base_lang_pack_version` all share `flags.2`. Writing only the
        // language code sets the bit while omitting the two integers that
        // follow it, which makes the result unparsable (clients stop at EOF).
        lang_pack_version: Some(0),
        base_lang_pack_version: Some(0),
        reactions_default: None,
        autologin_token: None,
    }))
}

pub fn build_user(ctx: &RpcContext, u: &UserRow) -> Result<tl::enums::User> {
    let is_self = u.id == ctx.user_id;
    let status = if is_self {
        tl::enums::UserStatus::Empty
    } else {
        tl::enums::UserStatus::Offline(tl::types::UserStatusOffline {
            was_online: now() as i32,
        })
    };
    let names: Vec<tl::enums::Username> = if u.username.is_empty() {
        None
    } else {
        Some(vec![tl::enums::Username::Username(tl::types::Username {
            editable: true,
            active: true,
            username: u.username.clone(),
        })])
    }
    .unwrap_or_default();
    let user = tl::types::User {
        is_self,
        contact: false,
        mutual_contact: false,
        deleted: false,
        bot: u.is_bot,
        bot_chat_history: false,
        bot_nochats: false,
        verified: false,
        restricted: false,
        min: false,
        bot_inline_geo: false,
        support: false,
        scam: false,
        apply_min_photo: false,
        fake: false,
        bot_attach_menu: false,
        premium: false,
        attach_menu_enabled: false,
        bot_can_edit: false,
        close_friend: false,
        stories_hidden: false,
        stories_unavailable: false,
        contact_require_premium: false,
        bot_business: false,
        bot_has_main_app: false,
        bot_forum_view: false,
        bot_forum_can_manage_topics: false,
        bot_can_manage_bots: false,
        bot_guestchat: false,
        bot_guard: false,
        id: u.id,
        access_hash: Some(u.access_hash),
        first_name: Some(u.first_name.clone()).filter(|s| !s.is_empty()),
        last_name: Some(u.last_name.clone()).filter(|s| !s.is_empty()),
        username: Some(u.username.clone()).filter(|s| !s.is_empty()),
        phone: Some(u.phone.clone()).filter(|s| !s.is_empty()),
        photo: Some(tl::enums::UserProfilePhoto::Empty),
        status: Some(status),
        bot_info_version: if u.is_bot { Some(1) } else { None },
        restriction_reason: None,
        bot_inline_placeholder: None,
        lang_code: Some("en".into()),
        emoji_status: None,
        usernames: if names.is_empty() { None } else { Some(names) },
        stories_max_id: None,
        color: None,
        profile_color: None,
        bot_active_users: None,
        bot_verification_icon: None,
        send_paid_messages_stars: None,
    };
    Ok(tl::enums::User::User(user))
}

pub fn build_chat(c: &crate::store::ChatRow, participants: i32) -> tl::enums::Chat {
    tl::enums::Chat::Chat(tl::types::Chat {
        creator: false,
        left: false,
        deactivated: false,
        call_active: false,
        call_not_empty: false,
        noforwards: false,
        id: c.id,
        title: c.title.clone(),
        photo: tl::enums::ChatPhoto::Empty,
        participants_count: participants,
        date: c.created_at as i32,
        version: 1,
        migrated_to: None,
        admin_rights: None,
        default_banned_rights: None,
    })
}

fn default_notify() -> tl::enums::PeerNotifySettings {
    tl::enums::PeerNotifySettings::Settings(tl::types::PeerNotifySettings {
        show_previews: None,
        silent: None,
        mute_until: None,
        ios_sound: None,
        android_sound: None,
        other_sound: None,
        stories_muted: None,
        stories_hide_sender: None,
        stories_ios_sound: None,
        stories_android_sound: None,
        stories_other_sound: None,
    })
}

/// TL `Message` and `auth.Authorization` are the only two objects whose
/// constructor id changed between layer 224 (Telethon and most third-party
/// clients) and layer 227 (the schema this server is generated from). Their
/// field layouts are byte-identical, so a reply can be re-stamped for whichever
/// layer the client announced.
pub const LAYER_227: i32 = 227;
const MESSAGE_CTOR_224: u32 = 0x3ae56482;
const MESSAGE_CTOR_227: u32 = 0x7600b9d3;
const AUTH_AUTHORIZATION_CTOR_224: u32 = 0x2ea2c0d4;
const AUTH_AUTHORIZATION_CTOR_227: u32 = 0xad01d61d;

/// Rewrites a single object's leading constructor id when the client speaks an
/// older layer than the schema this server serializes with.
fn restamp_ctor(body: &mut [u8], layer: i32, current: u32, legacy: u32) {
    if layer >= LAYER_227 || body.len() < 4 {
        return;
    }
    if u32::from_le_bytes(body[0..4].try_into().unwrap()) == current {
        body[0..4].copy_from_slice(&legacy.to_le_bytes());
    }
}

/// Re-stamps every `Message` inside a `vector<Message>` in a response body.
///
/// Responses nest their message vector at a different offset depending on the
/// envelope (`messages.messages`, `messages.dialogs`, `messages.peerDialogs`,
/// `messages.messagesSlice`, ...), so rather than hard-code each one this walks
/// the body looking for a vector whose every element parses as a `Message`.
/// Requiring a clean parse of all elements keeps `vector<Chat>`/`vector<User>`
/// from being mistaken for it.
fn restamp_message_list(body: &mut [u8], layer: i32) {
    if layer >= LAYER_227 || body.len() < 12 {
        return;
    }
    let needle = 0x1cb5c415u32.to_le_bytes();
    for i in 0..body.len().saturating_sub(8) {
        if body[i..i + 4] != needle {
            continue;
        }
        let count = i32::from_le_bytes(body[i + 4..i + 8].try_into().unwrap());
        if count <= 0 || count > 100_000 {
            continue;
        }
        let mut cursor = tl::Cursor::from_slice(&body[i + 8..]);
        let mut offsets = Vec::with_capacity(count as usize);
        let mut all_ok = true;
        for _ in 0..count {
            let start = cursor.pos();
            // The generated `Deserializable` impl for the bare struct reads
            // only the fields; the enum consumes the constructor as well.
            if tl::enums::Message::deserialize(&mut cursor).is_err() {
                all_ok = false;
                break;
            }
            offsets.push(i + 8 + start);
        }
        if !all_ok {
            continue;
        }

        // Only rewrite when the vector really held layer-227 messages.
        if !offsets.iter().any(|off| {
            off + 4 <= body.len()
                && u32::from_le_bytes(body[*off..*off + 4].try_into().unwrap()) == MESSAGE_CTOR_227
        }) {
            continue;
        }
        for off in offsets {
            if off + 4 <= body.len()
                && u32::from_le_bytes(body[off..off + 4].try_into().unwrap()) == MESSAGE_CTOR_227
            {
                body[off..off + 4].copy_from_slice(&MESSAGE_CTOR_224.to_le_bytes());
            }
        }
        return;
    }
}

/// Re-stamps an `auth.Authorization` reply for older-layer clients.
fn restamp_authorization(body: &mut [u8], layer: i32) {
    restamp_ctor(
        body,
        layer,
        AUTH_AUTHORIZATION_CTOR_227,
        AUTH_AUTHORIZATION_CTOR_224,
    );
}

/// Downgrades the constructor ids in a reply to the layer the client announced.
pub fn restamp_response_for_layer(body: &mut [u8], layer: i32, ctor: u32) {
    if layer >= LAYER_227 {
        return;
    }
    match ctor {
        // auth.signIn / auth.signUp / auth.importAuthorization
        0x8d52a951 | 0xaac7b717 | 0xa57a7dad => restamp_authorization(body, layer),
        // Every reply that can carry `Message` objects.
        0xa0f4cb4f // messages.getDialogs
        | 0x4423e6c5 // messages.getHistory
        | 0x63c66506 // messages.getMessages
        | 0xe470bcfd // messages.getPeerDialogs
        | 0x29ee847a // messages.search
        | 0xe40ca104 // messages.getCommonChats
        | 0x49e9528f // messages.getChats
        | 0xfef48f62 // messages.sendMessage
        | 0x545cd15a // messages.sendMessage (layer <= 224)
        | 0xb106e66c // messages.editMessage
        | 0x51e842e1 // messages.editMessage (layer <= 224)
        | 0x0e306d3a // messages.readHistory
        | 0x19c2f763 // updates.getDifference
        | 0xedd4882a => restamp_message_list(body, layer),
        _ => {}
    }
}

fn build_message(m: &MessageRow, peer: tl::enums::Peer) -> tl::enums::Message {
    let from = if let Some(sid) = m.sender_chat_id {
        tl::enums::Peer::Chat(tl::types::PeerChat { chat_id: sid })
    } else {
        tl::enums::Peer::User(tl::types::PeerUser {
            user_id: m.sender_id,
        })
    };
    let media = None;
    tl::enums::Message::Message(tl::types::Message {
        out: false,
        mentioned: false,
        media_unread: false,
        silent: false,
        post: false,
        from_scheduled: false,
        legacy: false,
        edit_hide: m.edited,
        pinned: false,
        noforwards: false,
        invert_media: false,
        offline: false,
        video_processing_pending: false,
        paid_suggested_post_stars: false,
        paid_suggested_post_ton: false,
        id: m.id,
        from_id: Some(from),
        from_boosts_applied: None,
        from_rank: None,
        peer_id: peer,
        saved_peer_id: None,
        fwd_from: None,
        via_bot_id: None,
        via_business_bot_id: None,
        guestchat_via_from: None,
        reply_to: None,
        date: m.date as i32,
        message: m.message.clone(),
        media,
        reply_markup: None,
        entities: None,
        views: None,
        forwards: None,
        replies: None,
        edit_date: if m.edited { Some(m.date as i32) } else { None },
        post_author: None,
        grouped_id: None,
        reactions: None,
        restriction_reason: None,
        ttl_period: None,
        quick_reply_shortcut_id: None,
        effect: None,
        factcheck: None,
        report_delivery_until_date: None,
        paid_message_stars: None,
        suggested_post: None,
        schedule_repeat_period: None,
        summary_from_language: None,
        rich_message: None,
    })
}

// ------------------------------------------------------------- utilities

use crate::store::normalize_phone;

fn hash_for(s: &str) -> String {
    let h = crate::crypto::sha1(s.as_bytes());
    hex::encode(h)
}

fn read_i32(buf: &[u8], off: &mut usize) -> Result<i32> {
    if *off + 4 > buf.len() {
        bail!("read_i32 out of bounds");
    }
    let v = i32::from_le_bytes(buf[*off..*off + 4].try_into().unwrap());
    *off += 4;
    Ok(v)
}

fn read_i64(buf: &[u8], off: &mut usize) -> Result<i64> {
    if *off + 8 > buf.len() {
        bail!("read_i64 out of bounds");
    }
    let v = i64::from_le_bytes(buf[*off..*off + 8].try_into().unwrap());
    *off += 8;
    Ok(v)
}

fn read_string(buf: &[u8], off: &mut usize) -> Result<String> {
    if *off >= buf.len() {
        bail!("string out of bounds");
    }
    let first = buf[*off];
    let (len, hdr) = if first == 254 {
        if *off + 4 > buf.len() {
            bail!("string header out of bounds");
        }
        let l = u32::from_le_bytes([buf[*off + 1], buf[*off + 2], buf[*off + 3], 0]) as usize;
        (l, 4usize)
    } else {
        (first as usize, 1usize)
    };
    *off += hdr;
    if *off + len > buf.len() {
        bail!("string body out of bounds");
    }
    let s = String::from_utf8_lossy(&buf[*off..*off + len]).into_owned();
    *off += len;
    let total = hdr + len;
    *off += (4 - (total % 4)) % 4;
    Ok(s)
}

/// Skip one arbitrary TL value by walking the type structure. Only the subset
/// used by wrapper functions is needed, so a small explicit parser is enough.
fn skip_value(buf: &[u8], off: &mut usize) -> Result<()> {
    if *off + 4 > buf.len() {
        bail!("value out of bounds");
    }
    let ctor = u32::from_le_bytes(buf[*off..*off + 4].try_into().unwrap());
    *off += 4;
    match ctor {
        // inputClientProxy / JSONValue are not expected in practice; consume
        // conservatively by returning an error rather than corrupting the stream.
        _ => bail!("unsupported wrapper parameter {:#010x}", ctor),
    }
}

/// Reads the `flags:#` word that TL uses to gate optional fields.
fn read_flags(buf: &[u8], off: &mut usize) -> Result<u32> {
    Ok(read_i32(buf, off)? as u32)
}

/// Fields the server needs from a `messages.sendMessage` request, whichever
/// layer the client speaks. The leading fields are identical across layers, so
/// the trailing optionals are simply walked past.
struct SendMessageArgs {
    peer: tl::enums::InputPeer,
    message: String,
    random_id: i64,
}

/// `messages.sendMessage#545cd15a` (layer <= 224, as sent by Telethon) and the
/// current `#fef48f62`: same leading layout, extra trailing optionals.
fn parse_send_message(args: &[u8]) -> Result<SendMessageArgs> {
    let mut cur = tl::Cursor::from_slice(args);
    let flags = u32::deserialize(&mut cur)?;
    let peer = tl::enums::InputPeer::deserialize(&mut cur)?;
    if flags & 1 != 0 {
        let _reply_to = tl::enums::InputReplyTo::deserialize(&mut cur)?;
    }
    let message = String::deserialize(&mut cur)?;
    let random_id = i64::deserialize(&mut cur)?;
    if flags & 4 != 0 {
        let _reply_markup = tl::enums::ReplyMarkup::deserialize(&mut cur)?;
    }
    if flags & 8 != 0 {
        let _entities = Vec::<tl::enums::MessageEntity>::deserialize(&mut cur)?;
    }
    if flags & 1024 != 0 {
        let _schedule_date = i32::deserialize(&mut cur)?;
    }
    if flags & 16777216 != 0 {
        let _schedule_repeat_period = i32::deserialize(&mut cur)?;
    }
    if flags & 8192 != 0 {
        let _send_as = tl::enums::InputPeer::deserialize(&mut cur)?;
    }
    if flags & 131072 != 0 {
        let _quick_reply_shortcut = tl::enums::InputQuickReplyShortcut::deserialize(&mut cur)?;
    }
    if flags & 262144 != 0 {
        let _effect = i64::deserialize(&mut cur)?;
    }
    if flags & 2097152 != 0 {
        let _allow_paid_stars = i64::deserialize(&mut cur)?;
    }
    if flags & 4194304 != 0 {
        let _suggested_post = tl::enums::SuggestedPost::deserialize(&mut cur)?;
    }
    Ok(SendMessageArgs {
        peer,
        message,
        random_id,
    })
}

/// Fields the server needs from a `messages.editMessage` request.
struct EditMessageArgs {
    peer: tl::enums::InputPeer,
    id: i32,
    message: Option<String>,
}

/// `messages.editMessage#51e842e1` (layer <= 224) and the current `#b106e66c`.
fn parse_edit_message(args: &[u8]) -> Result<EditMessageArgs> {
    let mut cur = tl::Cursor::from_slice(args);
    let flags = u32::deserialize(&mut cur)?;
    let peer = tl::enums::InputPeer::deserialize(&mut cur)?;
    let id = i32::deserialize(&mut cur)?;
    let message = if flags & 2048 != 0 {
        Some(String::deserialize(&mut cur)?)
    } else {
        None
    };
    if flags & 16384 != 0 {
        let _media = tl::enums::InputMedia::deserialize(&mut cur)?;
    }
    if flags & 4 != 0 {
        let _reply_markup = tl::enums::ReplyMarkup::deserialize(&mut cur)?;
    }
    if flags & 8 != 0 {
        let _entities = Vec::<tl::enums::MessageEntity>::deserialize(&mut cur)?;
    }
    if flags & 32768 != 0 {
        let _schedule_date = i32::deserialize(&mut cur)?;
    }
    if flags & 262144 != 0 {
        let _schedule_repeat_period = i32::deserialize(&mut cur)?;
    }
    if flags & 131072 != 0 {
        let _quick_reply_shortcut_id = i32::deserialize(&mut cur)?;
    }
    Ok(EditMessageArgs { peer, id, message })
}

fn resolve_peer(input: &tl::enums::InputPeer) -> Result<(String, i64)> {
    use tl::enums::InputPeer as P;
    match input {
        P::PeerSelf => Ok(("self".into(), 0)),
        P::User(u) => Ok(("user".into(), u.user_id)),
        P::Chat(c) => Ok(("chat".into(), c.chat_id)),
        P::Channel(c) => Ok(("channel".into(), c.channel_id)),
        P::UserFromMessage(u) => Ok(("user".into(), u.user_id)),
        P::ChannelFromMessage(c) => Ok(("channel".into(), c.channel_id)),
        P::Empty => return bail_rpc(400, "PEER_ID_INVALID"),
    }
}

fn resolve_input_user(
    store: &Store,
    i: &tl::enums::InputUser,
    self_id: i64,
) -> Result<Option<UserRow>> {
    use tl::enums::InputUser as U;
    let id = match i {
        U::User(u) => u.user_id,
        U::FromMessage(u) => u.user_id,
        // `inputUserSelf` means *the caller*, not "any user in the database".
        U::UserSelf => self_id,
        U::Empty => return Ok(None),
    };
    if id == 0 {
        return Ok(None);
    }
    store.get_user(id)
}

fn find_message(ctx: &RpcContext, kind: &str, dialog: i64, id: i32) -> Result<Option<MessageRow>> {
    ctx.store.get_message(kind, dialog, id)
}

fn dialog_peer(kind: &str, id: i64) -> tl::enums::Peer {
    match kind {
        "chat" => tl::enums::Peer::Chat(tl::types::PeerChat { chat_id: id }),
        "channel" => tl::enums::Peer::Channel(tl::types::PeerChannel { channel_id: id }),
        _ => tl::enums::Peer::User(tl::types::PeerUser { user_id: id }),
    }
}

fn peer_of_dialog(kind: &str, id: i64, self_id: i64) -> tl::enums::Peer {
    // `inputPeerSelf` resolves to `("self", 0)`, and a dialog with yourself is
    // addressed as your own user id.
    if kind == "self" {
        return tl::enums::Peer::User(tl::types::PeerUser { user_id: self_id });
    }
    if kind == "user" && id == self_id {
        return tl::enums::Peer::User(tl::types::PeerUser { user_id: self_id });
    }
    dialog_peer(kind, id)
}

fn self_peer(m: &MessageRow, self_id: i64) -> tl::enums::Peer {
    if m.sender_id == self_id {
        tl::enums::Peer::User(tl::types::PeerUser { user_id: self_id })
    } else if let Some(sid) = m.sender_chat_id {
        tl::enums::Peer::Chat(tl::types::PeerChat { chat_id: sid })
    } else {
        tl::enums::Peer::User(tl::types::PeerUser {
            user_id: m.sender_id,
        })
    }
}

fn collect_refs(
    ctx: &RpcContext,
    peer: &tl::enums::Peer,
    users: &mut Vec<tl::enums::User>,
    chats: &mut Vec<tl::enums::Chat>,
) -> Result<()> {
    match peer {
        tl::enums::Peer::User(u) => {
            if let Some(usr) = ctx.store.get_user(u.user_id)? {
                users.push(build_user(ctx, &usr)?);
            }
        }
        tl::enums::Peer::Chat(c) => {
            if let Some(chat) = ctx.store.get_chat(c.chat_id)? {
                let members = ctx.store.chat_members(chat.id)?.len() as i32;
                chats.push(build_chat(&chat, members));
            }
        }
        tl::enums::Peer::Channel(_) => {}
    }
    Ok(())
}
