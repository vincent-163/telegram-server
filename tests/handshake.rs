use grammers_tl_types as tl;
use grammers_tl_types::{Deserializable, Serializable};
use num_bigint::{BigUint, RandBigInt};
use num_traits::One;
use rand::RngCore;
use telegram_server::crypto::{
    dh_aes_key_iv, factorize_pq, ige_encrypt, rsa_encrypt_hashed, rsa_encrypt_legacy, sha1,
    RsaPublicKey,
};
use telegram_server::mtproto::{Handshake, RsaKeyPair};

/// `resPQ` must echo the client's nonce (not the placeholder the handshake
/// object was constructed with) and must be serialized with its constructor
/// id, which clients use to recognize the reply.
#[test]
fn res_pq_echoes_client_nonce_and_carries_ctor() {
    let key = RsaKeyPair::generate();
    let mut handshake = Handshake::new([0u8; 16]);
    let client_nonce = [0xAB; 16];
    handshake.set_nonce(client_nonce);

    let res_pq = handshake.step1(&key).unwrap();
    let bytes = res_pq.to_bytes();
    assert_eq!(
        u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        0x05162463,
        "resPQ must be serialized with its constructor id"
    );
    assert_eq!(
        &bytes[4..20],
        &client_nonce[..],
        "resPQ must echo the client nonce"
    );

    let tl::enums::ResPq::Pq(inner) = res_pq;
    assert_eq!(inner.nonce, client_nonce);
    assert_ne!(inner.server_nonce, client_nonce, "server nonce must differ");
    assert_eq!(inner.pq.len(), 8, "pq must be an 8-byte big-endian value");
    assert_eq!(
        inner.server_public_key_fingerprints,
        vec![key.fingerprint()]
    );
}

#[test]
fn fake_connection_check_can_restart_for_real_pq_request() {
    let key = RsaKeyPair::generate();
    let mut handshake = Handshake::new([0x33; 16]);
    handshake.step1(&key).unwrap();

    handshake.restart_for_pq();
    handshake.set_nonce([0x44; 16]);
    let tl::enums::ResPq::Pq(res_pq) = handshake.step1(&key).unwrap();
    assert_eq!(res_pq.nonce, [0x44; 16]);
}

#[test]
fn complete_authorization_handshake() {
    let key = RsaKeyPair::generate();
    let mut handshake = Handshake::new([0x11; 16]);
    let res_pq = handshake.step1(&key).unwrap();
    let tl::enums::ResPq::Pq(res_pq) = res_pq;
    assert_eq!(res_pq.nonce, [0x11; 16]);
    assert_eq!(
        res_pq.server_public_key_fingerprints,
        vec![key.fingerprint()]
    );

    let pq_bytes: [u8; 8] = res_pq.pq.clone().try_into().unwrap();
    let pq = u64::from_be_bytes(pq_bytes);
    let (p, q) = factorize_pq(pq);
    let mut p_bytes = p.to_be_bytes().to_vec();
    while p_bytes.len() > 1 && p_bytes[0] == 0 {
        p_bytes.remove(0);
    }
    let mut q_bytes = q.to_be_bytes().to_vec();
    while q_bytes.len() > 1 && q_bytes[0] == 0 {
        q_bytes.remove(0);
    }

    let new_nonce = [0x22; 32];
    let pq_inner = tl::types::PQInnerData {
        pq: res_pq.pq.clone(),
        p: p_bytes.clone(),
        q: q_bytes.clone(),
        nonce: res_pq.nonce,
        server_nonce: res_pq.server_nonce,
        new_nonce,
    };
    let public_key = RsaPublicKey::from_bytes(&key.n_bytes, 65537).unwrap();
    assert_eq!(public_key.fingerprint(), key.fingerprint());
    let mut random = [0u8; 224];
    rand::thread_rng().fill_bytes(&mut random);
    // The real clients (Telethon, TDLib, official apps) use the legacy
    // `sha1(data) || data || padding` scheme, and serialize the object *with*
    // its constructor id; exercise that path here.
    let pq_inner_bytes = tl::enums::PQInnerData::Data(pq_inner.clone()).to_bytes();
    let encrypted_pq_inner = rsa_encrypt_legacy(&pq_inner_bytes, &public_key, &random).unwrap();

    let req_dh = tl::functions::ReqDhParams {
        nonce: res_pq.nonce,
        server_nonce: res_pq.server_nonce,
        p: p_bytes,
        q: q_bytes,
        public_key_fingerprint: key.fingerprint(),
        encrypted_data: encrypted_pq_inner,
    };
    let server_dh_params = handshake.step2(&req_dh, &key).unwrap();
    let server_dh_params =
        tl::enums::ServerDhParams::from_bytes(&server_dh_params.to_bytes()).unwrap();
    let server_dh = match server_dh_params {
        tl::enums::ServerDhParams::Ok(value) => value,
        other => panic!("expected server_DH_params_ok, got {other:?}"),
    };
    assert_eq!(server_dh.nonce, res_pq.nonce);
    assert_eq!(server_dh.server_nonce, res_pq.server_nonce);

    let (key2, iv2) = dh_aes_key_iv(&server_dh.server_nonce, &new_nonce);
    let mut answer = server_dh.encrypted_answer.clone();
    telegram_server::crypto::ige_decrypt(&mut answer, &key2, &iv2);
    // The server emits the inner data *with* its constructor id, and the hash
    // covers those bytes too, so parse through the enum.
    let mut answer_cursor = tl::Cursor::from_slice(&answer[20..]);
    let server_inner = match tl::enums::ServerDhInnerData::deserialize(&mut answer_cursor).unwrap()
    {
        tl::enums::ServerDhInnerData::Data(v) => v,
    };
    let answer_len = 20 + answer_cursor.pos();
    assert_eq!(answer[..20], sha1(&answer[20..answer_len]));
    assert_eq!(server_inner.nonce, res_pq.nonce);
    assert_eq!(server_inner.server_nonce, res_pq.server_nonce);
    assert_eq!(server_inner.g, 3);

    let dh_prime = BigUint::from_bytes_be(&server_inner.dh_prime);
    let server_public = BigUint::from_bytes_be(&server_inner.g_a);
    let client_secret =
        rand::thread_rng().gen_biguint_range(&BigUint::one(), &(&dh_prime - BigUint::one()));
    let client_public = BigUint::from(3u32).modpow(&client_secret, &dh_prime);
    let lower = telegram_server::crypto::dh_public_lower_bound();
    assert!(client_public > BigUint::one() && client_public < dh_prime);
    assert!(client_public >= lower);
    assert!((&dh_prime - &client_public) >= lower);

    let client_inner = tl::types::ClientDhInnerData {
        nonce: res_pq.nonce,
        server_nonce: res_pq.server_nonce,
        retry_id: 0,
        g_b: client_public.to_bytes_be(),
    };
    // Real clients hash the object including its constructor id.
    let client_inner = tl::enums::ClientDhInnerData::Data(client_inner).to_bytes();
    let mut client_answer = Vec::with_capacity(client_inner.len() + 32);
    client_answer.extend_from_slice(&sha1(&client_inner));
    client_answer.extend_from_slice(&client_inner);
    while client_answer.len() % 16 != 0 {
        client_answer.push(0);
    }
    ige_encrypt(&mut client_answer, &key2, &iv2);

    let req_client_dh = tl::functions::SetClientDhParams {
        nonce: res_pq.nonce,
        server_nonce: res_pq.server_nonce,
        encrypted_data: client_answer,
    };
    let (answer, server_auth_key, salt) = handshake.step3(&req_client_dh).unwrap();
    let client_auth_key = server_public.modpow(&client_secret, &dh_prime);
    let mut expected_auth_key = [0u8; 256];
    let auth_bytes = client_auth_key.to_bytes_be();
    assert!(auth_bytes.len() <= 256);
    let skip = 256 - auth_bytes.len();
    expected_auth_key[skip..].copy_from_slice(&auth_bytes);
    assert_eq!(server_auth_key, expected_auth_key);

    let aux_hash = sha1(&server_auth_key);
    let mut nonce_input = Vec::with_capacity(41);
    nonce_input.extend_from_slice(&new_nonce);
    nonce_input.push(1);
    nonce_input.extend_from_slice(&aux_hash[..8]);
    let expected_nonce_hash = sha1(&nonce_input)[4..20].to_vec();
    match answer {
        tl::enums::SetClientDhParamsAnswer::DhGenOk(ok) => {
            assert_eq!(ok.nonce, res_pq.nonce);
            assert_eq!(ok.server_nonce, res_pq.server_nonce);
            assert_eq!(ok.new_nonce_hash1, expected_nonce_hash[..]);
        }
        other => panic!("expected dh_gen_ok, got {other:?}"),
    }

    let expected_salt = i64::from_le_bytes(
        (0..8)
            .map(|i| new_nonce[i] ^ res_pq.server_nonce[i])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
    );
    assert_eq!(salt, expected_salt);
}

#[test]
fn official_rsa_fingerprint_matches_tdesktop_format() {
    let modulus = hex::decode(
        "e8bb3305c0b52c6cf2afdf7637313489e63e05268e5badb601af417786472e5f\
         93b85438968e20e6729a301c0afc121bf7151f834436f7fda680847a66bf64ac\
         cec78ee21c0b316f0edafe2f41908da7bd1f4a5107638eeb67040ace472a14f\
         90d9f7c2b7def99688ba3073adb5750bb02964902a359fe745d8170e36876d4\
         fd8a5d41b2a76cbff9a13267eb9580b2d06d10357448d20d9da2191cb5d8c93\
         982961cdfdeda629e37f1fb09a0722027696032fe61ed663db7a37f6f263d37\
         0f69db53a0dc0a1748bdaaff6209d5645485e6e001d1953255757e4b8e42813\
         347b11da6ab500fd0ace7e6dfa3736199ccaf9397ed0745a427dcfa6cd67bc\
         b1acff3",
    )
    .unwrap();
    let key = RsaPublicKey::from_bytes(&modulus, 0x010001).unwrap();
    assert_eq!(key.fingerprint() as u64, 0xd09d1d85de64fd85);
}

/// Minimal DER walker: returns `(tag, content)` for one TLV at `i`.
fn der_read(buf: &[u8], i: &mut usize) -> (u8, Vec<u8>) {
    let tag = buf[*i];
    *i += 1;
    let mut len = buf[*i] as usize;
    *i += 1;
    if len & 0x80 != 0 {
        let n = len & 0x7f;
        len = 0;
        for _ in 0..n {
            len = (len << 8) | buf[*i] as usize;
            *i += 1;
        }
    }
    let out = buf[*i..*i + len].to_vec();
    *i += len;
    (tag, out)
}

/// Extract the two INTEGERs (modulus, exponent) from a PKCS#1 RSAPublicKey
/// sequence.
fn pkcs1_ints(der: &[u8]) -> Vec<Vec<u8>> {
    let mut i = 0;
    let (tag, seq) = der_read(der, &mut i);
    assert_eq!(tag, 0x30, "expected a SEQUENCE");
    assert_eq!(i, der.len(), "trailing bytes after the SEQUENCE");
    let mut j = 0;
    let mut out = Vec::new();
    while j < seq.len() {
        let (t, v) = der_read(&seq, &mut j);
        assert_eq!(t, 0x02, "expected an INTEGER");
        out.push(v);
    }
    out
}

/// Unwrap a SubjectPublicKeyInfo into its inner PKCS#1 sequence.
fn spki_inner(der: &[u8]) -> Vec<u8> {
    let mut i = 0;
    let (tag, seq) = der_read(der, &mut i);
    assert_eq!(tag, 0x30, "expected an outer SEQUENCE");
    let mut j = 0;
    let (alg_tag, _alg) = der_read(&seq, &mut j);
    assert_eq!(alg_tag, 0x30, "expected an AlgorithmIdentifier SEQUENCE");
    let (bs_tag, bits) = der_read(&seq, &mut j);
    assert_eq!(bs_tag, 0x03, "expected a BIT STRING");
    assert_eq!(bits[0], 0x00, "BIT STRING must have no unused bits");
    bits[1..].to_vec()
}

fn pem_der(pem: &str) -> Vec<u8> {
    use base64::Engine;
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .expect("PEM body must be valid base64")
}

/// The advertised PEMs must be parseable as positive DER INTEGERs. A
/// 2048-bit modulus always has its high bit set, so omitting the DER sign
/// byte makes both encodings describe a negative number, which standard RSA
/// parsers reject outright.
#[test]
fn advertised_public_keys_are_valid_der() {
    let key = RsaKeyPair::generate();

    let pkcs1 = pkcs1_ints(&pem_der(&key.public_pem_rsa()));
    let spki = pkcs1_ints(&spki_inner(&pem_der(&key.public_pem())));
    assert_eq!(pkcs1, spki, "both encodings must describe the same key");

    for (idx, v) in pkcs1.iter().enumerate() {
        assert!(!v.is_empty(), "INTEGER {idx} must not be empty");
        assert!(
            v[0] & 0x80 == 0,
            "INTEGER {idx} must be non-negative (missing DER sign byte)"
        );
    }
    // The encoded length must match the value: a 256-byte modulus needs one
    // extra sign byte exactly when its high bit is set.
    let n = &pkcs1[0];
    let expected = if n[0] == 0x00 { 257 } else { 256 };
    assert_eq!(n.len(), expected, "modulus DER length must match its value");
    assert!(n.len() >= 256, "modulus must be at least 2048 bits");

    // A modulus with the high bit set is the shape real Telegram server keys
    // have, and the one that used to be encoded as a negative number.
    let high_bit = if n[0] == 0x00 { n[1] } else { n[0] };
    assert_eq!(
        high_bit & 0x80,
        0x80,
        "a 2048-bit modulus always has its high bit set"
    );
}
