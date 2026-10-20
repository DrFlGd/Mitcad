// SPDX-License-Identifier: MIT
//! The object map (the specification's chapter 23): every object's handle
//! with where its data starts, in runs of at most 2032 bytes, each a
//! big-endian size, pairs of modular chars (the handle's and the
//! location's offset from the previous pair) and a CRC.

use std::collections::BTreeMap;

use crate::DwgError;
use crate::bits::{modular_char, write_modular_char, write_signed_modular_char};
use crate::crc::crc16;

/// Reads the map: handle to the object's offset in the objects' bytes.
pub fn read(data: &[u8]) -> Result<BTreeMap<u64, u64>, DwgError> {
    let mut map = BTreeMap::new();
    let mut at = 0;
    // A missing empty last run: what was read stands.
    while let (Some(&hi), Some(&lo)) = (data.get(at), data.get(at + 1)) {
        let size = usize::from(u16::from_be_bytes([hi, lo]));
        if size <= 2 {
            break;
        }
        let end = at + size;
        if end > data.len() {
            return Err(DwgError::Corrupt(format!(
                "the object map's run at {at} ends past its section"
            )));
        }
        let mut p = at + 2;
        // Each run starts again from handle 0 and location 0.
        let mut handle: i64 = 0;
        let mut location: i64 = 0;
        while p < end {
            handle += modular_char(data, &mut p, false)?;
            location += modular_char(data, &mut p, true)?;
            if handle < 0 || location < 0 {
                return Err(DwgError::Corrupt(
                    "a negative handle or location in the object map".into(),
                ));
            }
            map.insert(handle as u64, location as u64);
        }
        at = end + 2;
    }
    Ok(map)
}

/// Writes a map (the test files): the handles ascending.
pub fn write(map: &BTreeMap<u64, u64>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut run: Vec<u8> = Vec::new();
    let mut last = (0u64, 0i64);
    let flush = |out: &mut Vec<u8>, run: &mut Vec<u8>| {
        let size = (run.len() + 2) as u16;
        let mut section = size.to_be_bytes().to_vec();
        section.append(run);
        let crc = crc16(0xC0C1, &section);
        out.extend_from_slice(&section);
        out.extend_from_slice(&crc.to_be_bytes());
    };
    for (&handle, &location) in map {
        let mut pair = Vec::new();
        write_modular_char(&mut pair, handle - last.0);
        write_signed_modular_char(&mut pair, location as i64 - last.1);
        if run.len() + pair.len() > 2030 {
            flush(&mut out, &mut run);
            // The next run starts again from 0.
            pair.clear();
            write_modular_char(&mut pair, handle);
            write_signed_modular_char(&mut pair, location as i64);
        }
        run.extend_from_slice(&pair);
        last = (handle, location as i64);
    }
    if !run.is_empty() {
        flush(&mut out, &mut run);
    }
    flush(&mut out, &mut run);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut map = BTreeMap::new();
        for i in 1..2000u64 {
            map.insert(i * 3, 1000 + (i * 37) % 5000);
        }
        let data = write(&map);
        assert_eq!(read(&data).unwrap(), map);
    }
}
