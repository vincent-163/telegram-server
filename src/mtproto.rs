//! MTProto 2.0 authorization-key handshake, message framing and encryption.

use anyhow::{anyhow, bail, Context, Result};
use grammers_tl_types as tl;
use grammers_tl_types::{Deserializable, Serializable};
use num_bigint::{BigInt, BigUint, RandBigInt};
use num_integer::Integer;
use num_traits::{One, Zero};
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::crypto::*;
use crate::tl::{put_bytes, put_string};

/// An RSA key pair. Telegram's handshake requires the server to be able to
/// RSA-decrypt `p_q_inner_data`, so the private exponent is needed.
#[derive(Clone)]
pub struct RsaKeyPair {
    pub n: BigUint,
    pub e: BigUint,
    pub d: BigUint,
    pub n_bytes: Vec<u8>,
}

impl RsaKeyPair {
    pub fn generate() -> Self {
        let e = BigUint::from(65537u32);
        let mut rng = rand::thread_rng();
        loop {
            let p = gen_prime_bits(&mut rng, 1024);
            let q = gen_prime_bits(&mut rng, 1024);
            if p == q {
                continue;
            }
            let phi = (&p - BigUint::one()) * (&q - BigUint::one());
            let d = match modinv(&e, &phi) {
                Some(d) => d,
                None => continue,
            };
            let n = &p * &q;
            let n_bytes = n.to_bytes_be();
            // Telegram requires a 2048-bit key, so the top byte must both
            // exist and have its high bit set. Without the second check a
            // 2047-bit modulus is accepted roughly half the time.
            if n_bytes.len() != 256 || n_bytes[0] & 0x80 == 0 {
                continue;
            }
            return RsaKeyPair { n, e, d, n_bytes };
        }
    }

    pub fn fingerprint(&self) -> i64 {
        let mut buf = Vec::with_capacity(self.n_bytes.len() + 16);
        put_bytes(&mut buf, &self.n_bytes);
        put_bytes(&mut buf, &self.e.to_bytes_be());
        let h = sha1(&buf);
        i64::from_le_bytes(h[12..20].try_into().unwrap())
    }

    /// Raw RSA decryption of a 256-byte ciphertext block.
    pub fn decrypt_block(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() != 256 {
            bail!("RSA ciphertext must be 256 bytes, got {}", data.len());
        }
        let c = BigUint::from_bytes_be(data);
        if c >= self.n {
            bail!("RSA ciphertext >= modulus");
        }
        let m = c.modpow(&self.d, &self.n);
        let mut out = m.to_bytes_be();
        while out.len() < 256 {
            out.insert(0, 0);
        }
        Ok(out)
    }

    pub fn to_hex(&self) -> String {
        format!(
            "{}:{}:{}",
            self.n.to_str_radix(16),
            self.e.to_str_radix(16),
            self.d.to_str_radix(16)
        )
    }

    pub fn from_hex(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            bail!("malformed RSA key");
        }
        let n = BigUint::parse_bytes(parts[0].as_bytes(), 16).ok_or_else(|| anyhow!("bad n"))?;
        let e = BigUint::parse_bytes(parts[1].as_bytes(), 16).ok_or_else(|| anyhow!("bad e"))?;
        let d = BigUint::parse_bytes(parts[2].as_bytes(), 16).ok_or_else(|| anyhow!("bad d"))?;
        let n_bytes = n.to_bytes_be();
        Ok(RsaKeyPair { n, e, d, n_bytes })
    }

    pub fn public_pem(&self) -> String {
        let pkcs1 = self.pkcs1_der();
        let algorithm = [
            0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05,
            0x00,
        ];
        let mut bit_content = Vec::with_capacity(pkcs1.len() + 1);
        bit_content.push(0x00);
        bit_content.extend_from_slice(&pkcs1);
        let mut bit_string = vec![0x03];
        bit_string.extend_from_slice(&der_len(bit_content.len()));
        bit_string.extend_from_slice(&bit_content);
        let mut spki = Vec::with_capacity(algorithm.len() + bit_string.len() + 8);
        spki.extend_from_slice(&algorithm);
        spki.extend_from_slice(&bit_string);
        let mut out = vec![0x30];
        out.extend_from_slice(&der_len(spki.len()));
        out.extend_from_slice(&spki);
        wrap_pem("PUBLIC KEY", &out)
    }

    pub fn public_pem_rsa(&self) -> String {
        wrap_pem("RSA PUBLIC KEY", &self.pkcs1_der())
    }
}

impl RsaKeyPair {
    fn pkcs1_der(&self) -> Vec<u8> {
        let mut ints = Vec::new();
        for value in [self.n.clone(), self.e.clone()] {
            let mut bytes = value.to_bytes_be();
            if bytes.is_empty() {
                bytes.push(0);
            }
            // DER INTEGERs are signed, so a value whose most significant bit
            // is set needs a leading zero byte. A 2048-bit modulus always has
            // it set, and without this the PEM encodes a negative number that
            // standard RSA parsers reject.
            if bytes[0] & 0x80 != 0 {
                bytes.insert(0, 0);
            }
            ints.push(0x02);
            ints.extend_from_slice(&der_len(bytes.len()));
            ints.extend_from_slice(&bytes);
        }
        let mut sequence = vec![0x30];
        sequence.extend_from_slice(&der_len(ints.len()));
        sequence.extend_from_slice(&ints);
        sequence
    }
}

fn der_len(length: usize) -> Vec<u8> {
    if length < 128 {
        return vec![length as u8];
    }
    let bytes = length.to_be_bytes();
    let first = bytes
        .iter()
        .position(|v| *v != 0)
        .unwrap_or(bytes.len() - 1);
    let bytes = &bytes[first..];
    let mut out = vec![0x80 | bytes.len() as u8];
    out.extend_from_slice(bytes);
    out
}

fn wrap_pem(label: &str, body: &[u8]) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(body);
    let mut out = format!("-----BEGIN {label}-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap());
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

fn modinv(a: &BigUint, m: &BigUint) -> Option<BigUint> {
    let a = BigInt::from(a.clone());
    let m = BigInt::from(m.clone());
    if a.is_zero() || m.is_zero() {
        return None;
    }
    let gcd = a.extended_gcd(&m);
    if gcd.gcd != BigInt::one() {
        return None;
    }
    gcd.x.mod_floor(&m).to_biguint()
}

fn gen_prime_bits<R: Rng>(rng: &mut R, bits: u32) -> BigUint {
    loop {
        let mut candidate = rng.gen_biguint(bits as u64);
        candidate.set_bit(0, true);
        candidate.set_bit((bits - 1) as u64, true);
        if (&candidate % BigUint::from(3u32)).is_zero() {
            continue;
        }
        if is_probable_prime_big(&candidate) {
            return candidate;
        }
    }
}

/// Miller-Rabin for arbitrary-size integers.
pub fn is_probable_prime_big(n: &BigUint) -> bool {
    if *n < BigUint::from(2u32) {
        return false;
    }
    for p in [2u32, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let p = BigUint::from(p);
        if *n == p {
            return true;
        }
        if (&*n % &p).is_zero() {
            return false;
        }
    }
    let one = BigUint::one();
    let n_minus_1 = n - &one;
    let r = n_minus_1.trailing_zeros().unwrap_or(0);
    let d = &n_minus_1 >> r;
    let bases = [2u32, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47];
    for a in bases {
        let a = BigUint::from(a) % n;
        if a.is_zero() || a == one {
            continue;
        }
        let mut x = a.modpow(&d, n);
        if x == one || x == n_minus_1 {
            continue;
        }
        let mut composite = true;
        for _ in 1..r {
            x = x.modpow(&BigUint::from(2u32), n);
            if x == n_minus_1 {
                composite = false;
                break;
            }
        }
        if composite {
            return false;
        }
    }
    true
}

/// Strips the `RSA_PAD_HASHED` framing from a decrypted `p_q_inner_data`
/// block: `temp_key(32) || AES-IGE(reversed data||padding || sha256(temp_key||data||padding))`.
///
/// The block was RSA-encoded as a big-endian integer, so leading zero bytes may
/// have been dropped; the scheme always produces a 256-byte plaintext, hence
/// the tail of the decrypted buffer is the exact block.
fn unpad_hashed(raw: &[u8]) -> Result<Vec<u8>> {
    if raw.len() < 256 {
        bail!("unexpected RSA plaintext length {}", raw.len());
    }
    let raw = &raw[raw.len() - 256..];
    let aes_encrypted = &raw[32..256];
    let temp_key_xor: [u8; 32] = raw[0..32].try_into().unwrap();
    let mut temp_key = temp_key_xor;
    let h = sha256(aes_encrypted);
    for i in 0..32 {
        temp_key[i] ^= h[i];
    }
    let mut data_hash = aes_encrypted.to_vec();
    ige_decrypt(&mut data_hash, &temp_key, &[0u8; 32]);
    let mut data_with_padding = data_hash[..192].to_vec();
    let hash = data_hash[192..].to_vec();
    data_with_padding.reverse();
    let expected = sha256_concat(&[&temp_key, &data_with_padding]);
    if hash[..] != expected[..] {
        bail!("p_q_inner_data hash mismatch");
    }
    Ok(data_with_padding)
}

/// Strips the legacy `RSA_PAD` framing: `sha1(data) || data || random padding`,
/// as produced by Telethon, TDLib and the official mobile/desktop clients.
///
/// The plaintext is always `20 + data + (235 - data)` = 255 bytes, so the tail
/// of the decrypted buffer is the exact block even when the integer encoding
/// dropped a leading zero byte.
fn unpad_legacy(raw: &[u8]) -> Result<Vec<u8>> {
    if raw.len() < 255 {
        bail!("unexpected RSA plaintext length {}", raw.len());
    }
    let block = &raw[raw.len() - 255..];
    let hash = &block[..20];
    let body = &block[20..];
    if body.len() < 4 {
        bail!("legacy p_q_inner_data too short");
    }
    let ctor = u32::from_le_bytes(body[0..4].try_into().unwrap());
    if !matches!(ctor, 0x83c95aec | 0xa9f55f95 | 0x3c6a84d4 | 0x56fddf88) {
        bail!(
            "legacy p_q_inner_data has unknown constructor {:#010x}",
            ctor
        );
    }
    let mut cursor = grammers_tl_types::Cursor::from_slice(body);
    let inner = tl::enums::PQInnerData::deserialize(&mut cursor)
        .map_err(|e| anyhow!("bad legacy p_q_inner_data: {}", e))?;
    let end = cursor.pos();
    if sha1(&body[..end])[..] != hash[..] {
        bail!("legacy p_q_inner_data hash mismatch");
    }
    let _ = inner;
    Ok(body[..end].to_vec())
}

/// Server-side handshake state.
pub struct Handshake {
    pub nonce: [u8; 16],
    pub server_nonce: Option<[u8; 16]>,
    pub new_nonce: Option<[u8; 32]>,
    pub pq: Option<u64>,
    pub p: Option<u64>,
    pub q: Option<u64>,
    pub dh_secret: Option<BigUint>,
    pub g_b: Option<BigUint>,
    pub state: HandshakePhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakePhase {
    WaitingPq,
    WaitingDhParams,
    WaitingClientDh,
    Finished,
}

impl Handshake {
    pub fn new(nonce: [u8; 16]) -> Self {
        Handshake {
            nonce,
            server_nonce: None,
            new_nonce: None,
            pq: None,
            p: None,
            q: None,
            dh_secret: None,
            g_b: None,
            state: HandshakePhase::WaitingPq,
        }
    }

    /// Returns the `resPQ` *enum*, whose `to_bytes()` includes the
    /// constructor id. Serializing the bare `types::ResPq` would omit it and
    /// clients would reject the reply as an unknown constructor.
    /// Record the client nonce from `req_pq`/`req_pq_multi`. The handshake
    /// object is created before the request arrives, so the nonce it was
    /// constructed with is only a placeholder.
    pub fn set_nonce(&mut self, nonce: [u8; 16]) {
        self.nonce = nonce;
    }

    pub fn step1(&mut self, key: &RsaKeyPair) -> Result<tl::enums::ResPq> {
        if self.state != HandshakePhase::WaitingPq {
            bail!("handshake out of order");
        }
        let mut server_nonce = [0u8; 16];
        rand::thread_rng().fill(&mut server_nonce);
        let (pqb, pb, qb) = generate_pq()?;
        let pq = u64::from_be_bytes(pqb);
        self.server_nonce = Some(server_nonce);
        self.pq = Some(pq);
        self.p = Some(u64::from_be_bytes(pad8(&pb)));
        self.q = Some(u64::from_be_bytes(pad8(&qb)));
        self.state = HandshakePhase::WaitingDhParams;
        let fingerprint = key.fingerprint();
        Ok(tl::enums::ResPq::Pq(tl::types::ResPq {
            nonce: self.nonce,
            server_nonce,
            pq: pqb.to_vec(),
            server_public_key_fingerprints: vec![fingerprint],
        }))
    }

    pub fn step2(
        &mut self,
        req: &tl::functions::ReqDhParams,
        key: &RsaKeyPair,
    ) -> Result<tl::enums::ServerDhParams> {
        if self.state != HandshakePhase::WaitingDhParams {
            bail!("handshake out of order");
        }
        let server_nonce = self
            .server_nonce
            .ok_or_else(|| anyhow!("missing server_nonce"))?;
        if req.nonce != self.nonce || req.server_nonce != server_nonce {
            bail!("nonce mismatch");
        }
        if req.public_key_fingerprint != key.fingerprint() {
            bail!("unknown public key fingerprint");
        }
        let p = BigUint::from_bytes_be(&req.p);
        let q = BigUint::from_bytes_be(&req.q);
        let expect_pq = BigUint::from(self.pq.ok_or_else(|| anyhow!("missing pq"))?);
        if &p * &q != expect_pq {
            bail!("p*q does not match pq");
        }

        // RSA-decrypt the p_q_inner_data blob. Clients use either the modern
        // RSA_PAD_HASHED scheme or the legacy RSA_PAD one, so try both.
        let raw = key.decrypt_block(&req.encrypted_data)?;
        let data_with_padding = if let Ok(v) = unpad_hashed(&raw) {
            v
        } else {
            unpad_legacy(&raw).context("p_q_inner_data could not be unpadded")?
        };
        let inner = tl::enums::PQInnerData::deserialize(
            &mut grammers_tl_types::Cursor::from_slice(&data_with_padding),
        )?;
        let new_nonce: [u8; 32] = inner.new_nonce();
        let inner_nonce: [u8; 16] = inner.nonce();
        let inner_server_nonce: [u8; 16] = inner.server_nonce();
        if inner_nonce != self.nonce || inner_server_nonce != server_nonce {
            bail!("p_q_inner_data nonce mismatch");
        }

        // Generate DH parameters.
        let dh_prime = dh_prime();
        let g = BigUint::from(DH_GENERATOR);
        let mut rng = rand::thread_rng();
        let dh_secret = rng.gen_biguint_below(&(dh_prime.clone() - BigUint::one()));
        let g_b = g.modpow(&dh_secret, &dh_prime);

        // Prepare server_DH_inner_data.
        let server_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i32)
            .unwrap_or(0);
        // The inner data is read by the client as a standalone TL object, so
        // its constructor id must be present.
        let mut answer = tl::enums::ServerDhInnerData::Data(tl::types::ServerDhInnerData {
            nonce: self.nonce,
            server_nonce,
            g: DH_GENERATOR as i32,
            dh_prime: dh_prime_bytes(),
            g_a: g_b.to_bytes_be(),
            server_time,
        })
        .to_bytes();
        let mut answer_with_hash = Vec::with_capacity(20 + answer.len() + 16);
        answer_with_hash.extend_from_slice(&sha1(&answer));
        answer_with_hash.extend_from_slice(&answer);
        while answer_with_hash.len() % 16 != 0 {
            answer_with_hash.push(0);
        }
        let (k, iv) = dh_aes_key_iv(&server_nonce, &new_nonce);
        ige_encrypt(&mut answer_with_hash, &k, &iv);

        self.new_nonce = Some(new_nonce);
        self.dh_secret = Some(dh_secret);
        self.g_b = Some(g_b);
        self.state = HandshakePhase::WaitingClientDh;
        Ok(tl::enums::ServerDhParams::Ok(tl::types::ServerDhParamsOk {
            nonce: self.nonce,
            server_nonce,
            encrypted_answer: answer_with_hash,
        }))
    }

    pub fn step3(
        &mut self,
        req: &tl::functions::SetClientDhParams,
    ) -> Result<(tl::enums::SetClientDhParamsAnswer, [u8; 256], i64)> {
        if self.state != HandshakePhase::WaitingClientDh {
            bail!("handshake out of order");
        }
        let server_nonce = self
            .server_nonce
            .ok_or_else(|| anyhow!("missing server_nonce"))?;
        let new_nonce = self.new_nonce.ok_or_else(|| anyhow!("missing new_nonce"))?;
        if req.nonce != self.nonce || req.server_nonce != server_nonce {
            bail!("nonce mismatch");
        }
        let (k, iv) = dh_aes_key_iv(&server_nonce, &new_nonce);
        if req.encrypted_data.len() % 16 != 0 {
            bail!("client DH data not block aligned");
        }
        let mut plain = req.encrypted_data.clone();
        ige_decrypt(&mut plain, &k, &iv);
        if plain.len() < 20 {
            bail!("client DH data too short");
        }
        let hash = plain[0..20].to_vec();
        // The client hashes the serialized object *including* its constructor
        // id, so parse through the enum to get the same bytes back.
        let mut cursor = grammers_tl_types::Cursor::from_slice(&plain[20..]);
        let inner = tl::enums::ClientDhInnerData::deserialize(&mut cursor)
            .map_err(|e| anyhow!("bad client DH inner data: {}", e))?;
        let cursor_pos = cursor.pos();
        let expected = sha1(&plain[20..20 + cursor_pos]);
        if hash[..] != expected[..] {
            bail!("client DH hash mismatch");
        }
        let inner = match inner {
            tl::enums::ClientDhInnerData::Data(v) => v,
        };
        if inner.nonce != self.nonce || inner.server_nonce != server_nonce {
            bail!("client DH nonce mismatch");
        }
        let dh_prime = dh_prime();
        let client_pub = BigUint::from_bytes_be(&inner.g_b);
        let lower = dh_public_lower_bound();
        let upper = &dh_prime - &lower;
        if client_pub <= BigUint::one()
            || client_pub >= dh_prime
            || client_pub < lower
            || client_pub >= upper
        {
            bail!("client DH public value out of range");
        }
        let dh_secret = self
            .dh_secret
            .clone()
            .ok_or_else(|| anyhow!("missing dh secret"))?;
        let auth = client_pub.modpow(&dh_secret, &dh_prime);
        let mut auth_key = [0u8; 256];
        let auth_bytes = auth.to_bytes_be();
        if auth_bytes.len() > 256 {
            bail!("auth key too long");
        }
        let skip = 256 - auth_bytes.len();
        auth_key[skip..].copy_from_slice(&auth_bytes);

        let aux_hash = sha1(&auth_key)[0..8].to_vec();
        let mut hash_input = Vec::with_capacity(32 + 1 + 8);
        hash_input.extend_from_slice(&new_nonce);
        hash_input.push(1);
        hash_input.extend_from_slice(&aux_hash);
        let mut nonce_hash = [0u8; 16];
        nonce_hash.copy_from_slice(&sha1(&hash_input)[4..20]);

        let mut salt_buf = [0u8; 8];
        for i in 0..8 {
            salt_buf[i] = new_nonce[i] ^ server_nonce[i];
        }
        let first_salt = i64::from_le_bytes(salt_buf);

        self.state = HandshakePhase::Finished;
        let answer = tl::enums::SetClientDhParamsAnswer::DhGenOk(tl::types::DhGenOk {
            nonce: self.nonce,
            server_nonce,
            new_nonce_hash1: nonce_hash,
        });
        Ok((answer, auth_key, first_salt))
    }
}

fn pad8(v: &[u8]) -> [u8; 8] {
    let mut out = [0u8; 8];
    let n = v.len().min(8);
    out[8 - n..].copy_from_slice(&v[v.len() - n..]);
    out
}

/// MTProto unencrypted (plain) message.
pub struct PlainMessage {
    pub msg_id: i64,
    pub body: Vec<u8>,
}

impl PlainMessage {
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 20 {
            bail!("plain message too short");
        }
        let key_id = i64::from_le_bytes(data[0..8].try_into().unwrap());
        if key_id != 0 {
            bail!("not a plain message");
        }
        let msg_id = i64::from_le_bytes(data[8..16].try_into().unwrap());
        if msg_id <= 0 || msg_id % 4 != 0 {
            bail!("invalid plain message id {}", msg_id);
        }
        let len = i32::from_le_bytes(data[16..20].try_into().unwrap());
        if len <= 0 {
            bail!("invalid plain message length");
        }
        let len = len as usize;
        if 20 + len > data.len() {
            bail!("plain message length overruns buffer");
        }
        Ok(PlainMessage {
            msg_id,
            body: data[20..20 + len].to_vec(),
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(20 + self.body.len());
        out.extend_from_slice(&0i64.to_le_bytes());
        out.extend_from_slice(&self.msg_id.to_le_bytes());
        out.extend_from_slice(&(self.body.len() as i32).to_le_bytes());
        out.extend_from_slice(&self.body);
        out
    }
}

/// Generate a server message id: high 32 bits are wall-clock seconds, low 32
/// bits encode the sequence. Message ids must remain divisible by four for
/// both plain and encrypted MTProto packets.
pub struct MsgIdGen {
    counter: u32,
}

impl MsgIdGen {
    pub fn new() -> Self {
        MsgIdGen { counter: 1 }
    }

    pub fn next(&mut self, _response: bool) -> i64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);
        self.counter = self.counter.wrapping_add(4);
        let low = self.counter & !3;
        ((now as i64) << 32) | (low as i64)
    }
}

/// An MTProto 2.0 encrypted envelope.
#[derive(Debug, Clone)]
pub struct EncryptedEnvelope {
    pub salt: i64,
    pub session_id: i64,
    pub msg_id: i64,
    pub seq_no: i32,
    pub body: Vec<u8>,
}

impl EncryptedEnvelope {
    /// Decrypt a server-bound (`auth_key_id != 0`) packet.
    pub fn decode(data: &[u8], auth_key: &[u8; 256]) -> Result<Self> {
        if data.len() < 24 || (data.len() - 24) % 16 != 0 {
            bail!("invalid encrypted packet length {}", data.len());
        }
        let key_id = i64::from_le_bytes(data[0..8].try_into().unwrap());
        if key_id != auth_key_id(auth_key) {
            bail!("auth key id mismatch");
        }
        let mut msg_key = [0u8; 16];
        msg_key.copy_from_slice(&data[8..24]);
        let (key, iv) = msg_key_to_aes_key_iv(auth_key, &msg_key, true);
        let mut plain = data[24..].to_vec();
        ige_decrypt(&mut plain, &key, &iv);
        let expected = sha256_concat(&[&auth_key[88..120], &plain]);
        if msg_key[..] != expected[8..24] {
            bail!("message key mismatch");
        }
        if plain.len() < 32 {
            bail!("encrypted plaintext too short");
        }
        let salt = i64::from_le_bytes(plain[0..8].try_into().unwrap());
        let session_id = i64::from_le_bytes(plain[8..16].try_into().unwrap());
        let msg_id = i64::from_le_bytes(plain[16..24].try_into().unwrap());
        let seq_no = i32::from_le_bytes(plain[24..28].try_into().unwrap());
        let len = i32::from_le_bytes(plain[28..32].try_into().unwrap());
        if len <= 0 || (32 + len as usize) > plain.len() {
            bail!("invalid inner message length {}", len);
        }
        Ok(EncryptedEnvelope {
            salt,
            session_id,
            msg_id,
            seq_no,
            body: plain[32..32 + len as usize].to_vec(),
        })
    }

    /// Encrypt a server->client packet.
    pub fn encode(&self, auth_key: &[u8; 256]) -> Vec<u8> {
        let mut plain = Vec::with_capacity(32 + self.body.len() + 32);
        plain.extend_from_slice(&self.salt.to_le_bytes());
        plain.extend_from_slice(&self.session_id.to_le_bytes());
        plain.extend_from_slice(&self.msg_id.to_le_bytes());
        plain.extend_from_slice(&self.seq_no.to_le_bytes());
        plain.extend_from_slice(&(self.body.len() as i32).to_le_bytes());
        plain.extend_from_slice(&self.body);
        let padding = 16 + (16 - (plain.len() % 16));
        let mut rng = rand::thread_rng();
        for _ in 0..padding {
            plain.push(rng.gen());
        }
        let expected = sha256_concat(&[&auth_key[96..128], &plain]);
        let mut msg_key = [0u8; 16];
        msg_key.copy_from_slice(&expected[8..24]);
        let (key, iv) = msg_key_to_aes_key_iv(auth_key, &msg_key, false);
        ige_encrypt(&mut plain, &key, &iv);
        let mut out = Vec::with_capacity(24 + plain.len());
        out.extend_from_slice(&auth_key_id(auth_key).to_le_bytes());
        out.extend_from_slice(&msg_key);
        out.extend_from_slice(&plain);
        out
    }
}

/// Build an `rpc_result` container for a response body.
pub fn rpc_result(req_msg_id: i64, result: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + result.len());
    out.extend_from_slice(&0xf35c_6d01u32.to_le_bytes());
    out.extend_from_slice(&req_msg_id.to_le_bytes());
    out.extend_from_slice(result);
    out
}

/// Build an `rpc_error#2144ca19 error_code:int error_message:string` body.
///
/// Note this is *not* a top-level reply: a client correlates errors by the
/// `req_msg_id` of the enclosing `rpc_result`, so callers must wrap this with
/// [`rpc_result`].
pub fn rpc_error(code: i32, message: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + message.len());
    out.extend_from_slice(&0x2144_ca19u32.to_le_bytes());
    out.extend_from_slice(&code.to_le_bytes());
    put_string(&mut out, message);
    out
}
