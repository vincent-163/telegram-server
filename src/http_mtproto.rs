use anyhow::{anyhow, bail, Result};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use grammers_tl_types::{Deserializable, Serializable};
use parking_lot::Mutex;
use std::collections::HashMap;

use crate::botapi::AppState;
use crate::crypto::auth_key_id;
use crate::mtproto::{rpc_error, rpc_result, EncryptedEnvelope, Handshake, MsgIdGen, PlainMessage};
use crate::rpc::{dispatch, RpcContext, RpcError};

pub struct HttpMtProtoState {
    handshakes: Mutex<HashMap<[u8; 16], Handshake>>,
    msg_ids: Mutex<MsgIdGen>,
}

impl HttpMtProtoState {
    pub fn new() -> Self {
        Self {
            handshakes: Mutex::new(HashMap::new()),
            msg_ids: Mutex::new(MsgIdGen::new()),
        }
    }
}

pub async fn handle(State(state): State<AppState>, body: Bytes) -> Response {
    if body.is_empty() {
        let mut response = StatusCode::NO_CONTENT.into_response();
        add_cors(&mut response);
        return response;
    }

    match process(&state, &body) {
        Ok(response) => {
            let mut response = response.into_response();
            add_cors(&mut response);
            response
        }
        Err(error) => {
            tracing::warn!("HTTP MTProto request failed: {:#}", error);
            let mut response = (StatusCode::BAD_REQUEST, format!("{:#}", error)).into_response();
            add_cors(&mut response);
            response
        }
    }
}

pub fn process(state: &AppState, payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() >= 8 && i64::from_le_bytes(payload[0..8].try_into().unwrap()) != 0 {
        return process_encrypted(state, payload);
    }
    process_plain(state, payload)
}

fn process_plain(state: &AppState, payload: &[u8]) -> Result<Vec<u8>> {
    let plain = PlainMessage::decode(payload)?;
    if plain.body.len() < 4 {
        bail!("plain MTProto body too short");
    }
    let ctor = u32::from_le_bytes(plain.body[0..4].try_into().unwrap());
    let response_body = match ctor {
        0x60469778 | 0xbe7e8ef1 => {
            let request = grammers_tl_types::functions::ReqPqMulti::deserialize(
                &mut grammers_tl_types::Cursor::from_slice(&plain.body),
            )?;
            let mut handshake = Handshake::new(request.nonce);
            let response = handshake.step1(&state.rsa)?.to_bytes();
            state
                .http_mtproto
                .handshakes
                .lock()
                .insert(request.nonce, handshake);
            response
        }
        0xd712e4be => {
            let request = grammers_tl_types::functions::ReqDhParams::deserialize(
                &mut grammers_tl_types::Cursor::from_slice(&plain.body),
            )?;
            let mut handshakes = state.http_mtproto.handshakes.lock();
            let handshake = handshakes
                .get_mut(&request.nonce)
                .ok_or_else(|| anyhow!("unknown HTTP handshake nonce"))?;
            handshake.step2(&request, &state.rsa)?.to_bytes()
        }
        0xf5045f1f => {
            let request = grammers_tl_types::functions::SetClientDhParams::deserialize(
                &mut grammers_tl_types::Cursor::from_slice(&plain.body),
            )?;
            let nonce = request.nonce;
            let mut handshakes = state.http_mtproto.handshakes.lock();
            let handshake = handshakes
                .get_mut(&nonce)
                .ok_or_else(|| anyhow!("unknown HTTP handshake nonce"))?;
            let (answer, key, _salt) = handshake.step3(&request)?;
            state.store.save_auth_key(auth_key_id(&key), &key)?;
            handshakes.remove(&nonce);
            answer.to_bytes()
        }
        _ => bail!("unsupported plain HTTP MTProto constructor {:#010x}", ctor),
    };
    let response = PlainMessage {
        msg_id: state.http_mtproto.msg_ids.lock().next(true),
        body: response_body,
    };
    Ok(response.encode())
}

fn process_encrypted(state: &AppState, payload: &[u8]) -> Result<Vec<u8>> {
    let key_id = i64::from_le_bytes(payload[0..8].try_into().unwrap());
    let Some(key) = state.store.load_auth_key(key_id)? else {
        return Ok((-404i32).to_le_bytes().to_vec());
    };
    let envelope = EncryptedEnvelope::decode(payload, &key)?;
    let key_id = auth_key_id(&key);
    let mut ctx = RpcContext {
        store: state.store.clone(),
        cfg: state.cfg.clone(),
        auth_key_id: key_id,
        user_id: state.store.session_user(key_id)?.unwrap_or(0),
        layer: state.store.session_layer(key_id)?.unwrap_or(227),
    };
    let response_body = match dispatch(&mut ctx, &envelope.body) {
        Ok(result) if result.is_empty() => return Ok(Vec::new()),
        Ok(result) => rpc_result(envelope.msg_id, &result),
        Err(error) => {
            let body = match error.downcast_ref::<RpcError>() {
                Some(rpc) => rpc_error(rpc.code, &rpc.message),
                None => {
                    tracing::warn!("internal error handling HTTP MTProto request: {:#}", error);
                    rpc_error(400, "INTERNAL_ERROR")
                }
            };
            rpc_result(envelope.msg_id, &body)
        }
    };
    let response = EncryptedEnvelope {
        salt: envelope.salt,
        session_id: envelope.session_id,
        msg_id: state.http_mtproto.msg_ids.lock().next(true),
        seq_no: 1,
        body: response_body,
    };
    Ok(response.encode(&key))
}

fn add_cors(response: &mut Response) {
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
}
