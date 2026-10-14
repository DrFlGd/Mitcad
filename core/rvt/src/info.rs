// SPDX-License-Identifier: MIT
//! The metadata streams: `BasicFileInfo` (the saving release and build),
//! `PartAtom` (a family's Atom XML), `TransmissionData` (the linked files)
//! and the preview image.
//!
//! `BasicFileInfo` also holds user names and paths; the reader keeps the
//! fields it names here and leaves the personal ones out of what it shows.

use serde::Serialize;

/// The keys of `BasicFileInfo`'s text block that hold personal data or
/// paths: never shown.
pub const PRIVATE_KEYS: [&str; 4] = ["Username", "Central Model Path", "Last Save Path", "Author"];

/// What `BasicFileInfo` says about the file.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BasicFileInfo {
    /// The layout version of the stream (10 for the 2016 to 2018 releases,
    /// 13 for 2019 and 2020, 14 from 2021).
    pub layout: u32,
    /// The release that saved the file (its year).
    pub release: Option<u32>,
    /// The build of that release.
    pub build: Option<String>,
    /// The text block's `Key: value` lines, the personal ones
    /// ([`PRIVATE_KEYS`]) left out.
    pub fields: Vec<(String, String)>,
}

impl BasicFileInfo {
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn parse(data: &[u8]) -> Result<BasicFileInfo, String> {
        let layout = u32_at(data, 0).ok_or("BasicFileInfo is shorter than its layout word")?;
        // Length-prefixed strings (an i32 count of UTF-16 units) from offset
        // 14: the release and the build (layout 13 and later), or one text
        // naming both (layout 10), then the original path.
        let mut strings = Vec::new();
        let mut at = 14;
        while strings.len() < 3 {
            let Some(n) = u32_at(data, at) else { break };
            let n = n as usize;
            let Some(bytes) = data.get(at + 4..at + 4 + 2 * n) else {
                break;
            };
            strings.push(utf16(bytes));
            at += 4 + 2 * n;
        }
        let mut info = BasicFileInfo {
            layout,
            ..BasicFileInfo::default()
        };
        info.fields = text_block(data)
            .into_iter()
            .filter(|(k, _)| !PRIVATE_KEYS.contains(&k.as_str()))
            .collect();
        info.release = info
            .field("Format")
            .and_then(year)
            .or_else(|| strings.first().and_then(|s| year(s)));
        info.build = if layout >= 13 {
            strings.get(1).cloned()
        } else {
            strings.first().and_then(|s| {
                let start = s.find("Build: ")? + 7;
                let rest = &s[start..];
                Some(rest.strip_suffix(')').unwrap_or(rest).to_string())
            })
        }
        .filter(|b| !b.is_empty());
        Ok(info)
    }
}

/// The first number of four digits from 2000 on in a text.
fn year(text: &str) -> Option<u32> {
    let bytes = text.as_bytes();
    (0..bytes.len().saturating_sub(3)).find_map(|i| {
        let digits = bytes.get(i..i + 4)?;
        let before = i
            .checked_sub(1)
            .map(|j| bytes[j].is_ascii_digit())
            .unwrap_or(false);
        let after = bytes.get(i + 4).is_some_and(u8::is_ascii_digit);
        if digits.iter().all(u8::is_ascii_digit) && !before && !after {
            let y: u32 = std::str::from_utf8(digits).ok()?.parse().ok()?;
            (2000..2100).contains(&y).then_some(y)
        } else {
            None
        }
    })
}

/// The trailing UTF-16 text block of `Key: value` lines, found by its
/// first key at either byte parity.
fn text_block(data: &[u8]) -> Vec<(String, String)> {
    let key: Vec<u8> = "Worksharing:"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let Some(start) = data.windows(key.len()).position(|w| w == key.as_slice()) else {
        return Vec::new();
    };
    let text = utf16(&data[start..]);
    text.split(['\r', '\n'])
        .filter_map(|line| {
            let (k, v) = line
                .split_once(": ")
                .or_else(|| line.strip_suffix(':').map(|k| (k, "")))?;
            let k = k
                .trim_matches(|c: char| c.is_control() || c == '\u{feff}')
                .trim();
            (!k.is_empty() && k.chars().all(|c| !c.is_control()))
                .then(|| (k.to_string(), v.trim_end_matches('\0').trim().to_string()))
        })
        .collect()
}

pub(crate) fn utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16_lossy(&units)
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// A family's `PartAtom` (Atom XML): its title and categories.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PartAtom {
    pub title: Option<String>,
    pub id: Option<String>,
    pub updated: Option<String>,
    /// The `category` elements: term and scheme.
    pub categories: Vec<(String, String)>,
}

impl PartAtom {
    pub fn parse(data: &[u8]) -> PartAtom {
        let text = String::from_utf8_lossy(data);
        let first = |tag: &str| {
            elements(&text, tag)
                .into_iter()
                .next()
                .map(|t| unescape(t.trim()))
        };
        PartAtom {
            title: first("title"),
            id: first("id"),
            updated: first("updated"),
            categories: elements(&text, "category")
                .into_iter()
                .map(|c| {
                    let get = |tag: &str| {
                        elements(c, tag)
                            .first()
                            .map(|t| unescape(t.trim()))
                            .unwrap_or_default()
                    };
                    (get("term"), get("scheme"))
                })
                .collect(),
        }
    }
}

/// The contents of every `<tag ...>...</tag>` element (any namespace
/// prefix), in order; nested elements of the same name are not split.
pub fn elements<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(found) = text[at..].find('<') {
        let open = at + found;
        let Some(close) = text[open..].find('>').map(|c| open + c) else {
            break;
        };
        let head = &text[open + 1..close];
        let name = head
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default();
        let local = name.rsplit(':').next().unwrap_or(name);
        if local == tag && !head.starts_with('/') {
            if head.ends_with('/') {
                out.push("");
                at = close + 1;
                continue;
            }
            let end_tag = format!("</{name}>");
            match text[close + 1..].find(&end_tag) {
                Some(e) => {
                    out.push(&text[close + 1..close + 1 + e]);
                    at = close + 1 + e + end_tag.len();
                }
                None => break,
            }
        } else {
            at = close + 1;
        }
    }
    out
}

/// XML's predefined entities and numeric references decoded.
pub fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let Some(semi) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let entity = &tail[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &tail[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// One external file of `TransmissionData`: a linked model, an image, a
/// CAD file.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ExternalFile {
    pub element_id: Option<i64>,
    /// `ExternalFileReferenceType` (`RevitLink`, ... as the file writes it).
    pub kind: String,
    /// The file's name (the path's last part; the folders are not kept).
    pub name: String,
    pub load_state: String,
}

/// `TransmissionData`: a `u32` count of UTF-16 units, then XML.
pub fn transmission_data(data: &[u8]) -> Vec<ExternalFile> {
    let Some(units) = u32_at(data, 0) else {
        return Vec::new();
    };
    let end = (4 + 2 * units as usize).min(data.len());
    let text = utf16(&data[4.min(end)..end]);
    elements(&text, "ExternalFileReference")
        .into_iter()
        .map(|block| {
            let get = |tag: &str| {
                elements(block, tag)
                    .first()
                    .map(|t| unescape(t.trim()))
                    .unwrap_or_default()
            };
            let path = get("DesiredPath");
            let path = if path.is_empty() {
                get("LastSavedPath")
            } else {
                path
            };
            ExternalFile {
                element_id: get("ElementId").parse().ok(),
                kind: get("ExternalFileReferenceType"),
                name: path
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                load_state: get("DesiredLoadState"),
            }
        })
        .collect()
}

/// The PNG signature.
pub const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The PNG image in a preview stream: from its signature to the end of its
/// `IEND` chunk.
pub fn preview_png(data: &[u8]) -> Option<Vec<u8>> {
    let start = data.windows(8).position(|w| w == PNG_SIGNATURE)?;
    let mut at = start + 8;
    loop {
        let len = u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?) as usize;
        let kind = data.get(at + 4..at + 8)?;
        let next = at.checked_add(12)?.checked_add(len)?;
        if next > data.len() {
            return None;
        }
        if kind == b"IEND" {
            return Some(data[start..next].to_vec());
        }
        at = next;
    }
}

/// The width and height of a PNG image.
pub fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    let w = u32::from_be_bytes(png.get(16..20)?.try_into().ok()?);
    let h = u32::from_be_bytes(png.get(20..24)?.try_into().ok()?);
    Some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16_bytes(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn basic_file_info_of_both_layouts() {
        let new = crate::testfile::basic_file_info(2024, "20230509_0315(x64)");
        let info = BasicFileInfo::parse(&new).unwrap();
        assert_eq!(info.layout, 14);
        assert_eq!(info.release, Some(2024));
        assert_eq!(info.build.as_deref(), Some("20230509_0315(x64)"));
        assert_eq!(info.field("Format"), Some("2024"));
        assert_eq!(info.field("Locale when saved"), Some("ENU"));
        assert!(info.field("Username").is_none());

        let mut old = 10u32.to_le_bytes().to_vec();
        old.extend([0u8; 10]);
        let text = "Program 2016 (Build: 20150129_1515(x64))";
        old.extend((text.encode_utf16().count() as u32).to_le_bytes());
        old.extend(utf16_bytes(text));
        old.extend(0u32.to_le_bytes());
        old.push(0);
        old.extend(utf16_bytes(
            "Worksharing: Not enabled\r\nUsername: someone\r\nLocale when saved: ENU\r\n",
        ));
        let info = BasicFileInfo::parse(&old).unwrap();
        assert_eq!(info.layout, 10);
        assert_eq!(info.release, Some(2016));
        assert_eq!(info.build.as_deref(), Some("20150129_1515(x64)"));
        assert_eq!(info.field("Worksharing"), Some("Not enabled"));
        assert!(info.fields.iter().all(|(k, _)| k != "Username"));
    }

    #[test]
    fn part_atom_and_links() {
        let atom = PartAtom::parse(crate::testfile::PART_ATOM.as_bytes());
        assert_eq!(atom.title.as_deref(), Some("Test Family & Co"));
        assert_eq!(atom.categories[0].0, "Furniture");
        let files = transmission_data(&crate::testfile::transmission_data());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "linked.rvt");
        assert_eq!(files[0].kind, "Link");
        assert_eq!(files[0].element_id, Some(1234));
        assert_eq!(unescape("a&lt;b&#x41;&#66;&bogus;"), "a<bAB&bogus;");
    }

    #[test]
    fn preview_image() {
        let stream = crate::testfile::preview();
        let png = preview_png(&stream).unwrap();
        assert_eq!(png_size(&png), Some((2, 1)));
        assert!(png.ends_with(&[0xAE, 0x42, 0x60, 0x82]));
        assert!(preview_png(&stream[..stream.len() - 11]).is_none());
    }
}
