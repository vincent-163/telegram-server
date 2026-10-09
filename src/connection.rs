use anyhow::{bail, Context, Result};
use grammers_tl_types::{Deserializable, Serializable};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::config::Config;
use crate::crypto::auth_key_id;
use crate::mtproto::{
    EncryptedEnvelope, Handshake, HandshakePhase, MsgIdGen, PlainMessage, RsaKeyPair, SeqNoGen,
};
use crate::rpc::{dispatch_replies, frame_replies, RpcContext};
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
    let mut auth_keys: Vec<[u8; 256]> = Vec::new();
    let mut msg_ids = MsgIdGen::new();
    let mut seq_nos = SeqNoGen::new();
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
            // Padded-intermediate appends random transport padding. Recover the
            // exact MTProto packet from its own framing: plain messages declare
            // their length in bytes 16..20, encrypted ones end on a 16-byte
            // boundary. Trimming plain messages to a 16-byte boundary (the old
            // behaviour) truncated larger requests such as req_DH_params.
            trim_transport_padding(&mut payload);
            let is_plain = payload.len() >= 8 && payload[..8] == [0u8; 8];
            if !is_plain {
                if payload.len() < 8 {
                    tracing::warn!("short encrypted packet from {}", peer);
                    continue;
                }
                let packet_key_id = i64::from_le_bytes(payload[0..8].try_into().unwrap());
                let key = auth_keys
                    .iter()
                    .find(|key| auth_key_id(key) == packet_key_id)
                    .copied()
                    .or_else(|| store.load_auth_key(packet_key_id).ok().flatten());
                let Some(key) = key else {
                    tracing::warn!("unknown auth key id {} from {}", packet_key_id, peer);
                    continue;
                };
                let env = match EncryptedEnvelope::decode(&payload, &key) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("bad encrypted packet from {}: {:#}", peer, e);
                        continue;
                    }
                };
                let key_id = auth_key_id(&key);
                let mut ctx = RpcContext {
                    store: store.clone(),
                    cfg: cfg.clone(),
                    auth_key_id: key_id,
                    user_id: store.session_user(key_id)?.unwrap_or(0),
                    // Default to the schema this server speaks; a client that
                    // sent `invokeWithLayer` has its real layer persisted.
                    layer: store.session_layer(key_id)?.unwrap_or(227),
                };
                let replies = dispatch_replies(&mut ctx, &env.body, env.msg_id);
                if let Some((body, seq_no)) =
                    frame_replies(replies, env.session_id, &mut msg_ids, &mut seq_nos)
                {
                    let out_env = EncryptedEnvelope {
                        salt: env.salt,
                        session_id: env.session_id,
                        msg_id: msg_ids.next(true),
                        seq_no,
                        body,
                    };
                    write_frame(
                        &mut stream,
                        encoder.as_mut().unwrap(),
                        &out_env.encode(&key),
                    )
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
            tracing::debug!(
                peer = %peer,
                constructor = format_args!("{ctor:#010x}"),
                phase = ?handshake.state,
                "plain MTProto request"
            );
            let response_body = match ctor {
                0x60469778 | 0xbe7e8ef1 => {
                    // Both req_pq#60469778 and req_pq_multi#be7e8ef1 carry the
                    // client nonce, which resPQ must echo back.
                    let mut cur = grammers_tl_types::Cursor::from_slice(&body[4..]);
                    let nonce = <[u8; 16] as Deserializable>::deserialize(&mut cur)?;
                    if handshake.state != HandshakePhase::WaitingPq {
                        handshake.restart_for_pq();
                    }
                    handshake.set_nonce(nonce);
                    handshake
                        .step1(&rsa)
                        .with_context(|| format!("process req_pq in phase {:?}", handshake.state))?
                        .to_bytes()
                }
                0xd712e4be => {
                    // Generated `Deserializable` impls read only the fields;
                    // the constructor id has already been consumed above, so
                    // the argument buffer must start after it.
                    let f = grammers_tl_types::functions::ReqDhParams::deserialize(
                        &mut grammers_tl_types::Cursor::from_slice(&body[4..]),
                    )?;
                    handshake
                        .step2(&f, &rsa)
                        .with_context(|| {
                            format!("process req_DH_params in phase {:?}", handshake.state)
                        })?
                        .to_bytes()
                }
                0xf5045f1f => {
                    let f = grammers_tl_types::functions::SetClientDhParams::deserialize(
                        &mut grammers_tl_types::Cursor::from_slice(&body[4..]),
                    )?;
                    let (answer, key, _salt) = handshake.step3(&f).with_context(|| {
                        format!(
                            "process set_client_DH_params in phase {:?}",
                            handshake.state
                        )
                    })?;
                    let key_id = auth_key_id(&key);
                    store.save_auth_key(key_id, &key)?;
                    if !auth_keys.iter().any(|known| auth_key_id(known) == key_id) {
                        auth_keys.push(key);
                    }
                    answer.to_bytes()
                }
                _ => bail!("unexpected plain MTProto constructor {:#010x}", ctor),
            };
            let response = PlainMessage {
                msg_id: msg_ids.next(true),
                body: response_body,
            };
            write_frame(&mut stream, encoder.as_mut().unwrap(), &response.encode()).await?;
        }
    }
}

/// Drop transport padding from a decoded MTProto packet.
///
/// `auth_key_id == 0` marks a plain message, whose declared body length is at
/// bytes 16..20. Encrypted messages carry `auth_key_id` then a 16-byte message
/// key, and the AES-IGE payload is always a multiple of 16.

fn trim_transport_padding(payload: &mut Vec<u8>) {
    if payload.len() < 4 {
        return;
    }
    let is_plain = payload.len() >= 8 && payload[0..8] == [0u8; 8];
    if is_plain {
        if payload.len() >= 20 {
            let declared = i32::from_le_bytes(payload[16..20].try_into().unwrap());
            if declared > 0 {
                let end = 20 + declared as usize;
                if end <= payload.len() {
                    payload.truncate(end);
                }
            }
        }
    } else {
        if payload.len() > 24 {
            let trim = (payload.len() - 24) % 16;
            if trim != 0 {
                payload.truncate(payload.len() - trim);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::trim_transport_padding;

    #[test]
    fn trims_plain_message_padding() {
        let mut p = Vec::new();
        p.extend_from_slice(&0i64.to_le_bytes()); // auth key id
        p.extend_from_slice(&1i64.to_le_bytes()); // msg id
        p.extend_from_slice(&3i32.to_le_bytes()); // declared body length
        p.extend_from_slice(b"abc");
        p.extend_from_slice(&[0xaa; 5]); // transport padding
        trim_transport_padding(&mut p);
        assert_eq!(p.len(), 23);
    }

    #[test]
    fn leaves_large_plain_messages_intact() {
        let mut p = Vec::new();
        p.extend_from_slice(&0i64.to_le_bytes());
        p.extend_from_slice(&1i64.to_le_bytes());
        p.extend_from_slice(&400i32.to_le_bytes());
        p.extend_from_slice(&vec![7u8; 400]);
        trim_transport_padding(&mut p);
        assert_eq!(p.len(), 420);
    }

    #[test]
    fn trims_encrypted_padding_to_16() {
        let mut p = vec![0u8; 24 + 64 + 3];
        p[0..8].copy_from_slice(&1i64.to_le_bytes()); // encrypted
        trim_transport_padding(&mut p);
        assert_eq!(p.len(), 24 + 64);
    }
}

async fn write_frame(stream: &mut TcpStream, encoder: &mut Encoder, payload: &[u8]) -> Result<()> {
    let frame = encoder.encode(payload)?;
    stream.write_all(&frame).await?;
    stream.flush().await?;
    Ok(())
}
