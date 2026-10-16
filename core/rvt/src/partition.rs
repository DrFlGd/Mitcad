// SPDX-License-Identifier: MIT
//! The partition streams (`Partitions/NN`): a 44-byte header, then gzip
//! members whose inflated bytes hold the elements in two chains of
//! records, each ended by a record whose id is all ones:
//!
//! - the element headers: `id`, `u32` size, the `ElementHeader` object
//!   (its `u16` class tag and fields), the size again; after the end
//!   record a `u64` 0;
//! - the element objects, in the same order: `id`, a `u32` (not read),
//!   `u32` size, the element's object, the size again; after the end
//!   record a `u64` 1.
//!
//! An id is as wide as the schema's ElementId (4 bytes through the 2023
//! release, 8 from 2024). What follows the two chains (the loaded
//! families' own documents) is not read, nor inflated: members are
//! inflated only as far as the chains go.
//!
//! An element can have a record in several partitions; the one in the
//! partition with the highest number is the element as last saved.

use serde::Serialize;

use crate::gzip::Members;

/// Bytes before a partition's first gzip member.
pub const PARTITION_HEADER: usize = 44;

/// One record of a chain: where its object is in [`Partition::data`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Slot {
    pub id: i64,
    pub offset: usize,
    pub size: usize,
}

/// A partition's chains.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Partition {
    /// The stream's number (its name).
    pub number: u32,
    /// The header's first word (7, 8 or 9 by release).
    pub format: u64,
    pub headers: Vec<Slot>,
    pub objects: Vec<Slot>,
    /// The third chain of a document read by [`Documents`]: each element's
    /// geometry (a `GElement`; a 2-byte `SerializedDummy` for most).
    pub geometry: Vec<Slot>,
    /// Gzip members inflated (up to the end of the chains).
    pub members: usize,
    /// Why a chain stopped before its end record, if one did.
    pub stopped: Option<String>,
    /// The inflated bytes up to the end of the chains.
    #[serde(skip)]
    pub data: Vec<u8>,
}

impl Partition {
    /// The bytes of a slot's object (from its class tag).
    pub fn bytes(&self, slot: &Slot) -> &[u8] {
        &self.data[slot.offset..slot.offset + slot.size]
    }

    /// Reads a partition stream with its page trailers removed.
    pub fn read(number: u32, stream: &[u8], id_width: usize) -> Result<Partition, String> {
        let format = stream
            .get(..8)
            .map(|b| u64::from_le_bytes(b.try_into().expect("8 bytes")))
            .ok_or("the partition is shorter than its header")?;
        let mut members = Members::new(stream, PARTITION_HEADER);
        let mut partition = Partition {
            number,
            format,
            ..Partition::default()
        };
        let mut at = 0;
        for (chain, extra) in [(0u64, 0usize), (1, 4)] {
            let (slots, end) = read_chain(&mut members, &mut at, id_width, chain, extra)?;
            if chain == 0 {
                partition.headers = slots;
            } else {
                partition.objects = slots;
            }
            if let Err(e) = end {
                partition.stopped = Some(e);
                break;
            }
        }
        if partition.stopped.is_none() && partition.headers.len() != partition.objects.len() {
            partition.stopped = Some(format!(
                "{} headers but {} objects",
                partition.headers.len(),
                partition.objects.len()
            ));
        }
        partition.members = members.count;
        members.inflated.truncate(at.min(members.inflated.len()));
        partition.data = members.inflated;
        Ok(partition)
    }
}

/// A chain of records from `at` to its end record (whose mark must be
/// `chain`): the slots, and why it stopped early if it did. `extra` is the
/// word between a record's id and its size (4 in object chains).
fn read_chain(
    members: &mut Members<'_>,
    at: &mut usize,
    id_width: usize,
    chain: u64,
    extra: usize,
) -> Result<(Vec<Slot>, Result<(), String>), String> {
    let mut slots = Vec::new();
    let end = loop {
        let head = id_width + extra + 4;
        if !members.ensure(*at + head)? {
            break Err(format!("the stream ends inside chain {chain} at {at:#x}"));
        }
        let d = &members.inflated;
        let id = read_id(d, *at, id_width);
        if id == -1 {
            if !members.ensure(*at + id_width + 8)? {
                break Err(format!("the stream ends after chain {chain}"));
            }
            let d = &members.inflated;
            let mark = u64::from_le_bytes(
                d[*at + id_width..*at + id_width + 8]
                    .try_into()
                    .expect("8 bytes"),
            );
            *at += id_width + 8;
            if mark != chain {
                break Err(format!("chain {chain} ends with mark {mark}"));
            }
            break Ok(());
        }
        let size = u32_at(d, *at + id_width + extra) as usize;
        if size < 2 {
            break Err(format!("a record of {size} bytes at {at:#x}"));
        }
        if !members.ensure(*at + head + size + 4)? {
            break Err(format!("record {id} at {at:#x} runs past the stream"));
        }
        let d = &members.inflated;
        if u32_at(d, *at + head + size) as usize != size {
            break Err(format!(
                "record {id} at {at:#x}: its size is not repeated after it"
            ));
        }
        slots.push(Slot {
            id,
            offset: *at + head,
            size,
        });
        *at += head + size + 4;
    };
    Ok((slots, end))
}

/// The documents a partition carries after its own two chains: the loaded
/// families' documents (*seen*), each its own element headers and objects
/// in the same two chains, its ids its own. After each document's chains
/// (the partition's own included) come a `u32` and a third chain of
/// object records, one per element (2 bytes for most; not read); the next
/// document starts with a `u32` before its headers.
#[derive(Debug, Clone, Default)]
pub struct Documents {
    /// Each document's chains (the partition's own first, then the
    /// families'), their bytes in [`Documents::data`].
    pub documents: Vec<Partition>,
    /// Why the documents stopped before the stream's end, if they did.
    pub stopped: Option<String>,
    /// The whole stream inflated.
    pub data: Vec<u8>,
}

impl Documents {
    /// The bytes of a slot's object.
    pub fn bytes(&self, slot: &Slot) -> &[u8] {
        &self.data[slot.offset..slot.offset + slot.size]
    }

    /// Reads the partition's own document with its third chain and, with
    /// `families`, every document after it (else the stream is inflated only
    /// as far as the partition's own third chain goes).
    pub fn read(
        number: u32,
        stream: &[u8],
        id_width: usize,
        families: bool,
    ) -> Result<Documents, String> {
        let mut members = Members::new(stream, PARTITION_HEADER);
        let mut out = Documents::default();
        let mut at = 0;
        let mut first = true;
        let stopped = 'docs: loop {
            let mut doc = Partition {
                number,
                ..Partition::default()
            };
            for (chain, extra) in [(0u64, 0usize), (1, 4)] {
                if !first && chain == 0 {
                    // The word before a document's headers.
                    at += 4;
                }
                let (slots, end) = read_chain(&mut members, &mut at, id_width, chain, extra)?;
                if chain == 0 {
                    doc.headers = slots;
                } else {
                    doc.objects = slots;
                }
                if let Err(e) = end {
                    break 'docs Some(e);
                }
            }
            first = false;
            // The word and the third chain.
            at += 4;
            let (geometry, end) = read_chain(&mut members, &mut at, id_width, 1, 4)?;
            doc.geometry = geometry;
            out.documents.push(doc);
            if let Err(e) = end {
                break Some(format!("the third chain: {e}"));
            }
            if !families || !members.ensure(at + 5)? {
                break None;
            }
        };
        out.stopped = stopped;
        out.data = members.inflated;
        Ok(out)
    }
}

fn read_id(d: &[u8], at: usize, width: usize) -> i64 {
    if width == 8 {
        i64::from_le_bytes(d[at..at + 8].try_into().expect("8 bytes"))
    } else {
        i64::from(i32::from_le_bytes(
            d[at..at + 4].try_into().expect("4 bytes"),
        ))
    }
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(d[at..at + 4].try_into().expect("4 bytes"))
}

/// Writes a partition stream (without page trailers) of element records:
/// (id, header object, element object) each; for test files.
pub fn write_partition(
    format: u64,
    id_width: usize,
    records: &[(i64, Vec<u8>, Vec<u8>)],
) -> Vec<u8> {
    let id = |out: &mut Vec<u8>, v: i64| {
        if id_width == 8 {
            out.extend(v.to_le_bytes());
        } else {
            out.extend((v as i32).to_le_bytes());
        }
    };
    let mut inflated = Vec::new();
    for (element, header, _) in records {
        id(&mut inflated, *element);
        inflated.extend((header.len() as u32).to_le_bytes());
        inflated.extend(header);
        inflated.extend((header.len() as u32).to_le_bytes());
    }
    id(&mut inflated, -1);
    inflated.extend(0u64.to_le_bytes());
    for (element, _, object) in records {
        id(&mut inflated, *element);
        inflated.extend(0x1234_5678u32.to_le_bytes());
        inflated.extend((object.len() as u32).to_le_bytes());
        inflated.extend(object);
        inflated.extend((object.len() as u32).to_le_bytes());
    }
    id(&mut inflated, -1);
    inflated.extend(1u64.to_le_bytes());
    // Whatever the files keep after the chains: not read.
    inflated.extend([0xAB; 64]);
    let mut out = format.to_le_bytes().to_vec();
    out.resize(PARTITION_HEADER, 0);
    // Several members, as the files have them.
    for chunk in inflated.chunks(40) {
        out.extend(crate::gzip::gzip(chunk));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chains_of_both_id_widths() {
        for width in [4, 8] {
            let records = vec![
                (5, vec![1, 0, 9], vec![2, 0, 1, 2, 3]),
                (70_000, vec![1, 0], vec![3, 0]),
            ];
            let stream = write_partition(9, width, &records);
            let p = Partition::read(46, &stream, width).unwrap();
            assert_eq!(p.stopped, None, "width {width}");
            assert_eq!(p.headers.len(), 2);
            assert_eq!(p.objects[1].id, 70_000);
            assert_eq!(p.bytes(&p.headers[0]), &[1, 0, 9]);
            assert_eq!(p.bytes(&p.objects[0]), &[2, 0, 1, 2, 3]);
            assert_eq!(p.format, 9);
            // The bytes after the chains are not kept.
            assert!(!p.data.ends_with(&[0xAB]));
        }
    }

    #[test]
    fn a_broken_chain_is_reported() {
        let records = vec![(5, vec![1, 0, 9], vec![2, 0, 1])];
        let stream = write_partition(9, 8, &records);
        let mut inflated = crate::gzip::members_from(&stream, PARTITION_HEADER)
            .unwrap()
            .into_iter()
            .flat_map(|m| m.data)
            .collect::<Vec<u8>>();
        // The header record's repeated size.
        inflated[8 + 4 + 3] = 7;
        let mut damaged = stream[..PARTITION_HEADER].to_vec();
        damaged.extend(crate::gzip::gzip(&inflated));
        let p = Partition::read(1, &damaged, 8).unwrap();
        assert!(p.stopped.unwrap().contains("not repeated"));
        assert!(p.headers.is_empty());
    }
}
