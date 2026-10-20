// SPDX-License-Identifier: MIT
//! The checks of a drawing (the specification's 2.14 and 4.2): the 16-bit
//! CRC of objects and sections (the reflected polynomial 0xA001, with a
//! starting value), the standard 32-bit CRC, and the page checksum of the
//! R2004 format. The tables are computed, not stored.

const fn table16() -> [u16; 256] {
    let mut t = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u16;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { c >> 1 ^ 0xA001 } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

const fn table32() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                c >> 1 ^ 0xEDB8_8320
            } else {
                c >> 1
            };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

static TABLE16: [u16; 256] = table16();
static TABLE32: [u32; 256] = table32();

/// The 16-bit CRC of `data`, continuing from `seed`.
pub fn crc16(seed: u16, data: &[u8]) -> u16 {
    let mut dx = seed;
    for &b in data {
        let al = b ^ (dx & 0xff) as u8;
        dx = (dx >> 8) ^ TABLE16[usize::from(al)];
    }
    dx
}

/// The 32-bit CRC of `data` with `seed`.
pub fn crc32(seed: u32, data: &[u8]) -> u32 {
    let mut c = !seed;
    for &b in data {
        c = (c >> 8) ^ TABLE32[((c ^ u32::from(b)) & 0xff) as usize];
    }
    !c
}

/// The section page checksum of the R2004 format.
pub fn page_checksum(seed: u32, data: &[u8]) -> u32 {
    let mut sum1 = seed & 0xffff;
    let mut sum2 = seed >> 16;
    for chunk in data.chunks(0x15b0) {
        for &b in chunk {
            sum1 += u32::from(b);
            sum2 += sum1;
        }
        sum1 %= 0xfff1;
        sum2 %= 0xfff1;
    }
    sum2 << 16 | (sum1 & 0xffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_match_the_specification() {
        // The first and last entries the specification lists.
        assert_eq!(TABLE16[1], 0xC0C1);
        assert_eq!(TABLE16[255], 0x4040);
        assert_eq!(TABLE32[1], 0x7707_3096);
        assert_eq!(TABLE32[255], 0x2D02_EF8D);
        assert_eq!(crc32(0, b"123456789"), 0xCBF4_3926);
    }
}
