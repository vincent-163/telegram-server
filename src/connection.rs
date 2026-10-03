use anyhow::{bail, Context, Result};
use grammers_tl_types::{Deserializable, Serializable};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::config::Config;
use crate::crypto::auth_key_id;
use crate::mtproto::{
    rpc_error, rpc_result, EncryptedEnvelope, Handshake, MsgIdGen, PlainMessage, RsaKeyPair,
};
use crate::rpc::{dispatch, RpcContext, RpcError};
use crate::store::Store;
use crate::transport::{Decoder, Encoder};

pub async fn serve(cfg: Arc<Config>, store: Store, rsa: RsaKeyPair) -> Result<()> {
    let listener = TcpListener::bind((cfg.mtproto_bind.as_str(), cfg.mtproto_port))
        .await
        .with_context(|| format!("bind mtproto {}:{}", cfg.mtproto_bind, cfg.mtproto_port))?;
    tracing::info!(
        "MTProto listening on {}:{}",
        cfg.mtproto_bind,
        cfg.mtproto_port
    );
    loop {
        let (stream, peer) = listener.accept().await?;
        let cfg = cfg.clone();
        let store = store.clone();
        let rsa = rsa.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, cfg, store, rsa, peer).await {
                tracing::warn!("connection {} closed: {:#}", peer, e);
            }
        });
    }
}

async fn handle(
    mut stream: TcpStream,
    cfg: Arc<Config>,
    store: Store,
    rsa: RsaKeyPair,
    peer: std::net::SocketAddr,
) -> Result<()> {
    let mut decoder = Decoder::new();
    let mut encoder: Option<Encoder> = None;
    let mut handshake = Handshake::new(rand::random());
    let mut auth_key: Option<[u8; 256]> = None;
    let mut msg_ids = MsgIdGen::new();
    let mut read = vec![0u8; 64 * 1024];
    loop {
        let n = stream.read(&mut read).await?;
        if n == 0 {
            return Ok(());
        }
        decoder.push(&read[..n])?;
        if !decoder.ensure_started()? {
            continue;
        }
        if encoder.is_none() {
            let kind = decoder.kind().context("transport kind missing")?;
            let mut enc = Encoder::new(kind);
            if let Some(init) = decoder.obfuscation_init() {
                enc.enable_obfuscation(&init);
            }
            encoder = Some(enc);
        }
        while let Some(mut payload) = decoder.next_payload()? {
            // Padded-intermediate transport padding is not part of the MTProto packet.
            if payload.len() > 24 && (payload.len() - 24) % 16 != 0 {
                let trim = (payload.len() - 24) % 16;
                payload.truncate(payload.len() - trim);
            }
            if let Some(key) = auth_key.as_ref() {
                let env = match EncryptedEnvelope::decode(&payload, key) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("bad encrypted packet from {}: {:#}", peer, e);
                        continue;
                    }
                };
                let mut ctx = RpcContext {
                    store: store.clone(),
                    cfg: cfg.clone(),
                    auth_key_id: auth_key_id(key),
                    user_id: store.session_user(auth_key_id(key))?.unwrap_or(0),
                    layer: 227,
                };
                let response = match dispatch(&mut ctx, &env.body) {
                    Ok(result) if result.is_empty() => Vec::new(),
                    Ok(result) => rpc_result(env.msg_id, &result),
                    Err(e) => match e.downcast_ref::<RpcError>() {
                        Some(re) => rpc_error(re.code, &re.message),
                        None => rpc_error(400, "INTERNAL_ERROR"),
                    },
                };
                if !response.is_empty() {
                    let out_env = EncryptedEnvelope {
                        salt: env.salt,
                        session_id: env.session_id,
                        msg_id: msg_ids.next(true),
                        seq_no: 1,
                        body: response,
                    };
                    write_frame(&mut stream, encoder.as_mut().unwrap(), &out_env.encode(key))
                        .await?;
                }
                continue;
            }
            let plain = PlainMessage::decode(&payload)?;
            let body = plain.body;
            if body.len() < 4 {
                continue;
            }
            let ctor = u32::from_le_bytes(body[0..4].try_into().unwrap());
            let response_body = match ctor {
                0x60469778 | 0xbe7e8ef1 => {
                    let _ = grammers_tl_types::functions::ReqPqMulti::deserialize(
                        &mut grammers_tl_types::Cursor::from_slice(&body),
                    )?;
                    handshake.step1(&rsa)?.to_bytes()
                }
                0xd712e4be => {
                    let f = grammers_tl_types::functions::ReqDhParams::deserialize(
                        &mut grammers_tl_types::Cursor::from_slice(&body),
                    )?;
                    handshake.step2(&f, &rsa)?.to_bytes()
                }
                0xf5045f1f => {
                    let f = grammers_tl_types::functions::SetClientDhParams::deserialize(
                        &mut grammers_tl_types::Cursor::from_slice(&body),
                    )?;
                    let (answer, key, _salt) = handshake.step3(&f)?;
                    store.save_auth_key(auth_key_id(&key), &key)?;
                    auth_key = Some(key);
                    answer.to_bytes()
                }
                _ => bail!("unexpected plain MTProto constructor {:#010x}", ctor),
            };
            let response = PlainMessage {
                msg_id: msg_ids.next(true),
                body: response_body,
            };
            write_frame(&mut stream, encoder.as_mut().unwrap(), &response.encode()).await?;
            if auth_key.is_some() {
                break;
            }
        }
    }
}

async fn write_frame(stream: &mut TcpStream, encoder: &mut Encoder, payload: &[u8]) -> Result<()> {
    let frame = encoder.encode(payload)?;
    stream.write_all(&frame).await?;
    stream.flush().await?;
    Ok(())
}
