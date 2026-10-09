use axum::body::Body;
use axum::http::Request;
use grammers_tl_types as tl;
use grammers_tl_types::{Deserializable, Serializable};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;
use telegram_server::botapi::router;
use telegram_server::botapi::AppState;
use telegram_server::config::Config;
use telegram_server::crypto::auth_key_id;
use telegram_server::http_mtproto::{self, HttpMtProtoState};
use telegram_server::mtproto::{EncryptedEnvelope, PlainMessage, RsaKeyPair};
use telegram_server::store::Store;
use tower::ServiceExt;

fn shared_rsa() -> &'static RsaKeyPair {
    static RSA: OnceLock<RsaKeyPair> = OnceLock::new();
    RSA.get_or_init(RsaKeyPair::generate)
}

fn fixture() -> (AppState, PathBuf) {
    let dir = std::env::temp_dir().join(format!("tgsrv-http-handshake-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::open(&dir.join("test.db")).unwrap();
    let cfg = Arc::new(Config::new(
        2,
        Some("127.0.0.1".into()),
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
    let state = AppState {
        store,
        cfg,
        rsa: shared_rsa().clone(),
        http_mtproto: Arc::new(HttpMtProtoState::new()),
    };
    (state, dir)
}

const HELP_GET_CONFIG: [u8; 4] = 0xc4f9_186bu32.to_le_bytes();
const PING: [u8; 4] = 0x7abe_77ecu32.to_le_bytes();
const HTTP_WAIT: [u8; 4] = 0x9299_359fu32.to_le_bytes();
const RPC_RESULT: u32 = 0xf35c_6d01;
const MSG_CONTAINER: u32 = 0x73f1_f8dc;
const NEW_SESSION_CREATED: u32 = 0x9ec2_0908;

fn container(messages: &[(i64, &[u8])]) -> Vec<u8> {
    let mut body = MSG_CONTAINER.to_le_bytes().to_vec();
    body.extend_from_slice(&(messages.len() as i32).to_le_bytes());
    for (msg_id, payload) in messages {
        body.extend_from_slice(&msg_id.to_le_bytes());
        body.extend_from_slice(&1i32.to_le_bytes());
        body.extend_from_slice(&(payload.len() as i32).to_le_bytes());
        body.extend_from_slice(payload);
    }
    body
}

fn encrypted_request(key: &[u8; 256], session_id: i64, msg_id: i64, body: &[u8]) -> Vec<u8> {
    EncryptedEnvelope {
        salt: 0,
        session_id,
        msg_id,
        seq_no: 1,
        body: body.to_vec(),
    }
    .encode(key)
}

fn setup_auth_key(state: &AppState, key: &[u8; 256]) {
    state.store.save_auth_key(auth_key_id(key), key).unwrap();
}

fn read_i32(body: &[u8], off: &mut usize) -> i32 {
    let v = i32::from_le_bytes(body[*off..*off + 4].try_into().unwrap());
    *off += 4;
    v
}

fn read_i64(body: &[u8], off: &mut usize) -> i64 {
    let v = i64::from_le_bytes(body[*off..*off + 8].try_into().unwrap());
    *off += 8;
    v
}

fn ctor(body: &[u8]) -> u32 {
    u32::from_le_bytes(body[0..4].try_into().unwrap())
}

fn req_msg_id(reply: &[u8]) -> i64 {
    assert_eq!(ctor(reply), RPC_RESULT, "reply is not an rpc_result");
    i64::from_le_bytes(reply[4..12].try_into().unwrap())
}

#[test]
fn http_req_pq_echoes_nonce() {
    let (state, _dir) = fixture();
    let nonce = [0x5Au8; 16];
    let request = grammers_tl_types::functions::ReqPqMulti { nonce };
    let plain = PlainMessage {
        msg_id: 0x1000,
        body: request.to_bytes(),
    };
    let response = http_mtproto::process(&state, &plain.encode()).unwrap();
    let response = PlainMessage::decode_response(&response).unwrap();
    let ctor = u32::from_le_bytes(response.body[0..4].try_into().unwrap());
    assert_eq!(ctor, 0x05162463);
    let res_pq = grammers_tl_types::types::ResPq::from_bytes(&response.body[4..]).unwrap();
    assert_eq!(res_pq.nonce, nonce);
}

/// The login flow batches `auth.sendCode` with an `http_wait`. Answering the
/// container id leaves the client's real request deferred pending forever, so
/// the reply must name the inner message instead.
#[test]
fn http_container_rpc_results_use_inner_message_ids() {
    let (state, _dir) = fixture();
    let key = [0x5Au8; 256];
    setup_auth_key(&state, &key);

    let config_msg_id = 0x1122_3344_5566_7700i64;
    let body = container(&[
        (config_msg_id, &HELP_GET_CONFIG),
        (config_msg_id + 4, &HTTP_WAIT),
    ]);
    let request = encrypted_request(&key, 7, 0x1000, &body);
    let response = http_mtproto::process(&state, &request).unwrap();
    let envelope = EncryptedEnvelope::decode(&response, &key).unwrap();

    assert_eq!(envelope.seq_no, 1);
    assert_eq!(req_msg_id(&envelope.body), config_msg_id);
    // The `http_wait` must not turn into a second, uncorrelatable reply.
    tl::enums::Config::from_bytes(&envelope.body[12..]).expect("getConfig result must parse");
}

/// tweb treats an empty HTTP keepalive response as a transport failure. A
/// standalone `http_wait` therefore needs a service message it can process,
/// while container members remain skipped to avoid duplicate keepalives.
#[test]
fn http_wait_single_request_receives_new_session() {
    let (state, _dir) = fixture();
    let key = [0x33u8; 256];
    setup_auth_key(&state, &key);

    let request_msg_id = 0x3000i64;
    let request = encrypted_request(&key, 11, request_msg_id, &HTTP_WAIT);
    let response = http_mtproto::process(&state, &request).unwrap();
    let envelope = EncryptedEnvelope::decode(&response, &key).unwrap();

    assert_eq!(envelope.seq_no, 1);
    assert_eq!(
        u32::from_le_bytes(envelope.body[0..4].try_into().unwrap()),
        NEW_SESSION_CREATED
    );
    assert_eq!(
        i64::from_le_bytes(envelope.body[4..12].try_into().unwrap()),
        request_msg_id
    );
}

#[test]
fn http_wait_delivers_a_queued_message_update_once() {
    let (state, _dir) = fixture();
    let key = [0x44u8; 256];
    setup_auth_key(&state, &key);
    let user = state
        .store
        .create_user("15550000101", "Queued", "Update", "queued", false, false)
        .unwrap();
    let sender = state
        .store
        .create_user("15550000102", "Sender", "User", "sender", false, false)
        .unwrap();
    state
        .store
        .save_session(
            "sess-http-update",
            user.id,
            auth_key_id(&key),
            "",
            "",
            1,
            3600,
        )
        .unwrap();
    let message_id = state
        .store
        .insert_message(
            "user", sender.id, sender.id, None, "queued", "", None, None, 7,
        )
        .unwrap();
    state
        .store
        .push_update(
            user.id,
            "message",
            &serde_json::json!({
                "dialog_type": "user",
                "dialog_id": sender.id,
                "message_id": message_id
            })
            .to_string(),
        )
        .unwrap();

    let request = encrypted_request(&key, 21, 0x4100, &HTTP_WAIT);
    let response = http_mtproto::process(&state, &request).unwrap();
    let envelope = EncryptedEnvelope::decode(&response, &key).unwrap();
    assert_eq!(ctor(&envelope.body), MSG_CONTAINER);
    let count = i32::from_le_bytes(envelope.body[4..8].try_into().unwrap());
    assert_eq!(count, 2);
    let mut offset = 8;
    let mut constructors = Vec::new();
    let mut updates_body = Vec::new();
    for _ in 0..count {
        let _msg_id = read_i64(&envelope.body, &mut offset);
        let _seq_no = read_i32(&envelope.body, &mut offset);
        let length = read_i32(&envelope.body, &mut offset) as usize;
        let body = &envelope.body[offset..offset + length];
        offset += length;
        constructors.push(ctor(body));
        if ctor(body) != NEW_SESSION_CREATED {
            updates_body = body.to_vec();
        }
    }
    assert!(constructors.contains(&NEW_SESSION_CREATED));
    assert!(matches!(
        tl::enums::Updates::from_bytes(&updates_body).unwrap(),
        tl::enums::Updates::UpdateShort(_)
    ));
    assert!(state
        .store
        .pending_updates(user.id, auth_key_id(&key), 10)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn http_wait_long_poll_waits_before_returning() {
    let (state, _dir) = fixture();
    let key = [0x55u8; 256];
    setup_auth_key(&state, &key);
    let mut body = HTTP_WAIT.to_vec();
    body.extend_from_slice(&0i32.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes());
    body.extend_from_slice(&60i32.to_le_bytes());
    let request_body = encrypted_request(&key, 31, 0x4200, &body);
    let app = router(state.clone());
    let request = Request::builder()
        .method("POST")
        .uri("/apiw1")
        .body(Body::from(request_body))
        .unwrap();
    let started = Instant::now();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(started.elapsed().as_millis() >= 40);
}

/// Two answered requests must be batched into a real `msg_container`, with
/// each inner entry carrying its own reply id and sequence number.
#[test]
fn http_container_replies_are_batched() {
    let (state, _dir) = fixture();
    let key = [0x11u8; 256];
    setup_auth_key(&state, &key);

    let config_msg_id = 0x2200i64;
    let ping_msg_id = 0x2210i64;
    let ping_body: Vec<u8> = PING.iter().copied().chain(42i64.to_le_bytes()).collect();
    let body = container(&[(config_msg_id, &HELP_GET_CONFIG), (ping_msg_id, &ping_body)]);
    let request = encrypted_request(&key, 9, 0x2000, &body);
    let response = http_mtproto::process(&state, &request).unwrap();
    let envelope = EncryptedEnvelope::decode(&response, &key).unwrap();

    assert_eq!(envelope.seq_no, 4, "a container is not content-related");
    assert_eq!(ctor(&envelope.body), MSG_CONTAINER);
    let mut off = 8;
    let count = i32::from_le_bytes(envelope.body[4..8].try_into().unwrap());
    assert_eq!(count, 2);
    let mut expected_req_ids = vec![config_msg_id, ping_msg_id];
    let mut expected_seq_nos = vec![1, 3];
    for _ in 0..count {
        let _msg_id = read_i64(&envelope.body, &mut off);
        let seq_no = read_i32(&envelope.body, &mut off);
        let len = read_i32(&envelope.body, &mut off) as usize;
        let reply = &envelope.body[off..off + len];
        off += len;
        assert_eq!(seq_no, expected_seq_nos.remove(0));
        assert_eq!(req_msg_id(reply), expected_req_ids.remove(0));
    }
    assert_eq!(off, envelope.body.len());
}

/// Sequence numbers must advance per session, otherwise a client that tracks
/// server `seq_no` rejects the second reply as "msg_seqno too low".
#[test]
fn http_sequence_numbers_advance_per_session() {
    let (state, _dir) = fixture();
    let key = [0x22u8; 256];
    setup_auth_key(&state, &key);

    let first = container(&[(0x3300, &HELP_GET_CONFIG)]);
    let second = container(&[(0x3310, &HELP_GET_CONFIG)]);
    let other_session = container(&[(0x3320, &HELP_GET_CONFIG)]);

    let one = http_mtproto::process(&state, &encrypted_request(&key, 11, 0x3000, &first)).unwrap();
    let two = http_mtproto::process(&state, &encrypted_request(&key, 11, 0x3010, &second)).unwrap();
    let fresh = http_mtproto::process(&state, &encrypted_request(&key, 12, 0x3020, &other_session))
        .unwrap();

    assert_eq!(EncryptedEnvelope::decode(&one, &key).unwrap().seq_no, 1);
    assert_eq!(EncryptedEnvelope::decode(&two, &key).unwrap().seq_no, 3);
    assert_eq!(EncryptedEnvelope::decode(&fresh, &key).unwrap().seq_no, 1);
}
