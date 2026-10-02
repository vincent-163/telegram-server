//! Minimal TL (Type Language) binary serialization helpers and a safe cursor
//! used both for hand-written MTProto structures and by the RPC dispatcher.

use anyhow::{anyhow, bail, Result};

#[inline]
pub fn read_u32(buf: &[u8], off: usize) -> Result<u32> {
    if off + 4 > buf.len() {
        bail!("read_u32 out of bounds ({} + 4 > {})", off, buf.len());
    }
    Ok(u32::from_le_bytes(buf[off..off + 4].try_into().unwrap()))
}

#[inline]
pub fn read_i32(buf: &[u8], off: usize) -> Result<i32> {
    Ok(read_u32(buf, off)? as i32)
}

#[inline]
pub fn read_u64(buf: &[u8], off: usize) -> Result<u64> {
    if off + 8 > buf.len() {
        bail!("read_u64 out of bounds");
    }
    Ok(u64::from_le_bytes(buf[off..off + 8].try_into().unwrap()))
}

#[inline]
pub fn read_i64(buf: &[u8], off: usize) -> Result<i64> {
    Ok(read_u64(buf, off)? as i64)
}

#[inline]
pub fn read_i128(buf: &[u8], off: usize) -> Result<[u8; 16]> {
    if off + 16 > buf.len() {
        bail!("read_i128 out of bounds");
    }
    Ok(buf[off..off + 16].try_into().unwrap())
}

#[inline]
pub fn read_i256(buf: &[u8], off: usize) -> Result<[u8; 32]> {
    if off + 32 > buf.len() {
        bail!("read_i256 out of bounds");
    }
    Ok(buf[off..off + 32].try_into().unwrap())
}

/// A cursor over a TL byte stream. All reads are bounds checked.
pub struct Reader<'a> {
    pub buf: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    #[inline]
    pub fn u32(&mut self) -> Result<u32> {
        let v = read_u32(self.buf, self.pos)?;
        self.pos += 4;
        Ok(v)
    }

    #[inline]
    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }

    #[inline]
    pub fn u64(&mut self) -> Result<u64> {
        let v = read_u64(self.buf, self.pos)?;
        self.pos += 8;
        Ok(v)
    }

    #[inline]
    pub fn i64(&mut self) -> Result<i64> {
        Ok(self.u64()? as i64)
    }

    #[inline]
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.u64()?))
    }

    #[inline]
    pub fn i128(&mut self) -> Result<[u8; 16]> {
        let v = read_i128(self.buf, self.pos)?;
        self.pos += 16;
        Ok(v)
    }

    #[inline]
    pub fn i256(&mut self) -> Result<[u8; 32]> {
        let v = read_i256(self.buf, self.pos)?;
        self.pos += 32;
        Ok(v)
    }

    #[inline]
    pub fn bytes_raw(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos + n > self.buf.len() {
            bail!("bytes_raw out of bounds");
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    /// TL `bytes`: first byte (or 0xfe+3 bytes) length, then payload, then padding.
    pub fn bytes(&mut self) -> Result<Vec<u8>> {
        let first = self.bytes_raw(1)?[0];
        let (len, hdr) = if first == 254 {
            let b = self.bytes_raw(3)?;
            (u32::from_le_bytes([b[0], b[1], b[2], 0]) as usize, 4usize)
        } else {
            (first as usize, 1usize)
        };
        let data = self.bytes_raw(len)?.to_vec();
        let total = hdr + len;
        let pad = (4 - (total % 4)) % 4;
        if pad > 0 {
            self.bytes_raw(pad)?;
        }
        Ok(data)
    }

    /// TL `string`: like bytes, but semantically UTF-8.
    pub fn string(&mut self) -> Result<String> {
        let b = self.bytes()?;
        Ok(String::from_utf8_lossy(&b).into_owned())
    }

    /// Peek the constructor id without consuming it.
    pub fn peek_ctor(&self) -> Result<u32> {
        read_u32(self.buf, self.pos)
    }

    /// Read a vector<T> where T is a bare constructor id + payload.
    pub fn vector_raw(&mut self) -> Result<Vec<Vec<u8>>> {
        let ctor = self.u32()?;
        if ctor != 0x1cb5c415 {
            bail!("expected vector#1cb5c415, got {:#010x}", ctor);
        }
        let n = self.u32()? as usize;
        if n > 1_000_000 {
            bail!("vector too large: {}", n);
        }
        let mut out = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            let start = self.pos;
            // We cannot know the element length generically; callers that need
            // concrete types should parse directly. This helper exists for
            // vectors of fixed-size scalars (ints, longs, strings).
            let _ = start;
            bail!("vector_raw requires a concrete element width; use typed helpers");
        }
        #[allow(unreachable_code)]
        Ok(out)
    }

    pub fn vector_long(&mut self) -> Result<Vec<i64>> {
        let ctor = self.u32()?;
        if ctor != 0x1cb5c415 {
            bail!("expected vector#1cb5c415, got {:#010x}", ctor);
        }
        let n = self.u32()? as usize;
        if n > 1_000_000 {
            bail!("vector too large: {}", n);
        }
        let mut out = Vec::with_capacity(n.min(65536));
        for _ in 0..n {
            out.push(self.i64()?);
        }
        Ok(out)
    }

    pub fn vector_int(&mut self) -> Result<Vec<i32>> {
        let ctor = self.u32()?;
        if ctor != 0x1cb5c415 {
            bail!("expected vector#1cb5c415, got {:#010x}", ctor);
        }
        let n = self.u32()? as usize;
        if n > 1_000_000 {
            bail!("vector too large: {}", n);
        }
        let mut out = Vec::with_capacity(n.min(65536));
        for _ in 0..n {
            out.push(self.i32()?);
        }
        Ok(out)
    }

    pub fn vector_string(&mut self) -> Result<Vec<String>> {
        let ctor = self.u32()?;
        if ctor != 0x1cb5c415 {
            bail!("expected vector#1cb5c415, got {:#010x}", ctor);
        }
        let n = self.u32()? as usize;
        if n > 100_000 {
            bail!("vector too large: {}", n);
        }
        let mut out = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            out.push(self.string()?);
        }
        Ok(out)
    }
}

/// TL `bytes` writer.
pub fn put_bytes(buf: &mut Vec<u8>, data: &[u8]) {
    let len = data.len();
    if len < 254 {
        buf.push(len as u8);
    } else {
        buf.push(254);
        buf.extend_from_slice(&(len as u32).to_le_bytes()[..3]);
    }
    buf.extend_from_slice(data);
    let total = if len < 254 { 1 + len } else { 4 + len };
    let pad = (4 - (total % 4)) % 4;
    buf.extend(std::iter::repeat(0u8).take(pad));
}

pub fn put_string(buf: &mut Vec<u8>, s: &str) {
    put_bytes(buf, s.as_bytes());
}

pub fn put_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub fn put_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub fn put_i64(buf: &mut Vec<u8>, v: i64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub fn put_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub fn put_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_bits().to_le_bytes());
}

pub fn put_i128(buf: &mut Vec<u8>, v: &[u8; 16]) {
    buf.extend_from_slice(v);
}

pub fn put_i256(buf: &mut Vec<u8>, v: &[u8; 32]) {
    buf.extend_from_slice(v);
}

/// Serialize a vector of raw already-encoded elements.
pub fn put_raw_vector(buf: &mut Vec<u8>, elems: &[Vec<u8>]) {
    put_u32(buf, 0x1cb5c415);
    put_i32(buf, elems.len() as i32);
    for e in elems {
        buf.extend_from_slice(e);
    }
}

pub fn read_ctor(buf: &[u8]) -> Result<u32> {
    read_u32(buf, 0)
}

/// Convenience for `Ok(())`/error plumbing.
pub fn ensure(cond: bool, msg: &str) -> Result<()> {
    if cond {
        Ok(())
    } else {
        Err(anyhow!("{}", msg))
    }
}
