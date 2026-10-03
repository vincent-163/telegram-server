//! MTProto transport framing: abridged, intermediate, padded intermediate and
//! full, plus the transport-obfuscation layer that wraps all of them.

use aes::cipher::{KeyIvInit, StreamCipher};
use anyhow::{bail, Result};

pub const TRANSPORT_TAG_ABRIDGED: u32 = 0xefef_efef;
pub const TRANSPORT_TAG_INTERMEDIATE: u32 = 0xeeee_eeee;
pub const TRANSPORT_TAG_PADDED_INTERMEDIATE: u32 = 0xdddd_dddd;

/// First-four-byte values that mean "this is not an obfuscation header".
const FORBIDDEN_FIRST_INTS: [[u8; 4]; 7] = [
    [b'H', b'E', b'A', b'D'],
    [b'P', b'O', b'S', b'T'],
    [b'G', b'E', b'T', b' '],
    [b'O', b'P', b'T', b'I'],
    [0x16, 0x03, 0x01, 0x02],
    [0xdd, 0xdd, 0xdd, 0xdd],
    [0xee, 0xee, 0xee, 0xee],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Abridged,
    Intermediate,
    PaddedIntermediate,
    Full,
}

/// An AES-256-CTR stream that is reused for the lifetime of a connection.
pub struct Ctr {
    key: [u8; 32],
    counter: u128,
}

impl Ctr {
    pub fn new(key: &[u8; 32], iv: &[u8; 16]) -> Self {
        Ctr {
            key: *key,
            counter: u128::from_be_bytes(*iv),
        }
    }

    pub fn apply(&mut self, buf: &mut [u8]) {
        if buf.is_empty() {
            return;
        }
        let iv = self.counter.to_be_bytes();
        let mut cipher = ctr::Ctr128BE::<aes::Aes256>::new(&self.key.into(), &iv.into());
        cipher.apply_keystream(buf);
        let blocks = ((buf.len() as u128) + 15) / 16;
        self.counter = self.counter.wrapping_add(blocks);
    }
}

/// The two key/IV pairs defined by the obfuscation spec. The primary init is
/// used by whoever *generated* it for outbound traffic; the reversed init is
/// used for inbound traffic by that same side (and vice versa for the peer).
pub struct ObfKeys {
    pub forward_key: [u8; 32],
    pub forward_iv: [u8; 16],
    pub reverse_key: [u8; 32],
    pub reverse_iv: [u8; 16],
}

impl ObfKeys {
    pub fn from_init(init: &[u8; 64]) -> Self {
        let mut rev = *init;
        rev.reverse();
        let mut forward_key = [0u8; 32];
        forward_key.copy_from_slice(&init[8..40]);
        let mut forward_iv = [0u8; 16];
        forward_iv.copy_from_slice(&init[40..56]);
        let mut reverse_key = [0u8; 32];
        reverse_key.copy_from_slice(&rev[8..40]);
        let mut reverse_iv = [0u8; 16];
        reverse_iv.copy_from_slice(&rev[40..56]);
        ObfKeys {
            forward_key,
            forward_iv,
            reverse_key,
            reverse_iv,
        }
    }
}

/// Decodes a raw socket byte stream into complete MTProto payloads.
pub struct Decoder {
    buf: Vec<u8>,
    kind: Option<TransportKind>,
    inbound: Option<Ctr>,
    full_seq: u64,
    http: bool,
    obf_init: Option<[u8; 64]>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    pub fn new() -> Self {
        Decoder {
            buf: Vec::new(),
            kind: None,
            inbound: None,
            full_seq: 0,
            http: false,
            obf_init: None,
        }
    }

    pub fn kind(&self) -> Option<TransportKind> {
        self.kind
    }

    pub fn is_obfuscated(&self) -> bool {
        self.inbound.is_some()
    }

    pub fn is_http(&self) -> bool {
        self.http
    }

    pub fn obfuscation_init(&self) -> Option<[u8; 64]> {
        self.obf_init
    }

    pub fn push(&mut self, data: &[u8]) -> Result<()> {
        if let Some(c) = self.inbound.as_mut() {
            let mut data = data.to_vec();
            c.apply(&mut data);
            self.buf.extend_from_slice(&data);
        } else {
            self.buf.extend_from_slice(data);
        }
        Ok(())
    }

    /// Consume the transport (and obfuscation) header. Returns `false` when
    /// more bytes are needed.
    pub fn ensure_started(&mut self) -> Result<bool> {
        if self.kind.is_some() || self.http {
            return Ok(true);
        }
        if self.buf.is_empty() {
            return Ok(false);
        }
        if self.buf[0] == 0xef {
            self.buf.drain(..1);
            self.kind = Some(TransportKind::Abridged);
            return Ok(true);
        }
        if self.buf.len() < 4 {
            return Ok(false);
        }
        let first = u32::from_le_bytes(self.buf[0..4].try_into().unwrap());
        match first {
            TRANSPORT_TAG_INTERMEDIATE => {
                self.buf.drain(..4);
                self.kind = Some(TransportKind::Intermediate);
                return Ok(true);
            }
            TRANSPORT_TAG_PADDED_INTERMEDIATE => {
                self.buf.drain(..4);
                self.kind = Some(TransportKind::PaddedIntermediate);
                return Ok(true);
            }
            _ => {}
        }
        if FORBIDDEN_FIRST_INTS.iter().any(|f| *f == self.buf[0..4]) {
            self.http = true;
            return Ok(true);
        }
        if self.buf.len() < 8 {
            return Ok(false);
        }
        let second = u32::from_le_bytes(self.buf[4..8].try_into().unwrap());
        if second != 0 {
            return self.start_obfuscation();
        }
        // Plain full transport: length followed by TCP sequence number 0.
        if first >= 12 && first % 4 == 0 && first <= 16 * 1024 * 1024 {
            self.kind = Some(TransportKind::Full);
            return Ok(true);
        }
        self.start_obfuscation()
    }

    fn start_obfuscation(&mut self) -> Result<bool> {
        if self.buf.len() < 64 {
            return Ok(false);
        }
        let mut init = [0u8; 64];
        init.copy_from_slice(&self.buf[0..64]);
        self.obf_init = Some(init);
        let keys = ObfKeys::from_init(&init);
        let mut rx = Ctr::new(&keys.forward_key, &keys.forward_iv);
        let mut head = init;
        rx.apply(&mut head);
        let tag = u32::from_le_bytes(head[56..60].try_into().unwrap());
        self.buf.drain(..64);
        // The remainder arrived after the obfuscation header, so it must be
        // decrypted with the same CTR instance.
        let mut rest = self.buf.split_off(0);
        if rest.is_empty() {
            rest = Vec::new();
        }
        let mut tail = std::mem::take(&mut self.buf);
        rx.apply(&mut tail);
        let mut decrypted = tail;
        decrypted.append(&mut rest);
        self.buf = decrypted;
        self.inbound = Some(rx);
        self.kind = Some(match tag {
            TRANSPORT_TAG_ABRIDGED => TransportKind::Abridged,
            TRANSPORT_TAG_INTERMEDIATE => TransportKind::Intermediate,
            TRANSPORT_TAG_PADDED_INTERMEDIATE => TransportKind::PaddedIntermediate,
            other => bail!("unsupported obfuscation transport tag {:#010x}", other),
        });
        Ok(true)
    }

    /// Pull one complete payload, if available.
    pub fn next_payload(&mut self) -> Result<Option<Vec<u8>>> {
        let kind = match self.kind {
            Some(k) => k,
            None => return Ok(None),
        };
        if self.buf.is_empty() {
            return Ok(None);
        }
        match kind {
            TransportKind::Abridged => {
                let b = self.buf[0];
                let (offset, len) = if b < 0x7f {
                    (1usize, b as usize * 4)
                } else if b == 0x7f {
                    if self.buf.len() < 4 {
                        return Ok(None);
                    }
                    let len = self.buf[1] as usize
                        | ((self.buf[2] as usize) << 8)
                        | ((self.buf[3] as usize) << 16);
                    (4usize, len * 4)
                } else {
                    // Quick ACK token; the server does not expect one inbound.
                    if self.buf.len() >= 4 {
                        self.buf.drain(..4);
                    }
                    return Ok(None);
                };
                if len == 0 || len % 4 != 0 || self.buf.len() < offset + len {
                    return Ok(None);
                }
                let payload = self.buf[offset..offset + len].to_vec();
                self.buf.drain(..offset + len);
                Ok(Some(payload))
            }
            TransportKind::Intermediate | TransportKind::PaddedIntermediate => {
                if self.buf.len() < 4 {
                    return Ok(None);
                }
                let raw_len = u32::from_le_bytes(self.buf[0..4].try_into().unwrap());
                let len = (raw_len & 0x7fff_ffff) as usize;
                if len == 0 || len > 4 * 1024 * 1024 || self.buf.len() < 4 + len {
                    return Ok(None);
                }
                // Padded intermediate appends 0..15 random bytes after the
                // MTProto payload. The payload length is discoverable from the
                // MTProto framing itself (message_data_length for plain
                // messages, the 16-byte alignment for encrypted ones), so the
                // transport layer deliberately passes the padding through.
                let payload = self.buf[4..4 + len].to_vec();
                self.buf.drain(..4 + len);
                Ok(Some(payload))
            }
            TransportKind::Full => {
                if self.buf.len() < 8 {
                    return Ok(None);
                }
                let length = u32::from_le_bytes(self.buf[0..4].try_into().unwrap()) as usize;
                let seq = u32::from_le_bytes(self.buf[4..8].try_into().unwrap());
                if length < 12 || length % 4 != 0 || length > 16 * 1024 * 1024 {
                    bail!("invalid full transport length {}", length);
                }
                if self.buf.len() < length {
                    return Ok(None);
                }
                let crc = u32::from_le_bytes(self.buf[length - 4..length].try_into().unwrap());
                let mut hasher = crc32fast::Hasher::new();
                hasher.update(&self.buf[0..length - 4]);
                if hasher.finalize() != crc {
                    bail!("full transport CRC mismatch");
                }
                if u64::from(seq) != self.full_seq {
                    bail!(
                        "full transport seq mismatch: got {}, want {}",
                        seq,
                        self.full_seq
                    );
                }
                self.full_seq += 1;
                let payload = self.buf[8..length - 4].to_vec();
                self.buf.drain(..length);
                Ok(Some(payload))
            }
        }
    }
}

/// Encodes MTProto payloads for the selected transport.
pub struct Encoder {
    kind: TransportKind,
    outbound: Option<Ctr>,
    full_seq: u64,
    first: bool,
}

impl Encoder {
    pub fn new(kind: TransportKind) -> Self {
        Encoder {
            kind,
            outbound: None,
            full_seq: 0,
            first: true,
        }
    }

    /// On the server, outbound traffic uses the *reversed* init key.
    pub fn enable_obfuscation(&mut self, init: &[u8; 64]) {
        let keys = ObfKeys::from_init(init);
        self.outbound = Some(Ctr::new(&keys.reverse_key, &keys.reverse_iv));
    }

    pub fn preamble(&self) -> Vec<u8> {
        if self.outbound.is_some() {
            return Vec::new();
        }
        match self.kind {
            TransportKind::Abridged => vec![0xef],
            TransportKind::Intermediate => TRANSPORT_TAG_INTERMEDIATE.to_le_bytes().to_vec(),
            TransportKind::PaddedIntermediate => {
                TRANSPORT_TAG_PADDED_INTERMEDIATE.to_le_bytes().to_vec()
            }
            TransportKind::Full => Vec::new(),
        }
    }

    pub fn encode(&mut self, payload: &[u8]) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(payload.len() + 32);
        if self.first {
            out.extend_from_slice(&self.preamble());
            self.first = false;
        }
        match self.kind {
            TransportKind::Abridged => {
                if payload.len() % 4 != 0 {
                    bail!("abridged payload must be 4-byte aligned");
                }
                let q = payload.len() / 4;
                if q < 0x7f {
                    out.push(q as u8);
                } else {
                    out.push(0x7f);
                    out.push((q & 0xff) as u8);
                    out.push(((q >> 8) & 0xff) as u8);
                    out.push(((q >> 16) & 0xff) as u8);
                }
                out.extend_from_slice(payload);
            }
            TransportKind::Intermediate => {
                out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                out.extend_from_slice(payload);
            }
            TransportKind::PaddedIntermediate => {
                use rand::Rng;
                // Keep the padding under 4 bytes: then `frame_len % 4` and
                // `(frame_len - 24) % 16` both recover it for every reference
                // implementation (tweb, Android, tdesktop).
                let pad = rand::thread_rng().gen_range(0..4);
                out.extend_from_slice(&((payload.len() + pad) as u32).to_le_bytes());
                out.extend_from_slice(payload);
                let mut rng = rand::thread_rng();
                for _ in 0..pad {
                    out.push(rng.gen());
                }
            }
            TransportKind::Full => {
                let length = (payload.len() + 12) as u32;
                out.extend_from_slice(&length.to_le_bytes());
                // The sequence field is 4 bytes on the wire; `full_seq` is a
                // u64 counter, so it must be truncated rather than serialized
                // whole (writing 8 bytes shifts the payload by four).
                out.extend_from_slice(&(self.full_seq as u32).to_le_bytes());
                out.extend_from_slice(payload);
                let mut hasher = crc32fast::Hasher::new();
                hasher.update(&out);
                out.extend_from_slice(&hasher.finalize().to_le_bytes());
                self.full_seq += 1;
            }
        }
        if let Some(c) = self.outbound.as_mut() {
            c.apply(&mut out);
        }
        Ok(out)
    }
}

/// Build the 64-byte obfuscation initialization payload (client side).
pub fn make_obfuscation_init(tag: u32) -> [u8; 64] {
    use rand::RngCore;
    let mut init = [0u8; 64];
    loop {
        rand::thread_rng().fill_bytes(&mut init);
        let _first = u32::from_le_bytes(init[0..4].try_into().unwrap());
        let second = u32::from_le_bytes(init[4..8].try_into().unwrap());
        if init[0] == 0xef || FORBIDDEN_FIRST_INTS.iter().any(|f| *f == init[0..4]) {
            continue;
        }
        if second == 0 {
            continue;
        }
        break;
    }
    init[56..60].copy_from_slice(&tag.to_le_bytes());
    init
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abridged_roundtrip() {
        let mut enc = Encoder::new(TransportKind::Abridged);
        let payload = vec![7u8; 16];
        let frame = enc.encode(&payload).unwrap();
        assert_eq!(frame[0], 0xef);
        let mut dec = Decoder::new();
        dec.push(&frame).unwrap();
        assert!(dec.ensure_started().unwrap());
        assert_eq!(dec.next_payload().unwrap(), Some(payload));
    }

    #[test]
    fn intermediate_roundtrip() {
        let mut enc = Encoder::new(TransportKind::Intermediate);
        let payload = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let frame = enc.encode(&payload).unwrap();
        let mut dec = Decoder::new();
        dec.push(&frame).unwrap();
        assert!(dec.ensure_started().unwrap());
        assert_eq!(dec.next_payload().unwrap(), Some(payload));
    }

    #[test]
    fn padded_intermediate_roundtrip() {
        let mut enc = Encoder::new(TransportKind::PaddedIntermediate);
        let payload = vec![3u8; 32];
        let frame = enc.encode(&payload).unwrap();
        let mut dec = Decoder::new();
        dec.push(&frame).unwrap();
        assert!(dec.ensure_started().unwrap());
        let got = dec.next_payload().unwrap().unwrap();
        assert!(got.starts_with(&payload));
        assert!(got.len() - payload.len() < 4);
    }

    #[test]
    fn full_roundtrip() {
        let mut enc = Encoder::new(TransportKind::Full);
        let payload = vec![5u8; 40];
        let frame = enc.encode(&payload).unwrap();
        let length = u32::from_le_bytes(frame[0..4].try_into().unwrap()) as usize;
        assert_eq!(
            length,
            frame.len(),
            "length field must cover the whole frame"
        );
        assert_eq!(length, payload.len() + 12);
        let seq = u32::from_le_bytes(frame[4..8].try_into().unwrap());
        assert_eq!(seq, 0);
        let crc = u32::from_le_bytes(frame[length - 4..length].try_into().unwrap());
        assert_eq!(crc, crc32fast::hash(&frame[..length - 4]));

        let mut dec = Decoder::new();
        dec.push(&frame).unwrap();
        assert!(dec.ensure_started().unwrap());
        assert_eq!(dec.kind(), Some(TransportKind::Full));
        assert_eq!(
            dec.next_payload().unwrap().as_deref(),
            Some(payload.as_slice())
        );

        // A second frame must carry the next sequence number.
        let frame2 = enc.encode(&payload).unwrap();
        assert_eq!(u32::from_le_bytes(frame2[4..8].try_into().unwrap()), 1);
    }

    #[test]
    fn obfuscated_roundtrip() {
        let init = make_obfuscation_init(TRANSPORT_TAG_PADDED_INTERMEDIATE);
        let keys = ObfKeys::from_init(&init);
        let mut client = Ctr::new(&keys.forward_key, &keys.forward_iv);
        let mut header = init;
        client.apply(&mut header);
        let mut tail = init;
        tail[56..64].copy_from_slice(&header[56..64]);

        let payload = vec![9u8; 32];
        let mut frame = Vec::new();
        frame.extend_from_slice(&((payload.len() + 7) as u32).to_le_bytes());
        frame.extend_from_slice(&payload);
        frame.extend_from_slice(&[0xaa; 7]);
        client.apply(&mut frame);

        let mut dec = Decoder::new();
        dec.push(&tail).unwrap();
        assert!(dec.ensure_started().unwrap());
        assert_eq!(dec.kind(), Some(TransportKind::PaddedIntermediate));
        assert!(dec.is_obfuscated());
        dec.push(&frame).unwrap();
        let got = dec.next_payload().unwrap().unwrap();
        assert!(got.starts_with(&payload));
        assert_eq!(got.len(), payload.len() + 7);

        // Server-side response uses the reversed key.
        let mut enc = Encoder::new(TransportKind::PaddedIntermediate);
        enc.enable_obfuscation(&init);
        let mut out = enc.encode(&payload).unwrap();
        // Emulate a client decoder: decrypt with reverse key.
        let mut rx = Ctr::new(&keys.reverse_key, &keys.reverse_iv);
        rx.apply(&mut out);
        let mut buf = out;
        buf.drain(..4);
        assert_eq!(buf[..payload.len()], payload[..]);
        // The trailing bytes are the 0..3 transport padding.
        assert!(buf.len() - payload.len() < 4);
    }
}
