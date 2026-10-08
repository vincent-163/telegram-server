use grammers_tl_types::{Deserializable, Serializable};
use std::path::PathBuf;
use std::sync::Arc;
use telegram_server::botapi::AppState;
use telegram_server::config::Config;
use telegram_server::http_mtproto::{self, HttpMtProtoState};
use telegram_server::mtproto::{PlainMessage, RsaKeyPair};
use telegram_server::store::Store;

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
        rsa: RsaKeyPair::generate(),
        http_mtproto: Arc::new(HttpMtProtoState::new()),
    };
    (state, dir)
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
    let response = PlainMessage::decode(&response).unwrap();
    let ctor = u32::from_le_bytes(response.body[0..4].try_into().unwrap());
    assert_eq!(ctor, 0x05162463);
    let res_pq = grammers_tl_types::types::ResPq::from_bytes(&response.body[4..]).unwrap();
    assert_eq!(res_pq.nonce, nonce);
}
