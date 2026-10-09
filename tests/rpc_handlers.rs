//! End-to-end tests for the hand-written RPC handlers.
//!
//! These drive `rpc::dispatch` with real serialized TL function bodies and
//! parse the replies back with the declared return types, so a handler that
//! returns the wrong shape fails here rather than in a client.

use grammers_tl_types as tl;
use grammers_tl_types::{Deserializable, Serializable};
use std::path::PathBuf;
use std::sync::Arc;
use telegram_server::config::Config;
use telegram_server::rpc::{dispatch, RpcContext};
use telegram_server::store::Store;

struct Fixture {
    ctx: RpcContext,
    _dir: PathBuf,
}

fn setup() -> Fixture {
    let dir = std::env::temp_dir().join(format!("tgsrv-test-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::open(&dir.join("test.db")).unwrap();
    let cfg = Arc::new(Config::new(
        2,
        Some("192.168.37.27".into()),
        None,
        "0.0.0.0".into(),
        24443,
        "0.0.0.0".into(),
        28081,
        dir.join("test.db"),
        dir.join("key.pem"),
        None,
        Some("12345".into()),
    ));
    let self_user = store
        .create_user("15550000001", "Admin", "User", "admin", false, true)
        .unwrap();
    let peer = store
        .create_user("15550000002", "Peer", "Person", "peeruser", false, false)
        .unwrap();
    let chat = store.create_chat("Test Group", self_user.id).unwrap();
    store
        .add_chat_member(chat.id, self_user.id, "creator")
        .unwrap();
    store.add_chat_member(chat.id, peer.id, "member").unwrap();
    let ctx = RpcContext {
        store,
        cfg,
        auth_key_id: 42,
        user_id: self_user.id,
        // The schema this server serializes with. Tests that exercise the
        // layer-downgrade path set a lower value explicitly.
        layer: 227,
    };
    Fixture { ctx, _dir: dir }
}

fn call<F: Serializable, T: Deserializable>(ctx: &mut RpcContext, f: &F) -> T {
    let body = match dispatch(ctx, &f.to_bytes()) {
        Ok(b) => b,
        Err(e) => panic!("dispatch failed: {e:?}"),
    };
    let mut cur = tl::Cursor::from_slice(&body);
    let out = T::deserialize(&mut cur).expect("reply did not parse");
    assert_eq!(cur.pos(), body.len(), "reply left trailing bytes");
    out
}

#[test]
fn auth_send_code_accepts_registered_phone_in_international_format() {
    let mut fx = setup();
    let settings = tl::types::CodeSettings {
        allow_flashcall: false,
        current_number: false,
        allow_app_hash: false,
        allow_missed_call: false,
        allow_firebase: false,
        unknown_number: false,
        logout_tokens: None,
        token: None,
        app_sandbox: None,
    };
    let f = tl::functions::auth::SendCode {
        phone_number: "+1 555 000 0001".into(),
        api_id: 2,
        api_hash: "test".into(),
        settings: tl::enums::CodeSettings::Settings(settings),
    };
    let body = dispatch(&mut fx.ctx, &f.to_bytes()).expect("registered phone must be accepted");
    let _sent = tl::enums::auth::SentCode::deserialize(&mut tl::Cursor::from_slice(&body))
        .expect("sendCode reply must parse");
}

#[test]
fn auth_sign_in_user_uses_tweb_layer_229_schema() {
    let mut fx = setup();
    fx.ctx.layer = 229;
    let settings = tl::types::CodeSettings {
        allow_flashcall: false,
        current_number: false,
        allow_app_hash: false,
        allow_missed_call: false,
        allow_firebase: false,
        unknown_number: false,
        logout_tokens: None,
        token: None,
        app_sandbox: None,
    };
    dispatch(
        &mut fx.ctx,
        &tl::functions::auth::SendCode {
            phone_number: "+15550000001".into(),
            api_id: 2,
            api_hash: "test".into(),
            settings: tl::enums::CodeSettings::Settings(settings),
        }
        .to_bytes(),
    )
    .expect("sendCode must issue a login code");

    let body = dispatch(
        &mut fx.ctx,
        &tl::functions::auth::SignIn {
            phone_number: "+15550000001".into(),
            phone_code_hash: "hash".into(),
            phone_code: Some("12345".into()),
            email_verification: None,
        }
        .to_bytes(),
    )
    .expect("signIn must succeed");

    let mut cursor = tl::Cursor::from_slice(&body);
    let auth_ctor = u32::deserialize(&mut cursor).expect("auth constructor");
    assert_eq!(auth_ctor, 0x2ea2c0d4);
    let auth_flags = u32::deserialize(&mut cursor).expect("auth flags");
    assert_eq!(auth_flags, 0);
    let user_start = cursor.pos();
    let user_ctor = u32::deserialize(&mut cursor).expect("user constructor");
    assert_eq!(user_ctor, 0xb1b8cc83);
    let _user_flags = u32::deserialize(&mut cursor).expect("user flags");
    let user_flags2 = u32::deserialize(&mut cursor).expect("user flags2");
    assert_eq!(
        user_flags2 & (1 << 21),
        0,
        "layer 229 optional linked_community_id must be absent"
    );

    let mut restored = body.clone();
    restored[user_start..user_start + 4].copy_from_slice(&0x31774388u32.to_le_bytes());
    let parsed =
        tl::enums::auth::Authorization::deserialize(&mut tl::Cursor::from_slice(&restored))
            .expect("layer 229 user must retain the shared layer 227 field layout");
    let tl::enums::auth::Authorization::Authorization(auth) = parsed else {
        panic!("unexpected authorization variant");
    };
    let tl::enums::User::User(user) = auth.user else {
        panic!("unexpected user variant");
    };
    assert_eq!(user.id, fx.ctx.user_id);
}

#[test]
fn users_get_full_user_returns_profile() {
    let mut fx = setup();
    let me = fx.ctx.user_id;
    let f = tl::functions::users::GetFullUser {
        id: tl::enums::InputUser::User(tl::types::InputUser {
            user_id: me,
            access_hash: 0,
        }),
    };
    let out: tl::enums::users::UserFull = call(&mut fx.ctx, &f);
    let tl::enums::users::UserFull::Full(full) = out else {
        panic!("expected users::UserFull::Full")
    };
    let tl::enums::UserFull::Full(inner) = full.full_user else {
        panic!("expected UserFull::Full")
    };
    assert_eq!(inner.id, me);
    assert_eq!(full.users.len(), 1);
    // The shared test group is reported as a common chat.
    assert_eq!(inner.common_chats_count, 1);
}

#[test]
fn contacts_resolve_username_finds_peer() {
    let mut fx = setup();
    let f = tl::functions::contacts::ResolveUsername {
        username: "peeruser".into(),
        referer: None,
    };
    let out: tl::enums::contacts::ResolvedPeer = call(&mut fx.ctx, &f);
    let tl::enums::contacts::ResolvedPeer::Peer(res) = out else {
        panic!("expected ResolvedPeer::Peer")
    };
    assert!(matches!(res.peer, tl::enums::Peer::User(_)));
    assert_eq!(res.users.len(), 1);
}

#[test]
fn contacts_resolve_username_rejects_unknown() {
    let mut fx = setup();
    let f = tl::functions::contacts::ResolveUsername {
        username: "nobody".into(),
        referer: None,
    };
    let err = dispatch(&mut fx.ctx, &f.to_bytes()).unwrap_err();
    let rpc = telegram_server::rpc::as_rpc_error(&err).expect("expected an rpc_error");
    assert_eq!(rpc.code, 400);
    assert_eq!(rpc.message, "USERNAME_NOT_OCCUPIED");
}

#[test]
fn contacts_search_matches_name() {
    let mut fx = setup();
    let f = tl::functions::contacts::Search {
        broadcasts: false,
        bots: false,
        q: "peer".into(),
        limit: 10,
    };
    let out: tl::enums::contacts::Found = call(&mut fx.ctx, &f);
    let tl::enums::contacts::Found::Found(found) = out else {
        panic!("expected contacts::Found::Found")
    };
    assert_eq!(found.results.len(), 1);
}

#[test]
fn messages_get_peer_dialogs_reports_dialogs() {
    let mut fx = setup();
    let peer_id = fx
        .ctx
        .store
        .get_user_by_username("peeruser")
        .unwrap()
        .unwrap()
        .id;
    let f = tl::functions::messages::GetPeerDialogs {
        peers: vec![tl::enums::InputDialogPeer::Peer(
            tl::types::InputDialogPeer {
                peer: tl::enums::InputPeer::User(tl::types::InputPeerUser {
                    user_id: peer_id,
                    access_hash: 0,
                }),
            },
        )],
    };
    let out: tl::enums::messages::PeerDialogs = call(&mut fx.ctx, &f);
    let tl::enums::messages::PeerDialogs::Dialogs(d) = out else {
        panic!("expected PeerDialogs::Dialogs")
    };
    // No messages were sent, so no dialog rows exist yet; the reply must
    // still be a well-formed PeerDialogs carrying the update state.
    let tl::enums::updates::State::State(state) = d.state else {
        panic!("expected updates::State::State")
    };
    assert!(state.pts >= 1);
}

#[test]
fn messages_send_then_search_and_get_messages() {
    let mut fx = setup();
    let peer_id = fx
        .ctx
        .store
        .get_user_by_username("peeruser")
        .unwrap()
        .unwrap()
        .id;
    let send = tl::functions::messages::SendMessage {
        no_webpage: false,
        silent: false,
        background: false,
        clear_draft: false,
        noforwards: false,
        update_stickersets_order: false,
        invert_media: false,
        allow_paid_floodskip: false,
        peer: tl::enums::InputPeer::User(tl::types::InputPeerUser {
            user_id: peer_id,
            access_hash: 0,
        }),
        reply_to: None,
        random_id: 7,
        message: "hello searchable world".into(),
        reply_markup: None,
        entities: None,
        schedule_date: None,
        send_as: None,
        quick_reply_shortcut: None,
        effect: None,
        allow_paid_stars: None,
        suggested_post: None,
        schedule_repeat_period: None,
        rich_message: None,
    };
    let _: tl::enums::Updates = call(&mut fx.ctx, &send);

    let search = tl::functions::messages::Search {
        peer: tl::enums::InputPeer::User(tl::types::InputPeerUser {
            user_id: peer_id,
            access_hash: 0,
        }),
        q: "searchable".into(),
        from_id: None,
        saved_peer_id: None,
        saved_reaction: None,
        top_msg_id: None,
        filter: tl::enums::MessagesFilter::InputMessagesFilterEmpty,
        min_date: 0,
        max_date: 0,
        offset_id: 0,
        add_offset: 0,
        limit: 20,
        max_id: 0,
        min_id: 0,
        hash: 0,
    };
    let out: tl::enums::messages::Messages = call(&mut fx.ctx, &search);
    let tl::enums::messages::Messages::Messages(m) = out else {
        panic!("expected messages::Messages::Messages")
    };
    assert_eq!(m.messages.len(), 1, "search should find the sent message");

    // A search for text that was never sent must come back empty.
    let miss = tl::functions::messages::Search {
        q: "definitely-not-present".into(),
        ..search
    };
    let out: tl::enums::messages::Messages = call(&mut fx.ctx, &miss);
    let tl::enums::messages::Messages::Messages(m) = out else {
        panic!("expected messages::Messages::Messages")
    };
    assert!(m.messages.is_empty());
}

#[test]
fn messages_get_full_chat_lists_participants() {
    let mut fx = setup();
    let peer_id = fx
        .ctx
        .store
        .get_user_by_username("peeruser")
        .unwrap()
        .unwrap()
        .id;
    let chat = fx
        .ctx
        .store
        .chat_members_shared(fx.ctx.user_id, peer_id)
        .unwrap()[0]
        .clone();
    let f = tl::functions::messages::GetFullChat { chat_id: chat.id };
    let out: tl::enums::messages::ChatFull = call(&mut fx.ctx, &f);
    let tl::enums::messages::ChatFull::Full(full) = out else {
        panic!("expected ChatFull::Full")
    };
    let tl::enums::ChatFull::Full(inner) = full.full_chat else {
        panic!("expected ChatFull::Full")
    };
    let tl::enums::ChatParticipants::Participants(p) = inner.participants else {
        panic!("expected ChatParticipants::Participants")
    };
    assert_eq!(p.participants.len(), 2);
    assert_eq!(full.users.len(), 2);
}

#[test]
fn account_notify_settings_round_trip() {
    let mut fx = setup();
    let settings =
        tl::enums::InputPeerNotifySettings::Settings(tl::types::InputPeerNotifySettings {
            show_previews: Some(false),
            silent: Some(true),
            mute_until: Some(3600),
            sound: None,
            stories_muted: Some(true),
            stories_hide_sender: None,
            stories_sound: None,
        });
    let peer = tl::enums::InputNotifyPeer::InputNotifyUsers;
    let upd = tl::functions::account::UpdateNotifySettings {
        peer: peer.clone(),
        settings,
    };
    let ok: bool = call(&mut fx.ctx, &upd);
    assert!(ok);

    let get = tl::functions::account::GetNotifySettings { peer };
    let out: tl::enums::PeerNotifySettings = call(&mut fx.ctx, &get);
    let tl::enums::PeerNotifySettings::Settings(s) = out else {
        panic!("expected PeerNotifySettings::Settings")
    };
    assert_eq!(s.show_previews, Some(false));
    assert_eq!(s.silent, Some(true));
    assert_eq!(s.mute_until, Some(3600));
    assert_eq!(s.stories_muted, Some(true));
}

#[test]
fn account_authorizations_are_listed() {
    let mut fx = setup();
    fx.ctx
        .store
        .save_session("sess-1", fx.ctx.user_id, 42, "Desktop", "Linux", 1, 3600)
        .unwrap();
    let f = tl::functions::account::GetAuthorizations {};
    let out: tl::enums::account::Authorizations = call(&mut fx.ctx, &f);
    let tl::enums::account::Authorizations::Authorizations(a) = out else {
        panic!("expected account::Authorizations::Authorizations")
    };
    assert_eq!(a.authorizations.len(), 1);
}

#[test]
fn help_app_config_and_support_respond() {
    let mut fx = setup();
    let f = tl::functions::help::GetAppConfig { hash: 0 };
    let out: tl::enums::help::AppConfig = call(&mut fx.ctx, &f);
    assert!(matches!(out, tl::enums::help::AppConfig::Config(_)));

    let f = tl::functions::help::GetSupport {};
    let out: tl::enums::help::Support = call(&mut fx.ctx, &f);
    let tl::enums::help::Support::Support(s) = out else {
        panic!("expected help::Support::Support")
    };
    assert!(matches!(s.user, tl::enums::User::User(_)));
}

#[test]
fn upload_get_file_serves_stored_blob() {
    let mut fx = setup();
    fx.ctx
        .store
        .put_blob("blob:99", "note.txt", "text/plain", b"payload-bytes")
        .unwrap();
    let f = tl::functions::upload::GetFile {
        precise: false,
        cdn_supported: false,
        location: tl::enums::InputFileLocation::Location(tl::types::InputFileLocation {
            volume_id: 99,
            local_id: 0,
            secret: 0,
            file_reference: Vec::new(),
        }),
        offset: 0,
        limit: 1024,
    };
    let out: tl::enums::upload::File = call(&mut fx.ctx, &f);
    let tl::enums::upload::File::File(file) = out else {
        panic!("expected upload::File::File")
    };
    assert_eq!(file.bytes, b"payload-bytes");

    // A range request returns just the requested slice.
    let f = tl::functions::upload::GetFile {
        offset: 8,
        limit: 5,
        ..f
    };
    let out: tl::enums::upload::File = call(&mut fx.ctx, &f);
    let tl::enums::upload::File::File(file) = out else {
        panic!("expected upload::File::File")
    };
    assert_eq!(file.bytes, b"bytes");
}

#[test]
fn excluded_namespaces_return_rpc_errors() {
    let mut fx = setup();
    // payments.getPaymentForm is deliberately not stubbed.
    let body = 0x37148dbbu32.to_le_bytes();
    let err = dispatch(&mut fx.ctx, &body).unwrap_err();
    let rpc = telegram_server::rpc::as_rpc_error(&err).expect("expected an rpc_error");
    assert_eq!(rpc.code, 400);
    assert!(rpc.message.contains("unsupported"), "{}", rpc.message);
}

#[test]
fn unknown_ctor_returns_rpc_error() {
    let mut fx = setup();
    let body = 0xdeadbeefu32.to_le_bytes();
    let err = dispatch(&mut fx.ctx, &body).unwrap_err();
    assert!(telegram_server::rpc::as_rpc_error(&err).is_some());
}

/// tweb seals `auth.bindTempAuthKey`'s inner message with the *permanent* key
/// under the MTProto 1.0 (SHA1) scheme, prefixed by a random 128-bit value
/// instead of salt/session. Reproduced here byte for byte from
/// `src/tests/pfsNetworker.test.ts`, because a 2.0-only decrypt rejects it
/// with TEMP_AUTH_KEY_INVALID and the login never leaves "Please Wait".
#[test]
fn auth_bind_temp_auth_key_accepts_legacy_permanent_key_envelope() {
    use telegram_server::crypto::{auth_key_id, ige_encrypt, msg_key_v1, msg_key_v1_to_aes_key_iv};

    let mut fx = setup();
    let perm_key = [0x77u8; 256];
    let temp_key = [0x31u8; 256];
    let perm_id = auth_key_id(&perm_key);
    let temp_id = auth_key_id(&temp_key);
    fx.ctx.store.save_auth_key(perm_id, &perm_key).unwrap();
    fx.ctx.store.save_auth_key(temp_id, &temp_key).unwrap();
    // The binding call travels over the *temporary* key, so that is the key the
    // transport reported for this request.
    fx.ctx.auth_key_id = temp_id;

    let nonce = 0x1122_3344_5566_7788i64;
    let temp_session_id = 0x0a0b_0c0d_0e0f_1011i64;
    let msg_id = 0x7fff_0000_0000_0004i64;
    let inner = tl::types::BindAuthKeyInner {
        nonce,
        temp_auth_key_id: temp_id,
        perm_auth_key_id: perm_id,
        temp_session_id,
        expires_at: 1_700_000_000,
    }
    .to_bytes();

    // tweb writes the `bind_auth_key_inner#75a3f765` constructor itself, and
    // the 1.0 body carries a random 16-byte prefix in place of salt/session.
    let mut body = 0x75a3_f765u32.to_le_bytes().to_vec();
    body.extend_from_slice(&inner);
    let mut plain = vec![0xABu8; 16];
    plain.extend_from_slice(&msg_id.to_le_bytes());
    plain.extend_from_slice(&0i32.to_le_bytes());
    plain.extend_from_slice(&(body.len() as i32).to_le_bytes());
    plain.extend_from_slice(&body);
    let unpadded_len = plain.len();
    while plain.len() % 16 != 0 {
        plain.push(0xCC);
    }

    let msg_key = msg_key_v1(&plain[..unpadded_len]);
    // Client -> server: x = 0, i.e. `from_client = true`.
    let (key, iv) = msg_key_v1_to_aes_key_iv(&perm_key, &msg_key, true);
    let mut encrypted = plain.clone();
    ige_encrypt(&mut encrypted, &key, &iv);
    let mut envelope = perm_id.to_le_bytes().to_vec();
    envelope.extend_from_slice(&msg_key);
    envelope.extend_from_slice(&encrypted);

    let request = tl::functions::auth::BindTempAuthKey {
        perm_auth_key_id: perm_id,
        nonce,
        expires_at: 1_700_000_000,
        encrypted_message: envelope,
    };
    let status: bool = call(&mut fx.ctx, &request);
    assert!(status);
}

// --------------------------------------------------------------------------
// Constructor-id regressions.
//
// Each arm of `dispatch_inner` is keyed by a raw constructor id, so a wrong
// id silently routes a method to another method's handler (or to the generic
// compatibility fallback). These tests send the id declared by
// `grammers-tl-types` and assert the arm that actually handles it, which is
// what caught messages.getFullChat / createChat / importContacts /
// langpack.getDifference / langpack.getLanguage / setTyping / resetSaved
// pointing at ids that belonged to other methods.
// --------------------------------------------------------------------------

#[test]
fn typed_arms_are_reachable_by_declared_ctor() {
    use tl::Identifiable;
    let mut fx = setup();

    // messages.getFullChat#aeb00b34 -- must reach the chat handler, not the
    // generic fallback (a stale 0xa6f47c87 used to route it elsewhere).
    assert_eq!(
        <tl::functions::messages::GetFullChat as Identifiable>::CONSTRUCTOR_ID,
        0xaeb00b34
    );
    // A missing chat is a clean RPC error...
    let f = tl::functions::messages::GetFullChat { chat_id: 1 };
    let err = dispatch(&mut fx.ctx, &f.to_bytes()).unwrap_err();
    let rpc = telegram_server::rpc::as_rpc_error(&err).expect("expected an rpc_error");
    assert_eq!(rpc.message, "CHAT_ID_INVALID");

    // ...and a real chat reaches the chat handler with a parseable reply.
    let peer_id = fx
        .ctx
        .store
        .get_user_by_username("peeruser")
        .unwrap()
        .unwrap()
        .id;
    let chat = fx
        .ctx
        .store
        .chat_members_shared(fx.ctx.user_id, peer_id)
        .unwrap()[0]
        .clone();
    let f = tl::functions::messages::GetFullChat { chat_id: chat.id };
    let body = dispatch(&mut fx.ctx, &f.to_bytes()).expect("getFullChat must dispatch");
    let mut cur = tl::Cursor::from_slice(&body);
    tl::enums::messages::ChatFull::deserialize(&mut cur).expect("getFullChat reply must parse");
}

#[test]
fn contacts_import_contacts_ctor_is_reachable() {
    use tl::Identifiable;
    let mut fx = setup();
    let f = tl::functions::contacts::ImportContacts {
        contacts: Vec::new(),
    };
    assert_eq!(
        <tl::functions::contacts::ImportContacts as Identifiable>::CONSTRUCTOR_ID,
        0x2c800be5
    );
    let body = dispatch(&mut fx.ctx, &f.to_bytes()).expect("importContacts must dispatch");
    let mut cur = tl::Cursor::from_slice(&body);
    tl::enums::contacts::ImportedContacts::deserialize(&mut cur)
        .expect("importContacts reply must parse");
}

#[test]
fn langpack_ctors_are_distinct() {
    use tl::Identifiable;
    assert_eq!(
        <tl::functions::langpack::GetDifference as Identifiable>::CONSTRUCTOR_ID,
        0xcd984aa5
    );
    assert_eq!(
        <tl::functions::langpack::GetLanguage as Identifiable>::CONSTRUCTOR_ID,
        0x6a596502
    );
    let mut fx = setup();
    // getLanguage for an unknown pack must be a clean RPC error, not a
    // mis-parsed success produced by the getDifference arm.
    let f = tl::functions::langpack::GetLanguage {
        lang_pack: "android".into(),
        lang_code: "en".into(),
    };
    let err = dispatch(&mut fx.ctx, &f.to_bytes()).unwrap_err();
    let rpc = telegram_server::rpc::as_rpc_error(&err).expect("expected an rpc_error");
    assert_eq!(rpc.message, "LANG_PACK_LANGUAGE_INVALID");
}

#[test]
fn messages_get_chats_and_read_history_dispatch() {
    use tl::Identifiable;
    let mut fx = setup();
    assert_eq!(
        <tl::functions::messages::GetChats as Identifiable>::CONSTRUCTOR_ID,
        0x49e9528f
    );
    assert_eq!(
        <tl::functions::messages::ReadHistory as Identifiable>::CONSTRUCTOR_ID,
        0x0e306d3a
    );
    let peer_id = fx
        .ctx
        .store
        .get_user_by_username("peeruser")
        .unwrap()
        .unwrap()
        .id;
    let f = tl::functions::messages::ReadHistory {
        peer: tl::enums::InputPeer::User(tl::types::InputPeerUser {
            user_id: peer_id,
            access_hash: 0,
        }),
        max_id: 0,
    };
    let out: tl::enums::messages::AffectedHistory = call(&mut fx.ctx, &f);
    let tl::enums::messages::AffectedHistory::History(h) = out else {
        panic!("expected AffectedHistory::History")
    };
    assert!(h.pts >= 1);
}

#[test]
fn auth_reset_authorizations_clears_other_sessions() {
    let mut fx = setup();
    fx.ctx
        .store
        .save_session("sess-keep", fx.ctx.user_id, 42, "Desktop", "Linux", 1, 3600)
        .unwrap();
    fx.ctx
        .store
        .save_session("sess-drop", fx.ctx.user_id, 77, "Phone", "Android", 1, 3600)
        .unwrap();
    assert_eq!(fx.ctx.store.count_sessions(fx.ctx.user_id).unwrap(), 2);
    let f = tl::functions::auth::ResetAuthorizations {};
    let ok: bool = call(&mut fx.ctx, &f);
    assert!(ok);
    // The current session is kept, the other one is dropped.
    assert_eq!(fx.ctx.store.count_sessions(fx.ctx.user_id).unwrap(), 1);
}

#[test]
fn account_update_profile_writes_through() {
    let mut fx = setup();
    let f = tl::functions::account::UpdateProfile {
        first_name: Some("Renamed".into()),
        last_name: None,
        about: Some("bio".into()),
    };
    let _: tl::enums::User = call(&mut fx.ctx, &f);
    let me = fx.ctx.store.get_user(fx.ctx.user_id).unwrap().unwrap();
    assert_eq!(me.first_name, "Renamed");
    assert_eq!(me.about, "bio");
}

// ---------------------------------------------------------------------------
// Regression tests for the wire-level defects found while driving a real client
// (Telethon, layer 224) against the server. Each of these failed before.
// ---------------------------------------------------------------------------

/// Every reply that carries messages must come back stamped with the
/// constructor id of the layer the client announced. `Message` is one of only
/// two objects whose id changed between layer 224 and 227.
#[test]
fn message_replies_are_downgraded_for_older_layers() {
    let mut fx = setup();
    fx.ctx.layer = 224;
    let _: tl::enums::Updates = call(
        &mut fx.ctx,
        &tl::functions::messages::SendMessage {
            no_webpage: false,
            silent: false,
            background: false,
            clear_draft: false,
            noforwards: false,
            update_stickersets_order: false,
            invert_media: false,
            allow_paid_floodskip: false,
            peer: tl::enums::InputPeer::PeerSelf,
            reply_to: None,
            message: "hello".into(),
            random_id: 1,
            reply_markup: None,
            entities: None,
            schedule_date: None,
            schedule_repeat_period: None,
            send_as: None,
            quick_reply_shortcut: None,
            effect: None,
            allow_paid_stars: None,
            suggested_post: None,
            rich_message: None,
        },
    );

    let body = dispatch(
        &mut fx.ctx,
        &tl::functions::messages::GetHistory {
            peer: tl::enums::InputPeer::PeerSelf,
            offset_id: 0,
            offset_date: 0,
            add_offset: 0,
            limit: 10,
            max_id: 0,
            min_id: 0,
            hash: 0,
        }
        .to_bytes(),
    )
    .unwrap();

    // The reply must not contain a layer-227 `message#7600b9d3` any more.
    let needle = 0x7600b9d3u32.to_le_bytes();
    assert!(
        !body.windows(4).any(|w| w == needle),
        "reply still carries layer-227 Message constructors"
    );
    let legacy = 0x3ae56482u32.to_le_bytes();
    assert!(
        body.windows(4).any(|w| w == legacy),
        "reply does not carry layer-224 Message constructors"
    );
    // The envelope must still be a valid `messages.messages` holding exactly
    // one message. grammers is generated from layer 227, so the downgraded id
    // cannot be parsed directly; restore it and parse, which proves the only
    // change was the constructor id.
    let legacy = 0x3ae56482u32.to_le_bytes();
    let mut restored = body.clone();
    let mut i = 0;
    while i + 4 <= restored.len() {
        if restored[i..i + 4] == legacy {
            restored[i..i + 4].copy_from_slice(&0x7600b9d3u32.to_le_bytes());
        }
        i += 1;
    }
    let parsed = tl::enums::messages::Messages::deserialize(&mut tl::Cursor::from_slice(&restored))
        .expect("reply must be a messages.Messages once the id is restored");
    let msgs = match parsed {
        tl::enums::messages::Messages::Messages(m) => m.messages,
        other => panic!("unexpected reply {other:?}"),
    };
    assert_eq!(msgs.len(), 1);
}

/// The peer of a self-dialog must be the acting user, not user 0.
#[test]
fn self_dialog_messages_use_the_real_peer() {
    let mut fx = setup();
    let self_id = fx.ctx.user_id;
    let _: tl::enums::Updates = call(
        &mut fx.ctx,
        &tl::functions::messages::SendMessage {
            no_webpage: false,
            silent: false,
            background: false,
            clear_draft: false,
            noforwards: false,
            update_stickersets_order: false,
            invert_media: false,
            allow_paid_floodskip: false,
            peer: tl::enums::InputPeer::PeerSelf,
            reply_to: None,
            message: "note to self".into(),
            random_id: 2,
            reply_markup: None,
            entities: None,
            schedule_date: None,
            schedule_repeat_period: None,
            send_as: None,
            quick_reply_shortcut: None,
            effect: None,
            allow_paid_stars: None,
            suggested_post: None,
            rich_message: None,
        },
    );
    let hist: tl::enums::messages::Messages = call(
        &mut fx.ctx,
        &tl::functions::messages::GetHistory {
            peer: tl::enums::InputPeer::PeerSelf,
            offset_id: 0,
            offset_date: 0,
            add_offset: 0,
            limit: 10,
            max_id: 0,
            min_id: 0,
            hash: 0,
        },
    );
    let msgs = match hist {
        tl::enums::messages::Messages::Messages(m) => m.messages,
        other => panic!("unexpected reply {other:?}"),
    };
    let msg = match &msgs[0] {
        tl::enums::Message::Message(m) => m,
        other => panic!("unexpected message {other:?}"),
    };
    match &msg.peer_id {
        tl::enums::Peer::User(u) => assert_eq!(u.user_id, self_id),
        other => panic!("expected PeerUser({self_id}), got {other:?}"),
    }
}

/// A layer-224 `messages.sendMessage#545cd15a` body must be accepted, even
/// though this server is generated from layer 227.
#[test]
fn legacy_layer_send_message_body_is_accepted() {
    let mut fx = setup();
    // Hand-serialize `messages.sendMessage#545cd15a flags:# peer:InputPeer
    // message:string random_id:long` with no optional fields set.
    let mut body = Vec::new();
    body.extend_from_slice(&0x545cd15au32.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // flags
    body.extend_from_slice(&tl::enums::InputPeer::PeerSelf.to_bytes());
    let text = b"legacy layer message";
    body.push(text.len() as u8);
    body.extend_from_slice(text);
    while body.len() % 4 != 0 {
        body.push(0);
    }
    body.extend_from_slice(&7i64.to_le_bytes()); // random_id

    let reply = dispatch(&mut fx.ctx, &body).expect("legacy sendMessage must be accepted");
    let parsed = tl::enums::Updates::deserialize(&mut tl::Cursor::from_slice(&reply))
        .expect("legacy sendMessage reply must parse");
    match parsed {
        tl::enums::Updates::UpdateShortSentMessage(_) => {}
        other => panic!("unexpected reply {other:?}"),
    }
}

/// Unauthenticated calls must be rejected with 401 rather than returning empty
/// result lists, which would leave clients stuck in their init sequence.
#[test]
fn unauthenticated_calls_are_rejected() {
    let mut fx = setup();
    fx.ctx.user_id = 0;
    let err = dispatch(
        &mut fx.ctx,
        &tl::functions::messages::GetDialogs {
            exclude_pinned: false,
            folder_id: None,
            offset_date: 0,
            offset_id: 0,
            offset_peer: tl::enums::InputPeer::Empty,
            limit: 10,
            hash: 0,
        }
        .to_bytes(),
    )
    .expect_err("unauthenticated getDialogs must fail");
    let rpc = telegram_server::rpc::as_rpc_error(&err).expect("expected an rpc_error");
    assert_eq!(rpc.code, 401);
}

/// `contacts.search` changed shape between layers; both encodings must work.
#[test]
fn both_contacts_search_encodings_are_accepted() {
    let mut fx = setup();
    // Current layer: flags:# q:string limit:int.
    let mut current = Vec::new();
    current.extend_from_slice(&0x05f58d0fu32.to_le_bytes());
    current.extend_from_slice(&0u32.to_le_bytes());
    let q = b"admin";
    current.push(q.len() as u8);
    current.extend_from_slice(q);
    while current.len() % 4 != 0 {
        current.push(0);
    }
    current.extend_from_slice(&10i32.to_le_bytes());
    let reply = dispatch(&mut fx.ctx, &current).expect("current contacts.search must be accepted");
    let _: tl::enums::contacts::Found =
        tl::enums::contacts::Found::deserialize(&mut tl::Cursor::from_slice(&reply)).unwrap();

    // Legacy layer: q:string limit:int.
    let mut legacy = Vec::new();
    legacy.extend_from_slice(&0x11f812d8u32.to_le_bytes());
    legacy.push(q.len() as u8);
    legacy.extend_from_slice(q);
    while legacy.len() % 4 != 0 {
        legacy.push(0);
    }
    legacy.extend_from_slice(&10i32.to_le_bytes());
    let reply = dispatch(&mut fx.ctx, &legacy).expect("legacy contacts.search must be accepted");
    let found =
        tl::enums::contacts::Found::deserialize(&mut tl::Cursor::from_slice(&reply)).unwrap();
    match found {
        tl::enums::contacts::Found::Found(f) => assert!(!f.results.is_empty()),
    }
}

/// `+1 555 000 0001` and `15550000001` must resolve to the same account.
#[test]
fn phone_numbers_are_normalized() {
    let fx = setup();
    let with_plus = fx.ctx.store.get_user_by_phone("+1 555 000 0001").unwrap();
    let plain = fx.ctx.store.get_user_by_phone("15550000001").unwrap();
    assert!(with_plus.is_some(), "normalized lookup failed");
    assert_eq!(with_plus.unwrap().id, plain.unwrap().id);
}
