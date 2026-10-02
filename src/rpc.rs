//! TL RPC dispatch: turns serialized `functions::*` bodies into serialized
//! response bodies, with graceful `rpc_error` replies for unsupported calls.

use anyhow::{anyhow, bail, Result};
use grammers_tl_types as tl;
use grammers_tl_types::{Deserializable, Serializable};
use std::sync::Arc;

use crate::config::Config;
use crate::store::{now, MessageRow, Store, UserRow};

pub struct RpcContext {
    pub store: Store,
    pub cfg: Arc<Config>,
    pub auth_key_id: i64,
    pub user_id: i64,
    pub layer: i32,
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
    result
}

fn dispatch_inner(ctx: &mut RpcContext, body: &[u8], ctor: u32) -> Result<Vec<u8>> {
    use tl::Serializable as _;
    let store = &ctx.store;

    match ctor {
        // ---------------------------------------------------------------- ping
        0x7abe77ec => {
            let f = tl::functions::Ping::deserialize(&mut tl::Cursor::from_slice(body))?;
            let pong = tl::types::Pong {
                msg_id: 0,
                ping_id: f.ping_id,
            };
            return Ok(pong.to_bytes());
        }

        // ------------------------------------------------------------- help
        0xc4f9186b => {
            let _ = tl::functions::help::GetConfig::deserialize(&mut tl::Cursor::from_slice(body))?;
            return Ok(build_config(ctx)?.to_bytes());
        }
        0x1fb33026 => {
            let _ =
                tl::functions::help::GetNearestDc::deserialize(&mut tl::Cursor::from_slice(body))?;
            let nd = tl::types::NearestDc {
                country: "XX".into(),
                this_dc: ctx.cfg.dc_id,
                nearest_dc: ctx.cfg.dc_id,
            };
            return Ok(tl::enums::NearestDc::Dc(nd).to_bytes());
        }

        // ------------------------------------------------------------- auth
        0xa677244f => {
            let f = tl::functions::auth::SendCode::deserialize(&mut tl::Cursor::from_slice(body))?;
            return handle_send_code(ctx, &f);
        }
        0x8d52a951 => {
            let f = tl::functions::auth::SignIn::deserialize(&mut tl::Cursor::from_slice(body))?;
            return handle_sign_in(ctx, &f);
        }
        0xaac7b717 => {
            // Self-registration is disabled: accounts are created by an admin.
            return bail_rpc(403, "SIGNUP_DISABLED");
        }

        // ------------------------------------------------------------ users
        0xd91a548 => {
            let f = tl::functions::users::GetUsers::deserialize(&mut tl::Cursor::from_slice(body))?;
            let mut out = Vec::new();
            for input in &f.id {
                if let Some(u) = resolve_input_user(store, input)? {
                    out.push(build_user(ctx, &u)?);
                }
            }
            return Ok(tl::RawVec::<tl::enums::User>(out).to_bytes());
        }
        0xb60f5918 => {
            let _f =
                tl::functions::users::GetFullUser::deserialize(&mut tl::Cursor::from_slice(body))?;
            return bail_rpc(400, "USER_FULL_UNSUPPORTED");
        }

        // ---------------------------------------------------------- account
        0x78515775 => {
            let f = tl::functions::account::UpdateProfile::deserialize(
                &mut tl::Cursor::from_slice(body),
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
                body,
            ))?;
            return handle_get_dialogs(ctx, &f);
        }
        0x4423e6c5 => {
            let f = tl::functions::messages::GetHistory::deserialize(&mut tl::Cursor::from_slice(
                body,
            ))?;
            return handle_get_history(ctx, &f);
        }
        0xfef48f62 => {
            let f = tl::functions::messages::SendMessage::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            return handle_send_message(ctx, &f);
        }
        0xb106e66c => {
            let f = tl::functions::messages::EditMessage::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            return handle_edit_message(ctx, &f);
        }
        0xe58e95d2 => {
            let f = tl::functions::messages::DeleteMessages::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            return handle_delete_messages(ctx, &f);
        }
        0xe306d3a => {
            let f = tl::functions::messages::ReadHistory::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            return handle_read_history(ctx, &f);
        }
        0x63c66506 => {
            // messages.getChats
            let f =
                tl::functions::messages::GetChats::deserialize(&mut tl::Cursor::from_slice(body))?;
            let mut chats = Vec::new();
            for id in &f.id {
                if let Some(c) = store.get_chat(*id)? {
                    chats.push(build_chat(&c, 0));
                }
            }
            return Ok(tl::RawVec::<tl::enums::Chat>(chats).to_bytes());
        }
        0xa6f47c87 => {
            // messages.getFullChat
            let f = tl::functions::messages::GetFullChat::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            return handle_get_full_chat(ctx, &f);
        }
        0x628006bc => {
            // messages.createChat
            let f = tl::functions::messages::CreateChat::deserialize(&mut tl::Cursor::from_slice(
                body,
            ))?;
            let chat = store.create_chat(&f.title, ctx.user_id)?;
            for input in &f.users {
                if let Some(u) = resolve_input_user(store, input)? {
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
        0x9db1bb6d => {
            // messages.setTyping -> Bool true
            return Ok(true.to_bytes());
        }

        // ----------------------------------------------------------- updates
        0xedd4882a => {
            let _ =
                tl::functions::updates::GetState::deserialize(&mut tl::Cursor::from_slice(body))?;
            return handle_get_state(ctx);
        }
        0x19c2f763 => {
            let _f = tl::functions::updates::GetDifference::deserialize(
                &mut tl::Cursor::from_slice(body),
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
                body,
            ))?;
            store.put_file_part(f.file_id, f.file_part, &f.bytes)?;
            return Ok(true.to_bytes());
        }
        0xde7b673d => {
            let f = tl::functions::upload::SaveBigFilePart::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            store.put_file_part(f.file_id, f.file_part, &f.bytes)?;
            return Ok(true.to_bytes());
        }

        // ---------------------------------------------------------- contacts
        0x5dd69e12 => {
            let _ = tl::functions::contacts::GetContacts::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            let empty = tl::types::contacts::Contacts {
                contacts: Vec::new(),
                saved_count: 0,
                users: Vec::new(),
            };
            return Ok(tl::enums::contacts::Contacts::Contacts(empty).to_bytes());
        }
        0x2c800b5f => {
            let _f = tl::functions::contacts::ImportContacts::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            let imp = tl::types::contacts::ImportedContacts {
                imported: Vec::new(),
                popular_invites: Vec::new(),
                retry_contacts: Vec::new(),
                users: Vec::new(),
            };
            return Ok(tl::enums::contacts::ImportedContacts::Contacts(imp).to_bytes());
        }
        0x983b02bb => {
            // contacts.resetSaved
            return Ok(tl::enums::Updates::TooLong.to_bytes());
        }

        // ------------------------------------------------------------ langpack
        0x42c6978f => {
            let _f = tl::functions::langpack::GetLanguages::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            return Ok(tl::RawVec::<tl::enums::LangPackLanguage>(Vec::new()).to_bytes());
        }
        0x6a596502 => {
            let f = tl::functions::langpack::GetDifference::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            let diff = tl::types::LangPackDifference {
                lang_code: f.lang_code,
                from_version: f.from_version,
                version: f.from_version,
                strings: Vec::new(),
            };
            return Ok(diff.to_bytes());
        }
        0x8479c748 => {
            let _f = tl::functions::langpack::GetLanguage::deserialize(
                &mut tl::Cursor::from_slice(body),
            )?;
            return bail_rpc(400, "LANG_PACK_LANGUAGE_INVALID");
        }

        // -------------------------------------------------- no-op / benign
        0x62d6b459 => {
            // msgs_ack
            return Ok(Vec::new());
        }
        0x73f1f8dc => {
            // msg_container: recurse over each contained message
            return handle_container(ctx, body);
        }

        _ => {
            let name = tl::name_for_id(ctor);
            return bail_rpc(400, &format!("RPC_ERROR: unsupported method {}", name));
        }
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

fn handle_container(ctx: &mut RpcContext, body: &[u8]) -> Result<Vec<u8>> {
    let mut off = 4usize;
    let count = read_i32(body, &mut off)? as usize;
    let mut combined = Vec::new();
    for _ in 0..count {
        let msg_id = read_i64(body, &mut off)?;
        let _seq = read_i32(body, &mut off)?;
        let len = read_i32(body, &mut off)? as usize;
        if off + len > body.len() {
            bail!("container overruns buffer");
        }
        let inner = &body[off..off + len];
        off += len;
        if inner.len() < 4 {
            continue;
        }
        let inner_ctor = u32::from_le_bytes(inner[0..4].try_into().unwrap());
        if inner_ctor == 0xf35c6d01 || inner_ctor == 0xa7eff811 {
            continue;
        }
        let _ = msg_id;
        if let Ok(resp) = dispatch(ctx, inner) {
            combined.extend_from_slice(&resp);
        }
    }
    Ok(combined)
}

fn handle_send_code(ctx: &mut RpcContext, f: &tl::functions::auth::SendCode) -> Result<Vec<u8>> {
    let phone = normalize_phone(&f.phone_number);
    if !ctx.store.get_user_by_phone(&phone)?.is_some() && !phone.is_empty() {
        // Codes can only be issued for admin-registered accounts.
        return bail_rpc(400, "PHONE_NUMBER_INVALID");
    }
    let code = ctx.cfg.login_code.clone().unwrap_or_else(|| "00000".into());
    ctx.store.issue_login_code(&phone, &code, 300)?;
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

fn handle_send_message(
    ctx: &mut RpcContext,
    f: &tl::functions::messages::SendMessage,
) -> Result<Vec<u8>> {
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

fn handle_edit_message(
    ctx: &mut RpcContext,
    f: &tl::functions::messages::EditMessage,
) -> Result<Vec<u8>> {
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

fn handle_get_full_chat(
    ctx: &RpcContext,
    _f: &tl::functions::messages::GetFullChat,
) -> Result<Vec<u8>> {
    let _ = ctx;
    bail_rpc(400, "CHAT_FULL_UNSUPPORTED")
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

pub fn build_config(ctx: &RpcContext) -> Result<tl::types::Config> {
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
    Ok(tl::types::Config {
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
        lang_pack_version: None,
        base_lang_pack_version: None,
        reactions_default: None,
        autologin_token: None,
    })
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

fn normalize_phone(p: &str) -> String {
    p.chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect()
}

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

fn input_peer_user_id(i: &tl::enums::InputUser) -> Result<i64> {
    use tl::enums::InputUser as U;
    match i {
        U::User(u) => Ok(u.user_id),
        U::Empty => return bail_rpc(400, "USER_ID_INVALID"),
        U::FromMessage(u) => Ok(u.user_id),
        U::UserSelf => bail_rpc(400, "USER_SELF_UNSUPPORTED"),
    }
}

fn resolve_input_user(store: &Store, i: &tl::enums::InputUser) -> Result<Option<UserRow>> {
    use tl::enums::InputUser as U;
    let id = match i {
        U::User(u) => u.user_id,
        U::FromMessage(u) => u.user_id,
        U::UserSelf => return store.all_users().map(|mut users| users.pop()),
        U::Empty => return Ok(None),
    };
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
    if kind == "user" {
        if id == self_id {
            tl::enums::Peer::User(tl::types::PeerUser { user_id: self_id })
        } else {
            dialog_peer(kind, id)
        }
    } else {
        dialog_peer(kind, id)
    }
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
