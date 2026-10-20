// SPDX-License-Identifier: MIT
//! The header variables (the specification's chapter 9), read up to the
//! drawing units (`$INSUNITS`), with the model space's extents
//! (`$EXTMIN`, `$EXTMAX`) on the way. From R2007 on the texts are in a
//! string stream and the handles in a handle stream after the data; the
//! reader skips both kinds there, so it needs neither stream.

use crate::bits::BitReader;
use crate::container::HEADER_SENTINEL;
use crate::{DwgError, Version};

/// The header variables the import uses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeaderVars {
    /// `$INSUNITS` (0 unitless, 4 millimetres, ...).
    pub insunits: Option<i16>,
    /// The model space's `$EXTMIN` and `$EXTMAX` (drawing units).
    pub extmin: Option<[f64; 3]>,
    pub extmax: Option<[f64; 3]>,
    /// The next free handle (`$HANDSEED`).
    pub handseed: Option<u64>,
}

/// The first value of the header variables, the same in every file.
pub const FIRST_VALUE: f64 = 412_148_564_080.0;

struct Walk<'a, 'b> {
    r: &'b mut BitReader<'a>,
    v: Version,
}

impl Walk<'_, '_> {
    fn b(&mut self, n: usize) -> Result<(), DwgError> {
        for _ in 0..n {
            self.r.bit()?;
        }
        Ok(())
    }
    fn bs(&mut self, n: usize) -> Result<(), DwgError> {
        for _ in 0..n {
            self.r.bs()?;
        }
        Ok(())
    }
    fn bl(&mut self, n: usize) -> Result<(), DwgError> {
        for _ in 0..n {
            self.r.bl()?;
        }
        Ok(())
    }
    fn bd(&mut self, n: usize) -> Result<(), DwgError> {
        for _ in 0..n {
            self.r.bd()?;
        }
        Ok(())
    }
    fn rc(&mut self, n: usize) -> Result<(), DwgError> {
        for _ in 0..n {
            self.r.rc()?;
        }
        Ok(())
    }
    fn bd3(&mut self, n: usize) -> Result<(), DwgError> {
        self.bd(3 * n)
    }
    fn rd2(&mut self, n: usize) -> Result<(), DwgError> {
        for _ in 0..2 * n {
            self.r.rd()?;
        }
        Ok(())
    }
    /// TV: in the string stream from R2007 on.
    fn tv(&mut self, n: usize) -> Result<(), DwgError> {
        if self.v < Version::R2007 {
            for _ in 0..n {
                self.r.text8()?;
            }
        }
        Ok(())
    }
    /// T before R2007 (R13-R14 only here).
    fn t(&mut self, n: usize) -> Result<(), DwgError> {
        self.tv(n)
    }
    /// H: in the handle stream from R2007 on.
    fn h(&mut self, n: usize) -> Result<(), DwgError> {
        if self.v < Version::R2007 {
            for _ in 0..n {
                self.r.handle(0)?;
            }
        }
        Ok(())
    }
    /// CMC: a colour index before R2004; index, RGB, flags and names
    /// after.
    fn cmc(&mut self, n: usize) -> Result<(), DwgError> {
        for _ in 0..n {
            self.r.bs()?;
            if self.v >= Version::R2004 {
                self.r.bl()?;
                let flags = self.r.rc()?;
                if flags & 1 != 0 {
                    self.tv(1)?;
                }
                if flags & 2 != 0 {
                    self.tv(1)?;
                }
            }
        }
        Ok(())
    }
}

/// How the data starts: after an R2007 bit size, after the R2013
/// `$REQUIREDVERSIONS`.
fn layouts(v: Version) -> Vec<(bool, bool)> {
    let mut out = Vec::new();
    for size in [v >= Version::R2007, v < Version::R2007] {
        for required in [v >= Version::R2013, v < Version::R2013] {
            out.push((size, required));
        }
    }
    out
}

/// Reads the variables from the header section (from its sentinel on).
pub fn read(section: &[u8], version: Version, maintenance: u8) -> Result<HeaderVars, DwgError> {
    if section.get(..16) != Some(&HEADER_SENTINEL[..]) {
        return Err(DwgError::Corrupt(
            "the header variables have no sentinel".into(),
        ));
    }
    let mut start = 20;
    if (matches!(version, Version::R2010 | Version::R2013) && maintenance > 3)
        || version >= Version::R2018
    {
        start += 4;
    }
    let mut last = None;
    for (size, required) in layouts(version) {
        let mut r = BitReader::at(section, version, start * 8, section.len() * 8);
        if size {
            r.rl()?;
        }
        if required {
            r.bll()?;
        }
        let mut probe = r.clone();
        let first = probe.bd().unwrap_or(0.0);
        let ones = (0..3).all(|_| probe.bd().is_ok_and(|v| v == 1.0));
        if (first - FIRST_VALUE).abs() > 1.0 || !ones {
            continue;
        }
        match walk(&mut r, version) {
            Ok(vars) => return Ok(vars),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| {
        DwgError::Corrupt("the header variables do not start as the format describes".into())
    }))
}

fn walk(r: &mut BitReader, v: Version) -> Result<HeaderVars, DwgError> {
    use Version::*;
    let mut w = Walk { r, v };
    let r13 = v <= R14;
    let mut out = HeaderVars::default();
    w.bd(4)?;
    w.tv(4)?;
    w.bl(2)?;
    if r13 {
        w.bs(1)?;
    }
    if v < R2004 {
        w.h(1)?;
    }
    w.b(2)?;
    if r13 {
        w.b(1)?;
    }
    w.b(7)?;
    if r13 {
        w.b(1)?;
    }
    if v >= R2004 {
        w.b(1)?;
    }
    w.b(4)?;
    if r13 {
        w.b(2)?;
    }
    w.b(2)?;
    if r13 {
        w.b(1)?;
    }
    w.b(3)?;
    if r13 {
        w.b(1)?;
    }
    w.b(2)?;
    w.bs(1)?;
    if r13 {
        w.bs(1)?;
    }
    w.bs(5)?;
    if r13 {
        w.bs(1)?;
    }
    w.bs(1)?;
    if r13 {
        w.bs(1)?;
    }
    w.bs(1)?;
    if r13 {
        w.bs(1)?;
    }
    if v >= R2004 {
        w.bl(3)?;
    }
    w.bs(19)?;
    w.bd(21)?;
    if v <= R2004 {
        w.tv(1)?;
    }
    w.bl(4)?;
    if v >= R2004 {
        w.bl(3)?;
    }
    w.bl(4)?;
    w.cmc(1)?;
    // $HANDSEED: in the data stream in every version.
    out.handseed = Some(w.r.handle(0)?.handle);
    w.h(3)?;
    if v >= R2007 {
        w.h(1)?;
    }
    w.h(2)?;
    if v >= R2000 {
        w.bd(1)?;
    }
    // Paper space.
    w.bd3(3)?;
    w.rd2(2)?;
    w.bd(1)?;
    w.bd3(3)?;
    w.h(1)?;
    if v >= R2000 {
        w.h(1)?;
        w.bs(1)?;
        w.h(1)?;
        w.bd3(6)?;
    }
    // Model space.
    w.bd3(1)?;
    out.extmin = Some(w.r.bd3()?);
    out.extmax = Some(w.r.bd3()?);
    w.rd2(2)?;
    w.bd(1)?;
    w.bd3(3)?;
    w.h(1)?;
    if v >= R2000 {
        w.h(1)?;
        w.bs(1)?;
        w.h(1)?;
        w.bd3(6)?;
        w.tv(2)?;
    }
    if r13 {
        w.b(11)?;
        w.rc(2)?;
        w.b(2)?;
        w.rc(3)?;
        w.b(1)?;
        w.rc(4)?;
        w.bs(6)?;
        w.h(1)?;
    }
    w.bd(9)?;
    if v >= R2007 {
        w.bd(2)?;
        w.bs(1)?;
        w.cmc(1)?;
    }
    if v >= R2000 {
        w.b(6)?;
        w.bs(3)?;
    }
    if v >= R2007 {
        w.bs(1)?;
    }
    w.bd(8)?;
    if r13 {
        w.t(5)?;
    }
    if v >= R2000 {
        w.bd(1)?;
        w.b(1)?;
        w.bs(1)?;
        w.b(4)?;
    }
    w.cmc(3)?;
    if v >= R2000 {
        w.bs(11)?;
        w.b(2)?;
        w.bs(4)?;
        w.b(1)?;
        w.bs(1)?;
    }
    if v >= R2007 {
        w.b(1)?;
    }
    if v >= R2010 {
        w.b(1)?;
        w.bd(1)?;
        w.tv(1)?;
        w.bd(1)?;
        w.tv(1)?;
    }
    if v >= R2000 {
        w.h(5)?;
    }
    if v >= R2007 {
        w.h(3)?;
    }
    if v >= R2000 {
        w.bs(2)?;
    }
    w.h(9)?;
    if v <= R2000 {
        w.h(1)?;
    }
    w.h(3)?;
    if v >= R2000 {
        w.bs(2)?;
        w.tv(2)?;
        w.h(3)?;
    }
    if v >= R2004 {
        w.h(2)?;
    }
    if v >= R2007 {
        w.h(1)?;
    }
    if v >= R2013 {
        w.h(1)?;
    }
    if v >= R2000 {
        w.bl(1)?;
        let units = w.r.bs()?;
        if !(0..=24).contains(&units) {
            return Err(DwgError::Corrupt(format!("$INSUNITS {units}")));
        }
        out.insunits = Some(units);
    }
    Ok(out)
}

/// Writes header variables as `read` reads them (the test files): every
/// value its default but the units and the extents.
pub fn write(version: Version, vars: &HeaderVars) -> Vec<u8> {
    use crate::bits::BitWriter;
    use Version::*;
    let v = version;
    let r13 = v <= R14;
    let mut w = BitWriter::new(v);
    let tv = |w: &mut BitWriter, n: usize| {
        if v < R2007 {
            for _ in 0..n {
                w.text8("");
            }
        }
    };
    let h = |w: &mut BitWriter, n: usize| {
        if v < R2007 {
            for _ in 0..n {
                w.handle(5, 0);
            }
        }
    };
    let bits = |w: &mut BitWriter, n: usize| {
        for _ in 0..n {
            w.bit(false);
        }
    };
    let bs = |w: &mut BitWriter, n: usize| {
        for _ in 0..n {
            w.bs(0);
        }
    };
    let bl = |w: &mut BitWriter, n: usize| {
        for _ in 0..n {
            w.bl(0);
        }
    };
    let bd = |w: &mut BitWriter, n: usize| {
        for _ in 0..n {
            w.bd(0.0);
        }
    };
    let cmc = |w: &mut BitWriter, n: usize| {
        for _ in 0..n {
            w.bs(256);
            if v >= R2004 {
                w.bl(0).rc(0);
            }
        }
    };
    if v >= R2007 {
        w.rl(0);
    }
    if v >= R2013 {
        w.bll(0);
    }
    w.bd(FIRST_VALUE).bd(1.0).bd(1.0).bd(1.0);
    tv(&mut w, 4);
    w.bl(24).bl(0);
    if r13 {
        bs(&mut w, 1);
    }
    if v < R2004 {
        h(&mut w, 1);
    }
    bits(&mut w, 2);
    if r13 {
        bits(&mut w, 1);
    }
    bits(&mut w, 7);
    if r13 {
        bits(&mut w, 1);
    }
    if v >= R2004 {
        bits(&mut w, 1);
    }
    bits(&mut w, 4);
    if r13 {
        bits(&mut w, 2);
    }
    bits(&mut w, 2);
    if r13 {
        bits(&mut w, 1);
    }
    bits(&mut w, 3);
    if r13 {
        bits(&mut w, 1);
    }
    bits(&mut w, 2);
    bs(&mut w, 1);
    if r13 {
        bs(&mut w, 1);
    }
    bs(&mut w, 5);
    if r13 {
        bs(&mut w, 1);
    }
    bs(&mut w, 1);
    if r13 {
        bs(&mut w, 1);
    }
    bs(&mut w, 1);
    if r13 {
        bs(&mut w, 1);
    }
    if v >= R2004 {
        bl(&mut w, 3);
    }
    bs(&mut w, 19);
    bd(&mut w, 21);
    if v <= R2004 {
        tv(&mut w, 1);
    }
    bl(&mut w, 4);
    if v >= R2004 {
        bl(&mut w, 3);
    }
    bl(&mut w, 4);
    cmc(&mut w, 1);
    w.handle(0, vars.handseed.unwrap_or(0x100));
    h(&mut w, 3);
    if v >= R2007 {
        h(&mut w, 1);
    }
    h(&mut w, 2);
    if v >= R2000 {
        bd(&mut w, 1);
    }
    bd(&mut w, 9);
    for _ in 0..4 {
        w.rd(0.0);
    }
    bd(&mut w, 10);
    h(&mut w, 1);
    if v >= R2000 {
        h(&mut w, 1);
        bs(&mut w, 1);
        h(&mut w, 1);
        bd(&mut w, 18);
    }
    bd(&mut w, 3);
    w.bd3(vars.extmin.unwrap_or([0.0; 3]));
    w.bd3(vars.extmax.unwrap_or([0.0; 3]));
    for _ in 0..4 {
        w.rd(0.0);
    }
    bd(&mut w, 10);
    h(&mut w, 1);
    if v >= R2000 {
        h(&mut w, 1);
        bs(&mut w, 1);
        h(&mut w, 1);
        bd(&mut w, 18);
        tv(&mut w, 2);
    }
    if r13 {
        bits(&mut w, 11);
        for _ in 0..2 {
            w.rc(0);
        }
        bits(&mut w, 2);
        for _ in 0..3 {
            w.rc(0);
        }
        bits(&mut w, 1);
        for _ in 0..4 {
            w.rc(0);
        }
        bs(&mut w, 6);
        h(&mut w, 1);
    }
    bd(&mut w, 9);
    if v >= R2007 {
        bd(&mut w, 2);
        bs(&mut w, 1);
        cmc(&mut w, 1);
    }
    if v >= R2000 {
        bits(&mut w, 6);
        bs(&mut w, 3);
    }
    if v >= R2007 {
        bs(&mut w, 1);
    }
    bd(&mut w, 8);
    if r13 {
        tv(&mut w, 5);
    }
    if v >= R2000 {
        bd(&mut w, 1);
        bits(&mut w, 1);
        bs(&mut w, 1);
        bits(&mut w, 4);
    }
    cmc(&mut w, 3);
    if v >= R2000 {
        bs(&mut w, 11);
        bits(&mut w, 2);
        bs(&mut w, 4);
        bits(&mut w, 1);
        bs(&mut w, 1);
    }
    if v >= R2007 {
        bits(&mut w, 1);
    }
    if v >= R2010 {
        bits(&mut w, 1);
        bd(&mut w, 1);
        tv(&mut w, 1);
        bd(&mut w, 1);
        tv(&mut w, 1);
    }
    if v >= R2000 {
        h(&mut w, 5);
    }
    if v >= R2007 {
        h(&mut w, 3);
    }
    if v >= R2000 {
        bs(&mut w, 2);
    }
    h(&mut w, 9);
    if v <= R2000 {
        h(&mut w, 1);
    }
    h(&mut w, 3);
    if v >= R2000 {
        bs(&mut w, 2);
        tv(&mut w, 2);
        h(&mut w, 3);
    }
    if v >= R2004 {
        h(&mut w, 2);
    }
    if v >= R2007 {
        h(&mut w, 1);
    }
    if v >= R2013 {
        h(&mut w, 1);
    }
    if v >= R2000 {
        bl(&mut w, 1);
        w.bs(vars.insunits.unwrap_or(0));
    }
    // The rest of the variables are not written: a reader of the full
    // list would stop here.
    w.align();
    let mut out = HEADER_SENTINEL.to_vec();
    out.extend_from_slice(&(w.data.len() as u32).to_le_bytes());
    if (matches!(v, R2010 | R2013) && crate::testfile::MAINTENANCE > 3) || v >= R2018 {
        out.extend_from_slice(&0u32.to_le_bytes());
    }
    out.extend_from_slice(&w.data);
    let crc = crate::crc::crc16(0xC0C1, &out[16..]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&crate::container::HEADER_END_SENTINEL);
    out
}
