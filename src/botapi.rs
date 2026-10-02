use anyhow::{anyhow, Result};
use axum::body::{to_bytes, Body};
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::mtproto::RsaKeyPair;
use crate::store::{now, Store, UserRow};

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub cfg: Arc<Config>,
    pub rsa: RsaKeyPair,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/server-key", get(server_key))
        .route("/serverPublicKey", get(server_key))
        .route("/bot:token/:method", any(bot_route))
        .route("/file/bot:token/:file_id", get(file_route))
        .with_state(state)
}

#[derive(Debug, Default, serde::Deserialize)]
struct Params {
    #[serde(flatten)]
    map: HashMap<String, Value>,
}

impl Params {
    fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }
    fn str(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|v| v.as_str()).map(str::to_owned)
    }
    fn i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }
    fn i32(&self, key: &str) -> Option<i32> {
        self.get(key).and_then(|v| v.as_i64()).map(|v| v as i32)
    }
}

async fn server_key(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "result": {
            "fingerprint": state.rsa.fingerprint() as i64,
            "fingerprint_u64": format!("0x{:016x}", state.rsa.fingerprint() as u64),
            "public_key": state.rsa.public_pem(),
            "public_key_rsa": state.rsa.public_pem_rsa(),
            "modulus_hex": state.rsa.n.to_str_radix(16),
            "exponent": state.rsa.e.to_str_radix(10),
            "mtproto_port": state.cfg.mtproto_port
        }
    }))
}

async fn bot_route(
    State(state): State<AppState>,
    Path((token, method)): Path<(String, String)>,
    req: Request<Body>,
) -> Response {
    match handle_bot(&state, &token, &method, req).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            let msg = e.to_string();
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"ok": false, "error_code": 400, "description": msg})),
            )
                .into_response()
        }
    }
}

async fn file_route(
    State(state): State<AppState>,
    Path((token, file_id)): Path<(String, String)>,
) -> Response {
    let token = token.trim_start_matches("bot");
    let bot = match state.store.get_user_by_token(token) {
        Ok(Some(v)) => v,
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };
    let _ = bot;
    match state.store.get_file(&file_id) {
        Ok(Some((name, mime, data))) => {
            let mut res = data.into_response();
            res.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                mime.parse().unwrap_or(axum::http::HeaderValue::from_static(
                    "application/octet-stream",
                )),
            );
            res.headers_mut().insert(
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", name)
                    .parse()
                    .unwrap_or(axum::http::HeaderValue::from_static("attachment")),
            );
            res
        }
        _ => (StatusCode::NOT_FOUND, "file not found").into_response(),
    }
}

async fn handle_bot(
    state: &AppState,
    token: &str,
    method: &str,
    req: Request<Body>,
) -> Result<Value> {
    let token = token.trim_start_matches("bot");
    let bot = state
        .store
        .get_user_by_token(token)?
        .ok_or_else(|| anyhow!("unauthorized"))?;
    let (parts, body) = req.into_parts();
    let body = to_bytes(body, 16 * 1024 * 1024)
        .await
        .map_err(|e| anyhow!("read body: {}", e))?;
    let content_type = parts
        .headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let params = if content_type.starts_with("application/json") {
        serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null)
    } else {
        let text = String::from_utf8_lossy(&body);
        let mut map = HashMap::new();
        for pair in text.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                map.insert(urldecode(k), Value::String(urldecode(v)));
            }
        }
        Value::Object(map.into_iter().collect())
    };
    let p = Params {
        map: params
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect(),
    };
    match method {
        "getMe" => Ok(user_json(&bot)),
        "sendMessage" => {
            let chat_id = p
                .i64("chat_id")
                .ok_or_else(|| anyhow!("chat_id required"))?;
            let text = p.str("text").unwrap_or_default();
            let msg_id = send_message(state, &bot, chat_id, &text, "text", None)?;
            Ok(json!({"ok": true, "result": message_json(&bot, chat_id, msg_id, &text)}))
        }
        "sendPhoto" | "sendDocument" => {
            let chat_id = p
                .i64("chat_id")
                .ok_or_else(|| anyhow!("chat_id required"))?;
            let caption = p.str("caption").unwrap_or_default();
            let file_id = p
                .str("photo")
                .or_else(|| p.str("document"))
                .unwrap_or_default();
            let media = if method == "sendPhoto" {
                "photo"
            } else {
                "document"
            };
            let msg_id = send_message(state, &bot, chat_id, &caption, media, Some(&file_id))?;
            Ok(json!({"ok": true, "result": message_json(&bot, chat_id, msg_id, &caption)}))
        }
        "getUpdates" => {
            let offset = p.i64("offset").unwrap_or(0);
            let limit = p.i32("limit").unwrap_or(100).clamp(1, 100);
            let rows = state.store.bot_updates(bot.id, None, limit, offset)?;
            let mut out = Vec::new();
            let mut ids = Vec::new();
            for (seq, _chat, _user, _kind, payload) in rows {
                ids.push(seq);
                let mut obj = serde_json::from_str::<Value>(&payload).unwrap_or(Value::Null);
                if let Some(o) = obj.as_object_mut() {
                    o.insert("update_id".into(), json!(seq));
                }
                out.push(obj);
            }
            if !ids.is_empty() {
                state.store.mark_bot_updates_consumed(&ids)?;
            }
            Ok(json!({"ok": true, "result": out}))
        }
        "deleteWebhook" | "setWebhook" | "deleteMyCommands" | "setMyCommands" => {
            Ok(json!({"ok": true, "result": true}))
        }
        "getChat" => {
            let chat_id = p
                .i64("chat_id")
                .ok_or_else(|| anyhow!("chat_id required"))?;
            Ok(json!({"ok": true, "result": {"id": chat_id, "type": "private", "title": ""}}))
        }
        "sendChatAction" => Ok(json!({"ok": true, "result": true})),
        _ => Err(anyhow!("unsupported Bot API method {}", method)),
    }
}

fn send_message(
    state: &AppState,
    bot: &UserRow,
    chat_id: i64,
    text: &str,
    media: &str,
    file_id: Option<&str>,
) -> Result<i32> {
    let kind = if chat_id > 0 { "user" } else { "chat" };
    let dialog_id = chat_id.abs();
    let msg_id = state.store.insert_message(
        kind,
        dialog_id,
        bot.id,
        None,
        text,
        media,
        file_id,
        None,
        rand::random(),
    )?;
    state
        .store
        .touch_dialog(chat_id, kind, dialog_id, msg_id, true)?;
    let payload = json!({
        "message_id": msg_id,
        "chat": {"id": chat_id, "type": if kind == "user" { "private" } else { "supergroup" }},
        "from": user_json(bot),
        "date": now(),
        "text": text,
        "media_kind": media,
        "file_id": file_id,
    });
    state
        .store
        .push_bot_update(chat_id, bot.id, bot.id, "message", &payload.to_string())?;
    Ok(msg_id)
}

fn user_json(u: &UserRow) -> Value {
    json!({
        "id": u.id,
        "is_bot": u.is_bot,
        "first_name": u.first_name,
        "last_name": u.last_name,
        "username": u.username,
        "phone": u.phone
    })
}

fn message_json(bot: &UserRow, chat_id: i64, message_id: i32, text: &str) -> Value {
    json!({
        "message_id": message_id,
        "chat": {"id": chat_id, "type": "private"},
        "from": user_json(bot),
        "date": now(),
        "text": text
    })
}

fn urldecode(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v as char);
                    i += 3;
                } else {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}
