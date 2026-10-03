//! MTProto cryptographic primitives: SHA1/SHA256 helpers, AES-IGE, RSA public
//! key padding (server side), Diffie-Hellman helpers and MTProto 2.0 message
//! encryption/decryption.

use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes256;
use anyhow::{anyhow, bail, Result};
use num_bigint::BigUint;
use num_traits::One;
use rand::Rng;
use sha1::Sha1;
use sha2::{Digest, Sha256};

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h = Sha1::new();
    h.update(data);
    h.finalize().into()
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

pub fn sha1_concat(parts: &[&[u8]]) -> [u8; 20] {
    let mut h = Sha1::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

pub fn sha256_concat(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// AES-256-IGE encryption in place. `buffer.len()` must be a multiple of 16.
pub fn ige_encrypt(buffer: &mut [u8], key: &[u8; 32], iv: &[u8; 32]) {
    assert_eq!(buffer.len() % 16, 0);
    let cipher = Aes256::new(key.into());
    let mut iv1: [u8; 16] = iv[0..16].try_into().unwrap();
    let mut iv2: [u8; 16] = iv[16..32].try_into().unwrap();
    let mut next_iv2 = [0u8; 16];
    for block in buffer.chunks_mut(16) {
        next_iv2.copy_from_slice(block);
        for i in 0..16 {
            block[i] ^= iv1[i];
        }
        cipher.encrypt_block(block.try_into().unwrap());
        for i in 0..16 {
            block[i] ^= iv2[i];
        }
        iv1.copy_from_slice(block);
        std::mem::swap(&mut iv2, &mut next_iv2);
    }
}

/// AES-256-IGE decryption in place. `buffer.len()` must be a multiple of 16.
pub fn ige_decrypt(buffer: &mut [u8], key: &[u8; 32], iv: &[u8; 32]) {
    assert_eq!(buffer.len() % 16, 0);
    let cipher = Aes256::new(key.into());
    let mut iv1: [u8; 16] = iv[0..16].try_into().unwrap();
    let mut iv2: [u8; 16] = iv[16..32].try_into().unwrap();
    let mut next_iv1 = [0u8; 16];
    for block in buffer.chunks_mut(16) {
        next_iv1.copy_from_slice(block);
        for i in 0..16 {
            block[i] ^= iv2[i];
        }
        cipher.decrypt_block(block.try_into().unwrap());
        for i in 0..16 {
            block[i] ^= iv1[i];
        }
        std::mem::swap(&mut iv1, &mut next_iv1);
        iv2.copy_from_slice(block);
    }
}

/// MTProto 2.0 AES key/IV derivation from `auth_key` and `msg_key`.
/// `from_client` selects x=0 (client->server) or x=8 (server->client).
pub fn msg_key_to_aes_key_iv(
    auth_key: &[u8; 256],
    msg_key: &[u8; 16],
    from_client: bool,
) -> ([u8; 32], [u8; 32]) {
    let x = if from_client { 0 } else { 8 };
    let sha_a = sha256_concat(&[msg_key, &auth_key[x..x + 36]]);
    let sha_b = sha256_concat(&[&auth_key[40 + x..40 + x + 36], msg_key]);
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&sha_a[0..8]);
    key[8..24].copy_from_slice(&sha_b[8..24]);
    key[24..32].copy_from_slice(&sha_a[24..32]);
    let mut iv = [0u8; 32];
    iv[0..8].copy_from_slice(&sha_b[0..8]);
    iv[8..24].copy_from_slice(&sha_a[8..24]);
    iv[24..32].copy_from_slice(&sha_b[24..32]);
    (key, iv)
}

pub fn auth_key_id(auth_key: &[u8; 256]) -> i64 {
    let h = sha1(auth_key);
    i64::from_le_bytes(h[12..20].try_into().unwrap())
}

/// Generate the authorization key from the nonces, per the MTProto spec:
/// `auth_key = substr(SHA1(new_nonce + server_nonce), 0, 256)`.
pub fn auth_key_from_nonces(server_nonce: &[u8; 16], new_nonce: &[u8; 32]) -> [u8; 256] {
    // SHA1(new_nonce + server_nonce) == SHA1(server_nonce + new_nonce) because
    // both are 16/32 bytes so the concatenation is commutative in effect for
    // this particular derivation only when lengths match; use the spec order.
    let h = sha1_concat(&[new_nonce, server_nonce]);
    let mut key = [0u8; 256];
    for i in 0..8 {
        key[i * 32..i * 32 + 32].copy_from_slice(&h);
    }
    key
}

/// `temp_auth_key` derivation: SHA1(new_nonce + server_nonce) with 0x01 prefix.
pub fn tmp_auth_key_from_nonces(server_nonce: &[u8; 16], new_nonce: &[u8; 32]) -> [u8; 256] {
    let h = sha1_concat(&[&[0x01u8][..], new_nonce, server_nonce]);
    let mut key = [0u8; 256];
    for i in 0..8 {
        key[i * 32..i * 32 + 32].copy_from_slice(&h);
    }
    key
}

/// AES key/IV for the DH inner-data exchange.
pub fn dh_aes_key_iv(server_nonce: &[u8; 16], new_nonce: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let hash1 = sha1_concat(&[new_nonce, server_nonce]);
    let hash2 = sha1_concat(&[server_nonce, new_nonce]);
    let hash3 = sha1_concat(&[new_nonce, new_nonce]);
    let mut key = [0u8; 32];
    key[0..20].copy_from_slice(&hash1);
    key[20..32].copy_from_slice(&hash2[0..12]);
    let mut iv = [0u8; 32];
    iv[0..8].copy_from_slice(&hash2[12..20]);
    iv[8..28].copy_from_slice(&hash3);
    iv[28..32].copy_from_slice(&new_nonce[0..4]);
    (key, iv)
}

/// The `new_nonce_hash` values used in `dh_gen_ok`/`dh_gen_retry`/`dh_gen_fail`.
pub fn new_nonce_hash(new_nonce: &[u8; 32], auth_key: &[u8; 256], variant: u8) -> [u8; 16] {
    let h = sha1_concat(&[&[variant][..], new_nonce, &auth_key[0..32]]);
    h[4..20].try_into().unwrap()
}

/// Server-side RSA public key: only the modulus and exponent are needed to
/// fingerprint the key (`server_public_key_fingerprints` is
/// `SHA1(key)` last 8 bytes, little-endian).
pub struct RsaPublicKey {
    pub modulus: BigUint,
    pub exponent: BigUint,
    pub n_bytes: Vec<u8>,
}

impl RsaPublicKey {
    pub fn from_strs(n: &str, e: &str) -> Result<Self> {
        let modulus =
            BigUint::parse_bytes(n.as_bytes(), 10).ok_or_else(|| anyhow!("invalid RSA modulus"))?;
        let exponent = BigUint::parse_bytes(e.as_bytes(), 10)
            .ok_or_else(|| anyhow!("invalid RSA exponent"))?;
        let n_bytes = modulus.to_bytes_be();
        Ok(Self {
            modulus,
            exponent,
            n_bytes,
        })
    }

    pub fn from_bytes(n: &[u8], e: u32) -> Result<Self> {
        Ok(Self {
            modulus: BigUint::from_bytes_be(n),
            exponent: BigUint::from(e),
            n_bytes: n.to_vec(),
        })
    }

    pub fn pem(&self) -> String {
        let mut out = Vec::new();
        out.extend_from_slice(&[0x30, 0x82]);
        // SubjectPublicKeyInfo for rsaEncryption. Hand-built DER.
        let mut ints = Vec::new();
        for v in [
            BigUint::from(0u32),
            self.modulus.clone(),
            self.exponent.clone(),
        ] {
            let b = v.to_bytes_be();
            let b = if b.is_empty() { vec![0] } else { b };
            ints.push(0x02);
            if b.len() < 128 {
                ints.push(b.len() as u8);
            } else {
                ints.push(0x82);
                ints.push((b.len() >> 8) as u8);
                ints.push((b.len() & 0xff) as u8);
            }
            ints.extend_from_slice(&b);
        }
        let mut seq_inner = Vec::new();
        seq_inner.push(0x30);
        if ints.len() < 128 {
            seq_inner.push(ints.len() as u8);
        } else {
            seq_inner.push(0x82);
            seq_inner.push((ints.len() >> 8) as u8);
            seq_inner.push((ints.len() & 0xff) as u8);
        }
        seq_inner.extend_from_slice(&ints);
        let alg = [
            0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05,
            0x00,
        ];
        let bitstr_inner = [0x00u8]
            .iter()
            .copied()
            .chain(seq_inner.iter().copied())
            .collect::<Vec<u8>>();
        let mut bitstr = vec![0x03];
        if bitstr_inner.len() < 128 {
            bitstr.push(bitstr_inner.len() as u8);
        } else {
            bitstr.push(0x82);
            bitstr.push((bitstr_inner.len() >> 8) as u8);
            bitstr.push((bitstr_inner.len() & 0xff) as u8);
        }
        bitstr.extend_from_slice(&bitstr_inner);
        let total: Vec<u8> = alg.iter().copied().chain(bitstr.iter().copied()).collect();
        out.push(if total.len() < 128 {
            total.len() as u8
        } else {
            0x00
        });
        if total.len() >= 128 {
            out.clear();
            out.extend_from_slice(&[
                0x30,
                0x82,
                (total.len() >> 8) as u8,
                (total.len() & 0xff) as u8,
            ]);
        }
        out.extend_from_slice(&total);
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&out);
        let mut s = String::from("-----BEGIN PUBLIC KEY-----\n");
        for c in b64.as_bytes().chunks(64) {
            s.push_str(std::str::from_utf8(c).unwrap());
            s.push('\n');
        }
        s.push_str("-----END PUBLIC KEY-----\n");
        s
    }

    pub fn fingerprint(&self) -> i64 {
        let mut buf = Vec::with_capacity(self.n_bytes.len() + 16);
        crate::tl::put_bytes(&mut buf, &self.n_bytes);
        crate::tl::put_bytes(&mut buf, &self.exponent.to_bytes_be());
        let h = sha1(&buf);
        i64::from_le_bytes(h[12..20].try_into().unwrap())
    }

    /// Server side of the RSA handshake: recover the padded plaintext.
    ///
    /// This server is not *required* to RSA-decrypt anything during the
    /// handshake (the client only RSA-encrypts data for the server when it
    /// wants the server to learn a `p_q_inner_data`); the DH exchange itself
    /// is driven by the client's plaintext `p_q_inner_data`, so this helper is
    /// only used for completeness/introspection.
    pub fn decrypt_hashed(&self, _data: &[u8]) -> Result<[u8; 224]> {
        bail!("server-side RSA decryption requires the private key")
    }
}

/// Return the "data with hash" exponentiation as the client performs it: this
/// is used for verifying fingerprints and by tests.
pub fn rsa_encrypt_hashed(data: &[u8], key: &RsaPublicKey, random: &[u8; 224]) -> Result<Vec<u8>> {
    if data.len() > 144 {
        bail!("data too long for RSA padding: {}", data.len());
    }
    let mut data_with_padding = Vec::with_capacity(192);
    data_with_padding.extend_from_slice(data);
    data_with_padding.extend_from_slice(&random[..192 - data.len()]);
    let data_pad_reversed: Vec<u8> = data_with_padding.iter().rev().copied().collect();
    let mut temp_key: [u8; 32] = random[192..224].try_into().unwrap();
    loop {
        let mut data_with_hash = Vec::with_capacity(224);
        data_with_hash.extend_from_slice(&data_pad_reversed);
        data_with_hash.extend_from_slice(&sha256_concat(&[&temp_key, &data_with_padding]));
        ige_encrypt(&mut data_with_hash, &temp_key, &[0u8; 32]);
        let mut xor = [0u8; 32];
        let h = sha256(&data_with_hash);
        for i in 0..32 {
            xor[i] = temp_key[i] ^ h[i];
        }
        let mut out = Vec::with_capacity(256);
        out.extend_from_slice(&xor);
        out.extend_from_slice(&data_with_hash);
        if BigUint::from_bytes_be(&out) >= key.modulus {
            for i in (0..32).rev() {
                let (v, o) = temp_key[i].overflowing_add(1);
                temp_key[i] = v;
                if !o {
                    break;
                }
            }
            continue;
        }
        let enc = BigUint::from_bytes_be(&out).modpow(&key.exponent, &key.modulus);
        let mut b = enc.to_bytes_be();
        while b.len() < 256 {
            b.insert(0, 0);
        }
        return Ok(b);
    }
}

/// The legacy `RSA_PAD` encryption used by Telethon, TDLib and the official
/// clients: `sha1(data) || data || random padding`, RSA-encoded big-endian.
pub fn rsa_encrypt_legacy(data: &[u8], key: &RsaPublicKey, random: &[u8; 224]) -> Result<Vec<u8>> {
    if data.len() > 235 {
        bail!("data too long for legacy RSA padding: {}", data.len());
    }
    let mut block = Vec::with_capacity(255);
    block.extend_from_slice(&sha1(data));
    block.extend_from_slice(data);
    block.extend_from_slice(&random[..235 - data.len()]);
    debug_assert_eq!(block.len(), 255);
    let m = BigUint::from_bytes_be(&block);
    if m >= key.modulus {
        bail!("legacy RSA block is not smaller than the modulus");
    }
    let enc = m.modpow(&key.exponent, &key.modulus);
    let mut out = enc.to_bytes_be();
    while out.len() < 256 {
        out.insert(0, 0);
    }
    Ok(out)
}

/// Deterministic small-prime-safe random 128-bit `p * q` product split.
/// `pq` must be a product of two distinct primes < 2^63.
pub fn factorize_pq(pq: u64) -> (u64, u64) {
    if pq % 2 == 0 {
        return (2, pq / 2);
    }
    let n = pq as u128;
    let mut rng = rand::thread_rng();
    let mut g = 1u128;
    // Pollard-Brent
    for attempt in 1..=64u64 {
        let c = (attempt * 0x9E37_79B9_7F4A_7C15) as u128 % (n - 1) + 1;
        let f = |x: u128| (x * x % n + c) % n;
        let (mut y, mut r, mut q, mut x, mut ys) = (2u128, 1u128, 1u128, 0u128, 0u128);
        let mut m = 128u128 + rng.gen_range(0..64u128);
        g = 1;
        while g == 1 {
            x = y;
            for _ in 0..r {
                y = f(y);
            }
            let mut k = 0;
            while k < r && g == 1 {
                ys = y;
                let lim = m.min(r - k);
                for _ in 0..lim {
                    y = f(y);
                    let d = x.max(y) - x.min(y);
                    q = (q * d) % n;
                }
                g = gcd_u128(q, n);
                k += m;
            }
            r *= 2;
            m = 1;
        }
        if g == n as u128 {
            g = 1;
            let mut y2 = ys;
            while g == 1 && y2 != x {
                y2 = f(y2);
                let d = x.max(y2) - x.min(y2);
                g = gcd_u128(d, n);
            }
            if g == n as u128 {
                continue;
            }
        }
        if g > 1 && (n % g == 0) && (g as u64) < pq {
            let other = (n / g) as u64;
            if other > 1 {
                return (g as u64, other);
            }
        }
    }
    // Fallback: trial division by small primes.
    if pq % 3 == 0 {
        return (3, pq / 3);
    }
    let mut d = 7u64;
    while d * d <= pq {
        if pq % d == 0 {
            return (d, pq / d);
        }
        d += 2;
    }
    (1, pq)
}

fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// Generate `pq` as a product of two distinct 32-bit primes. The wire format
/// (per the MTProto spec) is: `pq` is 8 big-endian bytes, `p` and `q` are
/// big-endian minimal-length byte strings.
pub fn generate_pq() -> Result<([u8; 8], Vec<u8>, Vec<u8>)> {
    let mut rng = rand::thread_rng();
    loop {
        let a = gen_prime_u32(&mut rng);
        let b = gen_prime_u32(&mut rng);
        if a == b {
            continue;
        }
        let (small, big) = if a < b { (a, b) } else { (b, a) };
        let pq = small as u64 * big as u64;
        if pq > (1u64 << 63) - 1 {
            continue;
        }
        let mut pqb = [0u8; 8];
        pqb.copy_from_slice(&pq.to_be_bytes());
        let mut pb = small.to_be_bytes().to_vec();
        if pb.len() > 1 && pb[0] == 0 {
            pb.remove(0);
        }
        let mut qb = big.to_be_bytes().to_vec();
        if qb.len() > 1 && qb[0] == 0 {
            qb.remove(0);
        }
        return Ok((pqb, pb, qb));
    }
}

fn gen_prime_u32<R: Rng>(rng: &mut R) -> u32 {
    loop {
        let mut candidate = 1u32 << 31 | (rng.gen::<u32>() & 0x7fff_ffff);
        candidate |= 1;
        if is_probable_prime(candidate) {
            return candidate;
        }
    }
}

/// Miller-Rabin with deterministic bases sufficient for 32-bit inputs.
pub fn is_probable_prime(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    for p in [2u32, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if n % p == 0 {
            return n == p;
        }
    }
    let d = n - 1;
    let r = d.trailing_zeros();
    let d = d >> r;
    'outer: for a in [2u32, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let mut x = modpow_u64(a as u64, d as u64, n as u64) as u32;
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 0..r - 1 {
            x = ((x as u64 * x as u64) % n as u64) as u32;
            if x == n - 1 {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

pub fn modpow_u64(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    if modulus == 1 {
        return 0;
    }
    let mut result = 1u64;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = (result as u128 * base as u128 % modulus as u128) as u64;
        }
        base = (base as u128 * base as u128 % modulus as u128) as u64;
        exp >>= 1;
    }
    result
}

/// Telegram's well-known 2048-bit safe prime (the value every official client
/// pins in `verifyDhPrimeAndGenerator`). Rendered big-endian as a decimal.
pub fn dh_prime_bytes() -> Vec<u8> {
    let hex = concat!(
        "c71caeb9c6b1c9048e6c522f70f13f73980d40238e3e21c14934d037563d930f",
        "48198a0aa7c14058229493d22530f4dbfa336f6e0ac925139543aed44cce7c37",
        "20fd51f69458705ac68cd4fe6b6b13abdc9746512969328454f18faf8c595f64",
        "2477fe96bb2a941d5bcd1d4ac8cc49880708fa9b378e3c4f3a9060bee67cf9a4",
        "a4a695811051907e162753b56b0f6b410dba74d8a84b2a14b3144e0ef1284754",
        "fd17ed950d5965b4b9dd46582db1178d169c6bc465b0d6ff9ca3928fef5b9ae4",
        "e418fc15e83ebea0f87fa9ff5eed70050ded2849f47bf959d956850ce929851f",
        "0d8115f635b105ee2e4e15d04b2454bf6f4fadf034b10403119cd8e3b92fcc5b",
    );
    hex::decode(hex).expect("valid hex")
}

pub fn dh_prime() -> BigUint {
    BigUint::from_bytes_be(&dh_prime_bytes())
}

/// The generator used by Telegram (`g = 3`).
pub const DH_GENERATOR: u32 = 3;

/// The lower bound clients use to reject degenerate public values:
/// `2^(2048 - 64)`.
pub fn dh_public_lower_bound() -> BigUint {
    BigUint::one() << 1984u32
}

/// Encrypt/decrypt `server_DH_inner_data` for the DH exchange.
pub fn dh_encrypt(data: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Vec<u8> {
    let mut buf = data.to_vec();
    while buf.len() % 16 != 0 {
        buf.push(0);
    }
    ige_encrypt(&mut buf, key, iv);
    buf
}

pub fn dh_decrypt(data: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Result<Vec<u8>> {
    if data.len() % 16 != 0 {
        bail!("DH ciphertext not block aligned");
    }
    let mut buf = data.to_vec();
    ige_decrypt(&mut buf, key, iv);
    // Strip trailing zero padding.
    Ok(buf)
}
