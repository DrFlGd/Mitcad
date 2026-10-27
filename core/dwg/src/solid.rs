// SPDX-License-Identifier: MIT
//! The B-rep record of solids, bodies, regions and surfaces (the
//! specification's 20.4.41 and 24.2.2.3): an ASM file of the same family
//! as the `.f3d` and `.ipt` body records, in its binary form (in the
//! entity, or from R2013 on in the data storage) or its text form (up to
//! R2010, in blocks whose printable characters are mirrored: c becomes
//! 0x9F - c). Both are parsed by the `.f3d` reader's ASM code
//! (`mitcad_f3d::asm`), the text form through its text tokeniser.

#[cfg(feature = "solids")]
use mitcad_f3d::asm::AsmFile;

use crate::objects::BrepData;

/// The older naming's binary magic, as `mitcad_f3d::asm::file` has it: here
/// too, so that the data storage reads without the feature `solids`.
const OLDER_MAGIC: &[u8] = &[
    0x41, 0x43, 0x49, 0x53, b' ', b'B', b'i', b'n', b'a', b'r', b'y', b'F', b'i', b'l', b'e',
];

/// The binary form's magic, in the ASM and the older naming.
pub fn is_binary(data: &[u8]) -> bool {
    data.starts_with(b"ASM BinaryFile") || data.starts_with(OLDER_MAGIC)
}

/// The text form as stored in the entity, unmasked.
pub fn unmask(data: &[u8]) -> Vec<u8> {
    data.iter()
        .map(|&b| match b {
            0x21..=0x7e => 0x9f - b,
            0x7f => b' ',
            b'\t' => b' ',
            _ => b,
        })
        .collect()
}

/// Masks text as the entity stores it (the test files).
pub fn mask(text: &[u8]) -> Vec<u8> {
    text.iter()
        .map(|&b| match b {
            0x21..=0x7e => 0x9f - b,
            b' ' => 0x7f,
            _ => b,
        })
        .collect()
}

/// Where an entity's B-rep record came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Text,
    Binary,
    /// The binary form from the data storage.
    Stored,
}

impl Form {
    pub fn name(self) -> &'static str {
        match self {
            Form::Text => "text",
            Form::Binary => "binary",
            Form::Stored => "binary (data storage)",
        }
    }
}

/// The entity's record as bytes to parse, or None when it has none.
pub fn record(data: &BrepData, stored: Option<&Vec<u8>>) -> Option<(Form, Vec<u8>)> {
    if let Some(s) = stored {
        return Some((Form::Stored, s.clone()));
    }
    match data {
        BrepData::Empty | BrepData::Unreadable(_) => None,
        BrepData::Binary(b) => Some((Form::Binary, b.clone())),
        BrepData::Text(t) => Some((Form::Text, unmask(t))),
    }
}

/// Parses a record of either form (the feature `solids`).
#[cfg(feature = "solids")]
pub fn parse(form: Form, bytes: &[u8]) -> Result<AsmFile, String> {
    match form {
        Form::Text => mitcad_f3d::asm::text::parse(bytes).map_err(|e| e.to_string()),
        _ => AsmFile::parse(bytes).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "solids")]
    #[test]
    fn the_older_magic_is_the_asm_readers() {
        assert_eq!(OLDER_MAGIC, mitcad_f3d::asm::file::OLDER_MAGIC);
    }

    #[test]
    fn masking() {
        let text = b"700 0 1 0\n@7 unknown 15 ASM 7.0 #\tx";
        let masked = mask(text);
        assert_ne!(masked, text);
        let mut expected = text.to_vec();
        let tab = expected.len() - 2;
        expected[tab] = b' ';
        assert_eq!(unmask(&masked), expected);
        // The specification's rule: printable c is stored as 0x9F - c.
        assert_eq!(mask(b"A"), [0x9f - b'A']);
    }
}
