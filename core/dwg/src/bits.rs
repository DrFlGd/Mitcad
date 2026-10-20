// SPDX-License-Identifier: MIT
//! The bit-coded values of a drawing (the specification's chapter 2): a
//! reader over a byte slice, most significant bit of each byte first, and a
//! writer of the same forms for the test files.

use crate::{DwgError, Version};

/// A handle reference: its code (the ownership kind, or how an offset
/// applies) and the absolute handle it refers to (0 for none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HandleRef {
    pub code: u8,
    pub handle: u64,
}

/// Reads bit-coded values. Positions are in bits from the start of the
/// slice; reads past `end` fail.
#[derive(Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
    pub version: Version,
}

fn eof(pos: usize) -> DwgError {
    DwgError::Corrupt(format!("bit data ends early (at bit {pos})"))
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], version: Version) -> Self {
        BitReader {
            data,
            pos: 0,
            end: data.len() * 8,
            version,
        }
    }

    /// A reader of `data` starting at bit `pos` and ending at bit `end`.
    pub fn at(data: &'a [u8], version: Version, pos: usize, end: usize) -> Self {
        BitReader {
            data,
            pos,
            end: end.min(data.len() * 8),
            version,
        }
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn set_position(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn set_end(&mut self, end: usize) {
        self.end = end.min(self.data.len() * 8);
    }

    pub fn remaining(&self) -> usize {
        self.end.saturating_sub(self.pos)
    }

    /// The byte offset of the position, rounded up to a whole byte.
    pub fn byte_position(&self) -> usize {
        self.pos.div_ceil(8)
    }

    pub fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }

    /// B: one bit.
    pub fn bit(&mut self) -> Result<bool, DwgError> {
        if self.pos >= self.end {
            return Err(eof(self.pos));
        }
        let b = self.data[self.pos / 8] >> (7 - self.pos % 8) & 1;
        self.pos += 1;
        Ok(b == 1)
    }

    /// `n` bits (at most 64), the first read the most significant.
    pub fn bits(&mut self, n: usize) -> Result<u64, DwgError> {
        if self.pos + n > self.end {
            return Err(eof(self.pos));
        }
        let mut v = 0u64;
        for _ in 0..n {
            let b = self.data[self.pos / 8] >> (7 - self.pos % 8) & 1;
            v = v << 1 | u64::from(b);
            self.pos += 1;
        }
        Ok(v)
    }

    /// BB: a two-bit code.
    pub fn bb(&mut self) -> Result<u8, DwgError> {
        Ok(self.bits(2)? as u8)
    }

    /// 3B: one to three bits, ending at a zero bit.
    pub fn b3(&mut self) -> Result<u8, DwgError> {
        let mut v = 0u8;
        for _ in 0..3 {
            let b = self.bit()?;
            v = v << 1 | u8::from(b);
            if !b {
                break;
            }
        }
        Ok(v)
    }

    /// RC: a raw byte (not necessarily byte aligned).
    pub fn rc(&mut self) -> Result<u8, DwgError> {
        if self.pos.is_multiple_of(8) {
            if self.pos + 8 > self.end {
                return Err(eof(self.pos));
            }
            let b = self.data[self.pos / 8];
            self.pos += 8;
            return Ok(b);
        }
        Ok(self.bits(8)? as u8)
    }

    /// `n` raw bytes.
    pub fn bytes(&mut self, n: usize) -> Result<Vec<u8>, DwgError> {
        if n > self.remaining() / 8 {
            return Err(eof(self.pos));
        }
        if self.pos.is_multiple_of(8) {
            let start = self.pos / 8;
            self.pos += n * 8;
            return Ok(self.data[start..start + n].to_vec());
        }
        (0..n).map(|_| self.rc()).collect()
    }

    fn le(&mut self, n: usize) -> Result<u64, DwgError> {
        let mut v = 0u64;
        for i in 0..n {
            v |= u64::from(self.rc()?) << (8 * i);
        }
        Ok(v)
    }

    /// RS: a raw little-endian short.
    pub fn rs(&mut self) -> Result<u16, DwgError> {
        Ok(self.le(2)? as u16)
    }

    /// RL: a raw little-endian long.
    pub fn rl(&mut self) -> Result<u32, DwgError> {
        Ok(self.le(4)? as u32)
    }

    /// RD: a raw double.
    pub fn rd(&mut self) -> Result<f64, DwgError> {
        Ok(f64::from_bits(self.le(8)?))
    }

    /// BS: a bit short.
    pub fn bs(&mut self) -> Result<i16, DwgError> {
        Ok(match self.bb()? {
            0 => self.rs()? as i16,
            1 => i16::from(self.rc()?),
            2 => 0,
            _ => 256,
        })
    }

    /// BL: a bit long.
    pub fn bl(&mut self) -> Result<i32, DwgError> {
        match self.bb()? {
            0 => Ok(self.rl()? as i32),
            1 => Ok(i32::from(self.rc()?)),
            2 => Ok(0),
            _ => Err(DwgError::Corrupt(format!(
                "bit long with code 3 at bit {}",
                self.pos - 2
            ))),
        }
    }

    /// BLL: a bit long long: the byte count in three bits (0 to 7; the
    /// specification refers to 3B here, which cannot give every count),
    /// then the bytes.
    pub fn bll(&mut self) -> Result<u64, DwgError> {
        let n = self.bits(3)? as usize;
        self.le(n)
    }

    /// BD: a bit double.
    pub fn bd(&mut self) -> Result<f64, DwgError> {
        match self.bb()? {
            0 => self.rd(),
            1 => Ok(1.0),
            2 => Ok(0.0),
            _ => Err(DwgError::Corrupt(format!(
                "bit double with code 3 at bit {}",
                self.pos - 2
            ))),
        }
    }

    /// 2RD.
    pub fn rd2(&mut self) -> Result<[f64; 2], DwgError> {
        Ok([self.rd()?, self.rd()?])
    }

    /// 3RD.
    pub fn rd3(&mut self) -> Result<[f64; 3], DwgError> {
        Ok([self.rd()?, self.rd()?, self.rd()?])
    }

    /// 2BD.
    pub fn bd2(&mut self) -> Result<[f64; 2], DwgError> {
        Ok([self.bd()?, self.bd()?])
    }

    /// 3BD.
    pub fn bd3(&mut self) -> Result<[f64; 3], DwgError> {
        Ok([self.bd()?, self.bd()?, self.bd()?])
    }

    /// DD: a bit double with a default value.
    pub fn dd(&mut self, default: f64) -> Result<f64, DwgError> {
        let mut b = default.to_le_bytes();
        match self.bb()? {
            0 => {}
            1 => {
                for v in b.iter_mut().take(4) {
                    *v = self.rc()?;
                }
            }
            2 => {
                b[4] = self.rc()?;
                b[5] = self.rc()?;
                for v in b.iter_mut().take(4) {
                    *v = self.rc()?;
                }
            }
            _ => return self.rd(),
        }
        Ok(f64::from_le_bytes(b))
    }

    /// 3DD: three DDs with their defaults.
    pub fn dd3(&mut self, default: [f64; 3]) -> Result<[f64; 3], DwgError> {
        Ok([
            self.dd(default[0])?,
            self.dd(default[1])?,
            self.dd(default[2])?,
        ])
    }

    /// BT: a thickness (a bit for 0.0 from R2000 on).
    pub fn bt(&mut self) -> Result<f64, DwgError> {
        if self.version >= Version::R2000 && self.bit()? {
            return Ok(0.0);
        }
        self.bd()
    }

    /// BE: an extrusion direction (a bit for (0, 0, 1) from R2000 on).
    pub fn be(&mut self) -> Result<[f64; 3], DwgError> {
        if self.version >= Version::R2000 && self.bit()? {
            return Ok([0.0, 0.0, 1.0]);
        }
        self.bd3()
    }

    /// OT: an object type (a bit short before R2010).
    pub fn ot(&mut self) -> Result<u16, DwgError> {
        if self.version < Version::R2010 {
            return Ok(self.bs()? as u16);
        }
        Ok(match self.bb()? {
            0 => u16::from(self.rc()?),
            1 => u16::from(self.rc()?) + 0x1f0,
            _ => self.rs()?,
        })
    }

    /// H: a handle reference; offsets (codes 6, 8, 0xA, 0xC) apply to
    /// `base`, the referring object's handle.
    pub fn handle(&mut self, base: u64) -> Result<HandleRef, DwgError> {
        let b = self.rc()?;
        let code = b >> 4;
        let count = usize::from(b & 0x0f);
        let mut value = 0u64;
        for _ in 0..count {
            value = value << 8 | u64::from(self.rc()?);
        }
        let handle = match code {
            6 => base.wrapping_add(1),
            8 => base.wrapping_sub(1),
            0xA => base.wrapping_add(value),
            0xC => base.wrapping_sub(value),
            _ => value,
        };
        Ok(HandleRef { code, handle })
    }

    /// T: a text of the codepage (a bit short length, then the bytes).
    pub fn text8(&mut self) -> Result<String, DwgError> {
        let n = self.bs()? as u16 as usize;
        let b = self.bytes(n)?;
        Ok(latin1(&b))
    }

    /// TU: a Unicode text (a bit short length in characters, then UTF-16).
    pub fn text16(&mut self) -> Result<String, DwgError> {
        let n = self.bs()? as u16 as usize;
        let mut units = Vec::with_capacity(n);
        for _ in 0..n {
            units.push(self.rs()?);
        }
        // Texts end with a zero character in some objects.
        while units.last() == Some(&0) {
            units.pop();
        }
        Ok(String::from_utf16_lossy(&units))
    }

    /// TV: T before R2007, TU from then on.
    pub fn text(&mut self) -> Result<String, DwgError> {
        if self.version >= Version::R2007 {
            self.text16()
        } else {
            self.text8()
        }
    }

    /// The object's string stream from R2007 on: the flag bit just before
    /// `endbit` (the end of the object's data) tells whether there is one;
    /// its size is stored before the flag. A reader of the strings, or None.
    pub fn string_stream(&self, endbit: usize) -> Result<Option<BitReader<'a>>, DwgError> {
        if endbit == 0 || endbit > self.data.len() * 8 {
            return Err(DwgError::Corrupt(format!(
                "the object's data ends at bit {endbit}, past its bytes"
            )));
        }
        let mut r = BitReader::at(self.data, self.version, endbit - 1, endbit);
        if !r.bit()? {
            return Ok(None);
        }
        let mut pos = (endbit - 1).checked_sub(16).ok_or_else(|| eof(endbit))?;
        let mut s = BitReader::at(self.data, self.version, pos, pos + 16);
        let mut size = usize::from(s.rs()?);
        if size & 0x8000 != 0 {
            pos = pos.checked_sub(16).ok_or_else(|| eof(endbit))?;
            let mut h = BitReader::at(self.data, self.version, pos, pos + 16);
            let hi = usize::from(h.rs()?);
            size = (size & 0x7fff) | hi << 15;
        }
        let start = pos.checked_sub(size).ok_or_else(|| {
            DwgError::Corrupt(format!("a string stream of {size} bits before bit {pos}"))
        })?;
        Ok(Some(BitReader::at(self.data, self.version, start, pos)))
    }
}

/// Bytes of the codepage as text: Latin-1 (the drawings' names are ASCII
/// in practice).
pub fn latin1(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    b[..end].iter().map(|&c| char::from(c)).collect()
}

/// MC: a modular char at `*pos` of a byte slice; `signed` reads the sign
/// bit (0x40) of the last byte.
pub fn modular_char(data: &[u8], pos: &mut usize, signed: bool) -> Result<i64, DwgError> {
    let mut v: i64 = 0;
    let mut shift = 0;
    loop {
        let b = *data
            .get(*pos)
            .ok_or_else(|| DwgError::Corrupt("a modular char ends early".into()))?;
        *pos += 1;
        if b & 0x80 != 0 {
            v |= i64::from(b & 0x7f) << shift;
            shift += 7;
            if shift > 56 {
                return Err(DwgError::Corrupt("a modular char is too long".into()));
            }
            continue;
        }
        if signed {
            v |= i64::from(b & 0x3f) << shift;
            if b & 0x40 != 0 {
                v = -v;
            }
        } else {
            v |= i64::from(b & 0x7f) << shift;
        }
        return Ok(v);
    }
}

/// MS: a modular short at `*pos` (little-endian shorts, high bit set while
/// another follows).
pub fn modular_short(data: &[u8], pos: &mut usize) -> Result<u64, DwgError> {
    let mut v: u64 = 0;
    let mut shift = 0;
    loop {
        let (Some(&lo), Some(&hi)) = (data.get(*pos), data.get(*pos + 1)) else {
            return Err(DwgError::Corrupt("a modular short ends early".into()));
        };
        *pos += 2;
        let w = u64::from(u16::from_le_bytes([lo, hi]));
        v |= (w & 0x7fff) << shift;
        if w & 0x8000 == 0 {
            return Ok(v);
        }
        shift += 15;
        if shift > 45 {
            return Err(DwgError::Corrupt("a modular short is too long".into()));
        }
    }
}

/// Writes bit-coded values (the test files).
#[derive(Clone, Debug)]
pub struct BitWriter {
    pub data: Vec<u8>,
    /// Bits written.
    pub len: usize,
    pub version: Version,
}

impl BitWriter {
    pub fn new(version: Version) -> Self {
        BitWriter {
            data: Vec::new(),
            len: 0,
            version,
        }
    }

    pub fn bit(&mut self, b: bool) -> &mut Self {
        if self.len.is_multiple_of(8) {
            self.data.push(0);
        }
        if b {
            let i = self.len / 8;
            self.data[i] |= 0x80 >> (self.len % 8);
        }
        self.len += 1;
        self
    }

    pub fn bits(&mut self, v: u64, n: usize) -> &mut Self {
        for i in (0..n).rev() {
            self.bit(v >> i & 1 == 1);
        }
        self
    }

    pub fn bb(&mut self, v: u8) -> &mut Self {
        self.bits(u64::from(v), 2)
    }

    pub fn rc(&mut self, v: u8) -> &mut Self {
        self.bits(u64::from(v), 8)
    }

    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        for &v in b {
            self.rc(v);
        }
        self
    }

    pub fn rs(&mut self, v: u16) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    pub fn rl(&mut self, v: u32) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    pub fn rd(&mut self, v: f64) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    pub fn rd3(&mut self, p: [f64; 3]) -> &mut Self {
        for v in p {
            self.rd(v);
        }
        self
    }

    pub fn bs(&mut self, v: i16) -> &mut Self {
        match v {
            0 => self.bb(2),
            256 => self.bb(3),
            1..=255 => self.bb(1).rc(v as u8),
            _ => self.bb(0).rs(v as u16),
        }
    }

    pub fn bl(&mut self, v: i32) -> &mut Self {
        match v {
            0 => self.bb(2),
            1..=255 => self.bb(1).rc(v as u8),
            _ => self.bb(0).rl(v as u32),
        }
    }

    pub fn bll(&mut self, v: u64) -> &mut Self {
        let n = (8 - v.leading_zeros() as usize / 8).min(7);
        self.bits(n as u64, 3);
        for i in 0..n {
            self.rc((v >> (8 * i)) as u8);
        }
        self
    }

    pub fn bd(&mut self, v: f64) -> &mut Self {
        if v == 1.0 {
            self.bb(1)
        } else if v == 0.0 && v.is_sign_positive() {
            self.bb(2)
        } else {
            self.bb(0).rd(v)
        }
    }

    pub fn bd3(&mut self, p: [f64; 3]) -> &mut Self {
        for v in p {
            self.bd(v);
        }
        self
    }

    pub fn dd(&mut self, v: f64, default: f64) -> &mut Self {
        if v.to_bits() == default.to_bits() {
            self.bb(0)
        } else {
            self.bb(3).rd(v)
        }
    }

    pub fn bt(&mut self, v: f64) -> &mut Self {
        if self.version >= Version::R2000 {
            if v == 0.0 {
                return self.bit(true);
            }
            self.bit(false);
        }
        self.bd(v)
    }

    pub fn be(&mut self, p: [f64; 3]) -> &mut Self {
        if self.version >= Version::R2000 {
            if p == [0.0, 0.0, 1.0] {
                return self.bit(true);
            }
            self.bit(false);
        }
        self.bd3(p)
    }

    pub fn ot(&mut self, v: u16) -> &mut Self {
        if self.version < Version::R2010 {
            return self.bs(v as i16);
        }
        if v < 256 {
            self.bb(0).rc(v as u8)
        } else if (0x1f0..0x1f0 + 256).contains(&v) {
            self.bb(1).rc((v - 0x1f0) as u8)
        } else {
            self.bb(2).rs(v)
        }
    }

    /// H with an absolute handle.
    pub fn handle(&mut self, code: u8, handle: u64) -> &mut Self {
        let n = (8 - handle.leading_zeros() as usize / 8).min(8);
        self.rc(code << 4 | n as u8);
        for i in (0..n).rev() {
            self.rc((handle >> (8 * i)) as u8);
        }
        self
    }

    pub fn text8(&mut self, s: &str) -> &mut Self {
        let b: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
        self.bs(b.len() as i16).bytes(&b)
    }

    pub fn text16(&mut self, s: &str) -> &mut Self {
        let units: Vec<u16> = s.encode_utf16().collect();
        self.bs(units.len() as i16);
        for u in units {
            self.rs(u);
        }
        self
    }

    pub fn text(&mut self, s: &str) -> &mut Self {
        if self.version >= Version::R2007 {
            self.text16(s)
        } else {
            self.text8(s)
        }
    }

    /// Appends another writer's bits.
    pub fn append(&mut self, other: &BitWriter) -> &mut Self {
        for i in 0..other.len {
            self.bit(other.data[i / 8] >> (7 - i % 8) & 1 == 1);
        }
        self
    }

    pub fn align(&mut self) -> &mut Self {
        while !self.len.is_multiple_of(8) {
            self.bit(false);
        }
        self
    }
}

/// MC of an unsigned value.
pub fn write_modular_char(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

/// MC of a signed value (the sign in bit 0x40 of the last byte).
pub fn write_signed_modular_char(out: &mut Vec<u8>, v: i64) {
    let negative = v < 0;
    let mut m = v.unsigned_abs();
    loop {
        if m < 0x40 {
            out.push(m as u8 | if negative { 0x40 } else { 0 });
            return;
        }
        out.push((m & 0x7f) as u8 | 0x80);
        m >>= 7;
    }
}

/// MS of a value.
pub fn write_modular_short(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let w = (v & 0x7fff) as u16;
        v >>= 15;
        if v == 0 {
            out.extend_from_slice(&w.to_le_bytes());
            return;
        }
        out.extend_from_slice(&(w | 0x8000).to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_shorts_of_the_specification() {
        // 00 00000001 00000001 | 10 | 11 | 01 00001111 | 10 (chapter 2.2).
        let bits = "0000000001000000011011010000111110";
        let mut w = BitWriter::new(Version::R2000);
        for c in bits.chars() {
            w.bit(c == '1');
        }
        let mut r = BitReader::new(&w.data, Version::R2000);
        let v: Vec<i16> = (0..5).map(|_| r.bs().unwrap()).collect();
        assert_eq!(v, [257, 0, 256, 15, 0]);
    }

    #[test]
    fn modular_chars_of_the_specification() {
        let mut p = 0;
        assert_eq!(modular_char(&[0x82, 0x24], &mut p, true).unwrap(), 4610);
        let mut p = 0;
        assert_eq!(
            modular_char(&[0xe9, 0x97, 0xe6, 0x35], &mut p, true).unwrap(),
            112_823_273
        );
        let mut p = 0;
        assert_eq!(modular_char(&[0x85, 0x4b], &mut p, true).unwrap(), -1413);
        let mut p = 0;
        assert_eq!(
            modular_short(&[0x31, 0xf4, 0x8d, 0x00], &mut p).unwrap(),
            4_650_033
        );
        for v in [0i64, 1, 63, 64, 1413, -1413, 112_823_273, -5] {
            let mut out = Vec::new();
            write_signed_modular_char(&mut out, v);
            let mut p = 0;
            assert_eq!(modular_char(&out, &mut p, true).unwrap(), v);
        }
        for v in [0u64, 127, 128, 4_650_033] {
            let mut out = Vec::new();
            write_modular_short(&mut out, v);
            let mut p = 0;
            assert_eq!(modular_short(&out, &mut p).unwrap(), v);
            let mut out = Vec::new();
            write_modular_char(&mut out, v);
            let mut p = 0;
            assert_eq!(modular_char(&out, &mut p, false).unwrap() as u64, v);
        }
    }

    #[test]
    fn values_round_trip() {
        for version in [Version::R14, Version::R2000, Version::R2007, Version::R2010] {
            let mut w = BitWriter::new(version);
            w.bit(true)
                .bs(-1)
                .bs(300)
                .bl(-7)
                .bl(70000)
                .bll(0x0012_3456_789a)
                .bll(0)
                .bd(2.5)
                .bd(1.0)
                .dd(3.0, 3.0)
                .dd(4.0, 3.0)
                .bt(0.0)
                .be([0.0, 0.0, 1.0])
                .ot(0x1f3)
                .ot(38)
                .handle(5, 0x5e7)
                .text("Layer 1");
            let mut r = BitReader::new(&w.data, version);
            assert!(r.bit().unwrap());
            assert_eq!(r.bs().unwrap(), -1);
            assert_eq!(r.bs().unwrap(), 300);
            assert_eq!(r.bl().unwrap(), -7);
            assert_eq!(r.bl().unwrap(), 70000);
            assert_eq!(r.bll().unwrap(), 0x0012_3456_789a);
            assert_eq!(r.bll().unwrap(), 0);
            assert_eq!(r.bd().unwrap(), 2.5);
            assert_eq!(r.bd().unwrap(), 1.0);
            assert_eq!(r.dd(3.0).unwrap(), 3.0);
            assert_eq!(r.dd(3.0).unwrap(), 4.0);
            assert_eq!(r.bt().unwrap(), 0.0);
            assert_eq!(r.be().unwrap(), [0.0, 0.0, 1.0]);
            assert_eq!(r.ot().unwrap(), 0x1f3);
            assert_eq!(r.ot().unwrap(), 38);
            assert_eq!(
                r.handle(0).unwrap(),
                HandleRef {
                    code: 5,
                    handle: 0x5e7
                }
            );
            assert_eq!(r.text().unwrap(), "Layer 1");
        }
    }

    #[test]
    fn default_doubles_patch_bytes() {
        // 01: four bytes replace the low four of the default.
        let mut w = BitWriter::new(Version::R2000);
        let target = 1.000_000_1f64;
        let t = target.to_le_bytes();
        w.bb(1).bytes(&t[..4]);
        let mut r = BitReader::new(&w.data, Version::R2000);
        let d = f64::from_le_bytes([0, 0, 0, 0, t[4], t[5], t[6], t[7]]);
        assert_eq!(r.dd(d).unwrap(), target);
        // 10: bytes 5 and 6, then the low four.
        let mut w = BitWriter::new(Version::R2000);
        w.bb(2).bytes(&t[4..6]).bytes(&t[..4]);
        let mut r = BitReader::new(&w.data, Version::R2000);
        let d = f64::from_le_bytes([0, 0, 0, 0, 0, 0, t[6], t[7]]);
        assert_eq!(r.dd(d).unwrap(), target);
    }

    #[test]
    fn relative_handles() {
        let mut w = BitWriter::new(Version::R2000);
        w.rc(0x60).rc(0x80).rc(0xa1).rc(3).rc(0xc1).rc(2);
        let mut r = BitReader::new(&w.data, Version::R2000);
        assert_eq!(r.handle(0x10).unwrap().handle, 0x11);
        assert_eq!(r.handle(0x10).unwrap().handle, 0x0f);
        assert_eq!(r.handle(0x10).unwrap().handle, 0x13);
        assert_eq!(r.handle(0x10).unwrap().handle, 0x0e);
    }

    #[test]
    fn string_streams() {
        // Data, then strings, then their size and the flag bit.
        let mut w = BitWriter::new(Version::R2007);
        w.bl(5);
        let start = w.len;
        w.text16("abc");
        let size = w.len - start;
        w.rs(size as u16).bit(true);
        let endbit = w.len;
        let r = BitReader::new(&w.data, Version::R2007);
        let mut s = r.string_stream(endbit).unwrap().unwrap();
        assert_eq!(s.position(), start);
        assert_eq!(s.text().unwrap(), "abc");
    }
}
