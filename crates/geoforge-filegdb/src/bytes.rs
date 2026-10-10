//! Bounds-checked little-endian reader with FileGDB varint support.

use geoforge_core::{Error, Result};

pub(crate) fn err(message: impl Into<String>) -> Error {
    Error::format("FileGDB", message)
}

pub(crate) struct Reader<'a> {
    b: &'a [u8],
    pub p: usize,
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.p)
    }

    pub fn peek(&self) -> Option<u8> {
        self.b.get(self.p).copied()
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = self
            .b
            .get(
                self.p
                    ..self
                        .p
                        .checked_add(n)
                        .ok_or_else(|| err("length overflow"))?,
            )
            .ok_or_else(|| err("unexpected end of data"))?;
        self.p += n;
        Ok(s)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.bytes(N)?.try_into().unwrap())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.array()?))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    /// 7 bits per byte, least significant first; msb set = more bytes follow.
    pub fn varuint(&mut self) -> Result<u64> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            if shift >= 64 {
                return Err(err("varuint too long"));
            }
            v |= u64::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
        }
    }

    /// Like `varuint`, but the first byte holds a sign bit (0x40) and 6 value bits.
    pub fn varint(&mut self) -> Result<i64> {
        let b = self.u8()?;
        let negative = b & 0x40 != 0;
        let mut v = u64::from(b & 0x3F);
        if b & 0x80 != 0 {
            let mut shift = 6;
            loop {
                let b = self.u8()?;
                if shift >= 64 {
                    return Err(err("varint too long"));
                }
                v |= u64::from(b & 0x7F) << shift;
                if b & 0x80 == 0 {
                    break;
                }
                shift += 7;
            }
        }
        let v = v as i64;
        Ok(if negative { -v } else { v })
    }

    /// A `varuint` used as a count, checked against the bytes left (`min_bytes` each).
    pub fn count(&mut self, min_bytes: usize) -> Result<usize> {
        let n = self.varuint()?;
        let n = usize::try_from(n).map_err(|_| err("count overflow"))?;
        if n.saturating_mul(min_bytes) > self.remaining() {
            return Err(err(format!("count {n} exceeds remaining data")));
        }
        Ok(n)
    }

    /// `n` UTF-16LE code units.
    pub fn utf16(&mut self, n: usize) -> Result<String> {
        let raw = self.bytes(n * 2)?;
        let units: Vec<u16> = raw
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&units))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints() {
        assert_eq!(Reader::new(&[0x05]).varuint().unwrap(), 5);
        assert_eq!(Reader::new(&[0x96, 0x01]).varuint().unwrap(), 150);
        assert_eq!(Reader::new(&[0x42]).varint().unwrap(), -2);
        assert_eq!(Reader::new(&[0x05]).varint().unwrap(), 5);
        // 0x81 0x01: 6 bits (1) + 1 << 6 = 65
        assert_eq!(Reader::new(&[0x81, 0x01]).varint().unwrap(), 65);
        assert_eq!(Reader::new(&[0xC1, 0x01]).varint().unwrap(), -65);
    }
}
