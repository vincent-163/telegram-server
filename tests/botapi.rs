//! Integration tests for the HTTP Bot API.
//!
//! These drive the real router, so the URL shape, the form/JSON parameter
//! handling and the response envelope are all exercised together.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::path::PathBuf;
use std::sync::Arc;
use telegram_server::botapi::{router, AppState};
use telegram_server::config::Config;
use telegram_server::http_mtproto::HttpMtProtoState;
use telegram_server::mtproto::RsaKeyPair;
use telegram_server::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "12345:TESTBOTTOKEN";

struct Fixture {
    app: axum::Router,
    store: Store,
    user_id: i64,
    _dir: PathBuf,
}

fn setup() -> Fixture {
    let dir = std::env::temp_dir().join(format!("tgsrv-botapi-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::open(&dir.join("test.db")).unwrap();
    let user = store
        .create_user("15550000001", "Admin", "User", "admin", false, true)
        .unwrap();
    let bot = store
        .create_user("15550000099", "Bot", "", "testbot", true, false)
        .unwrap();
    store.set_bot_token(bot.id, TOKEN).unwrap();
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
    let rsa = RsaKeyPair::generate();
    let state = AppState {
        store: store.clone(),
        cfg: cfg.clone(),
        rsa,
        http_mtproto: Arc::new(HttpMtProtoState::new()),
    };
    Fixture {
        app: router(state),
        store,
        user_id: user.id,
        _dir: dir,
    }
}

async fn call_form(
    app: &axum::Router,
    method: &str,
    form: &str,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/bot{TOKEN}/{method}"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn call_json(
    app: &axum::Router,
    method: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/bot{TOKEN}/{method}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn call_get(app: &axum::Router, method: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("GET")
        .uri(format!("/bot{TOKEN}/{method}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn get_me_returns_the_bot() {
    let fx = setup();
    let (status, v) = call_get(&fx.app, "getMe").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["username"], "testbot");
    assert_eq!(v["result"]["is_bot"], true);
}

#[tokio::test]
async fn unknown_token_is_rejected() {
    let fx = setup();
    let req = Request::builder()
        .method("GET")
        .uri("/bot999:BAD/getMe")
        .body(Body::empty())
        .unwrap();
    let resp = fx.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// Form-encoded parameters arrive as strings; numeric ones must still parse.
#[tokio::test]
async fn send_message_accepts_form_encoded_chat_id() {
    let fx = setup();
    let (status, v) = call_form(
        &fx.app,
        "sendMessage",
        &format!("chat_id={}&text=hello", fx.user_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "response was {v}");
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["text"], "hello");
    assert_eq!(v["result"]["chat"]["id"], fx.user_id);
}

#[tokio::test]
async fn send_message_accepts_json_body() {
    let fx = setup();
    let (status, v) = call_json(
        &fx.app,
        "sendMessage",
        serde_json::json!({"chat_id": fx.user_id, "text": "json hello"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "response was {v}");
    assert_eq!(v["result"]["text"], "json hello");
}

#[tokio::test]
async fn send_message_requires_chat_id() {
    let fx = setup();
    let (status, v) = call_form(&fx.app, "sendMessage", "text=hello").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["ok"], false);
}

/// A message sent through the Bot API must be visible to the recipient's
/// normal client, i.e. it lands in the same dialog store.
#[tokio::test]
async fn bot_messages_reach_the_recipient_dialog() {
    let fx = setup();
    let (_, v) = call_form(
        &fx.app,
        "sendMessage",
        &format!("chat_id={}&text=from the bot", fx.user_id),
    )
    .await;
    assert_eq!(v["ok"], true);
    let rows = fx.store.dialogs_for(fx.user_id).unwrap();
    assert!(
        rows.iter().any(|r| r.dialog_type == "user"),
        "expected a user dialog, got {rows:?}"
    );
}

#[tokio::test]
async fn healthz_responds_ok() {
    let fx = setup();
    let req = Request::builder()
        .method("GET")
        .uri("/healthz")
        .body(Body::empty())
        .unwrap();
    let resp = fx.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

/// The advertised key must be parseable DER; clients reject anything else.
#[tokio::test]
async fn server_key_is_well_formed() {
    let fx = setup();
    let req = Request::builder()
        .method("GET")
        .uri("/server-key")
        .body(Body::empty())
        .unwrap();
    let resp = fx.app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let pem = v["result"]["public_key_rsa"].as_str().unwrap();
    assert!(pem.starts_with("-----BEGIN RSA PUBLIC KEY-----"));
    // A 2048-bit modulus plus a small exponent is ~270 bytes of base64.
    assert!(
        pem.len() > 300,
        "suspiciously short key: {} bytes",
        pem.len()
    );
    assert_eq!(v["result"]["exponent"], "65537");
}

/// Message ids are numbered per dialog, so the first message of every dialog
/// is id 1. A global primary key on `id` cannot express that.
#[tokio::test]
async fn two_dialogs_can_both_hold_message_id_1() {
    let fx = setup();
    let other = fx
        .store
        .create_user("15550000002", "Peer", "Person", "peer", false, false)
        .unwrap();
    let bot = fx.store.get_user_by_token(TOKEN).unwrap().unwrap();
    fx.store
        .insert_message(
            "user", fx.user_id, bot.id, None, "to admin", "text", None, None, 1,
        )
        .unwrap();
    fx.store
        .insert_message(
            "user", other.id, bot.id, None, "to peer", "text", None, None, 2,
        )
        .unwrap();
    let a = fx
        .store
        .get_message("user", fx.user_id, 1)
        .unwrap()
        .unwrap();
    let b = fx.store.get_message("user", other.id, 1).unwrap().unwrap();
    assert_eq!(a.message, "to admin");
    assert_eq!(b.message, "to peer");
}

/// A database created with the old global primary key must still open, keep its
/// rows, and accept a second dialog's first message afterwards.
#[tokio::test]
async fn legacy_messages_table_is_migrated_in_place() {
    let dir = std::env::temp_dir().join(format!("tgsrv-migrate-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("legacy.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 dialog_type TEXT NOT NULL,
                 dialog_id INTEGER NOT NULL,
                 sender_id INTEGER NOT NULL,
                 sender_chat_id INTEGER,
                 date INTEGER NOT NULL,
                 message TEXT NOT NULL DEFAULT '',
                 media_kind TEXT NOT NULL DEFAULT '',
                 media_file_id TEXT,
                 edited INTEGER NOT NULL DEFAULT 0,
                 reply_to INTEGER,
                 random_id INTEGER NOT NULL DEFAULT 0,
                 deleted INTEGER NOT NULL DEFAULT 0
             );
             INSERT INTO messages (id,dialog_type,dialog_id,sender_id,date,message)
             VALUES (1,'user',100,100,0,'legacy row');",
        )
        .unwrap();
    }
    let store = Store::open(&db).unwrap();
    let kept = store.get_message("user", 100, 1).unwrap().unwrap();
    assert_eq!(kept.message, "legacy row", "existing rows must survive");
    // And a different dialog may now also use id 1.
    store
        .insert_message(
            "user",
            200,
            200,
            None,
            "second dialog",
            "text",
            None,
            None,
            3,
        )
        .unwrap();
    assert_eq!(
        store.get_message("user", 200, 1).unwrap().unwrap().message,
        "second dialog"
    );
}
