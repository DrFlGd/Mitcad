// SPDX-License-Identifier: MIT
//! Reader for `.rvt` project files and `.rfa` family files (and their
//! templates, `.rte` and `.rft`). See `README.md` for the format as far as
//! it is implemented.
//!
//! Layers, bottom up:
//! - the compound file container (`mitcad_cfb`, Microsoft's published
//!   MS-CFB specification);
//! - [`gzip`]: page trailers and gzip members;
//! - [`info`]: the metadata streams (release, family Atom XML, linked
//!   files, preview);
//! - [`schema`]: the file's own serialisation schema (`Formats/Latest`);
//! - [`walk`]: records walked by that schema;
//! - [`partition`]: the element records of the partition streams;
//! - [`elements`]: the element table and typed views of the elements
//!   (levels, categories, families and types, materials, transforms,
//!   bounding boxes).
//!
//! There is no geometry here: the import builds bodies from these views
//! through the model's geometry kernel.

pub mod building;
pub mod categories;
pub mod elements;
pub mod gzip;
pub mod info;
pub mod partition;
pub mod schema;
#[doc(hidden)]
pub mod testfile;
pub mod walk;

use std::fmt;
use std::path::Path;

pub use mitcad_cfb as cfb;
use mitcad_cfb::{CfbError, CompoundFile, EntryKind};

use crate::info::{BasicFileInfo, ExternalFile, PartAtom};
use crate::partition::Partition;
use crate::schema::Schema;

/// The stream that holds the preview image (as the files name it).
pub const PREVIEW_STREAM: &str = "RevitPreview4.0";

/// The wrapper magic of the preview and `Contents` streams.
pub const WRAPPER_MAGIC: [u8; 4] = [0x62, 0x19, 0x22, 0x05];

/// The `Global/ElemTable` class's tag by release: the table's first word
/// (the same in families and projects; a check of the release and of the
/// schema).
pub const ELEM_TABLE_TAGS: [(u32, u16); 12] = [
    (2016, 1174),
    (2017, 1191),
    (2018, 1232),
    (2019, 1233),
    (2020, 1244),
    (2021, 1271),
    (2022, 1312),
    (2023, 1370),
    (2024, 1411),
    (2025, 1451),
    (2026, 1481),
    (2027, 1510),
];

#[derive(Debug)]
pub enum RvtError {
    Container(CfbError),
    /// A stream the file must have is missing.
    Missing(String),
    /// A stream does not read as it should.
    Stream(String),
}

impl fmt::Display for RvtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RvtError::Container(e) => write!(f, "{e}"),
            RvtError::Missing(s) => write!(f, "no {s} stream: not a project or family file"),
            RvtError::Stream(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for RvtError {}

impl From<CfbError> for RvtError {
    fn from(e: CfbError) -> Self {
        RvtError::Container(e)
    }
}

/// What the file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A project (`.rvt`, `.rte`): no `PartAtom`.
    Project,
    /// A family (`.rfa`, `.rft`): has a `PartAtom`.
    Family,
}

/// A project or family file, open.
pub struct RvtFile {
    pub container: CompoundFile,
    pub info: BasicFileInfo,
    pub kind: Kind,
}

impl RvtFile {
    pub fn open(path: &Path) -> Result<RvtFile, RvtError> {
        let data = std::fs::read(path)
            .map_err(|e| RvtError::Container(CfbError::Io(format!("{}: {e}", path.display()))))?;
        RvtFile::parse(data)
    }

    pub fn parse(data: Vec<u8>) -> Result<RvtFile, RvtError> {
        let container = CompoundFile::parse(data)?;
        let bytes = container
            .read_path("/BasicFileInfo")
            .map_err(|_| RvtError::Missing("BasicFileInfo".into()))?;
        let info = BasicFileInfo::parse(&bytes).map_err(RvtError::Stream)?;
        let kind = if container.find("/PartAtom").is_some() {
            Kind::Family
        } else {
            Kind::Project
        };
        Ok(RvtFile {
            container,
            info,
            kind,
        })
    }

    /// Every stream with its size, by path.
    pub fn streams(&self) -> Vec<(String, u64)> {
        self.container
            .entries()
            .filter(|(_, e)| e.kind == EntryKind::Stream)
            .map(|(p, e)| (p.to_string(), e.size))
            .collect()
    }

    /// A stream's stored bytes.
    pub fn raw(&self, path: &str) -> Result<Vec<u8>, RvtError> {
        let entry = self
            .container
            .find(path)
            .ok_or_else(|| RvtError::Missing(path.trim_start_matches('/').to_string()))?;
        Ok(self.container.read(entry)?)
    }

    /// A stream's contents: paged streams with their trailers removed and
    /// their gzip members inflated (after the prefix their kind has), the
    /// others as stored.
    pub fn inflated(&self, path: &str) -> Result<Vec<u8>, RvtError> {
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        let raw = self.raw(&path)?;
        let fail = |e: String| RvtError::Stream(format!("{}: {e}", path.trim_start_matches('/')));
        let members = |stream: &[u8], start: usize| -> Result<Vec<u8>, RvtError> {
            Ok(gzip::members_from(stream, start)
                .map_err(fail)?
                .into_iter()
                .flat_map(|m| m.data)
                .collect())
        };
        if path == "/Formats/Latest" {
            members(&gzip::strip_pages(&raw), 0)
        } else if path.starts_with("/Global/") {
            members(&gzip::strip_pages(&raw), 8)
        } else if path.starts_with("/Partitions/") {
            members(&gzip::strip_pages(&raw), partition::PARTITION_HEADER)
        } else if path == "/Contents" {
            let start = gzip::find_magic(&raw, 0).ok_or_else(|| fail("no gzip member".into()))?;
            members(&raw, start)
        } else {
            Ok(raw)
        }
    }

    /// The release that saved the file.
    pub fn release(&self) -> Option<u32> {
        self.info.release
    }

    pub fn schema(&self) -> Result<Schema, RvtError> {
        let data = self.inflated("/Formats/Latest")?;
        Schema::parse(&data).map_err(|e| RvtError::Stream(format!("Formats/Latest: {e}")))
    }

    pub fn part_atom(&self) -> Option<PartAtom> {
        Some(PartAtom::parse(&self.raw("/PartAtom").ok()?))
    }

    pub fn external_files(&self) -> Vec<ExternalFile> {
        self.raw("/TransmissionData")
            .map(|d| info::transmission_data(&d))
            .unwrap_or_default()
    }

    /// The editing episodes' GUIDs (`Global/History`), indexed by the
    /// element table's `EpisodeId`s counted from the last (newest first).
    pub fn episodes(&self) -> Vec<[u8; 16]> {
        self.inflated("/Global/History")
            .map(|d| info::episodes(&d))
            .unwrap_or_default()
    }

    /// The preview image (PNG), when the file has one.
    pub fn preview(&self) -> Option<Vec<u8>> {
        info::preview_png(&self.raw(PREVIEW_STREAM).ok()?)
    }

    /// The partition streams' numbers, ascending.
    pub fn partition_numbers(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .container
            .children("/Partitions")
            .filter_map(|(p, e)| {
                (e.kind == EntryKind::Stream)
                    .then(|| p.rsplit('/').next()?.parse().ok())
                    .flatten()
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// A partition's element records.
    pub fn partition(&self, number: u32, schema: &Schema) -> Result<Partition, RvtError> {
        let raw = self.raw(&format!("/Partitions/{number}"))?;
        Partition::read(number, &gzip::strip_pages(&raw), schema.id_width)
            .map_err(|e| RvtError::Stream(format!("Partitions/{number}: {e}")))
    }

    /// Every partition, ascending by number.
    pub fn partitions(&self, schema: &Schema) -> Result<Vec<Partition>, RvtError> {
        self.partition_numbers()
            .into_iter()
            .map(|n| self.partition(n, schema))
            .collect()
    }

    /// The release the element table's class tag names
    /// ([`ELEM_TABLE_TAGS`]), a check of `BasicFileInfo`'s.
    pub fn release_by_table(&self) -> Option<u32> {
        let table = self.inflated("/Global/ElemTable").ok()?;
        let tag = u16::from_le_bytes(table.get(..2)?.try_into().ok()?);
        ELEM_TABLE_TAGS
            .iter()
            .find(|(_, t)| *t == tag)
            .map(|(release, _)| *release)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_a_written_project() {
        let file = RvtFile::parse(testfile::project()).unwrap();
        assert_eq!(file.kind, Kind::Project);
        assert_eq!(file.release(), Some(2024));
        assert_eq!(file.container.version, 4);
        let streams: Vec<String> = file.streams().into_iter().map(|(p, _)| p).collect();
        for s in [
            "/BasicFileInfo",
            "/Formats/Latest",
            "/Global/ElemTable",
            "/Partitions/7",
        ] {
            assert!(streams.iter().any(|p| p == s), "{s}");
        }
        assert!(file.preview().is_some());
        assert_eq!(file.external_files().len(), 1);
        assert_eq!(file.partition_numbers(), vec![5, 7]);
        let schema = file.schema().unwrap();
        assert_eq!(schema.id_width, 8);
        assert!(schema.unknown_encodings().is_empty());
        assert_eq!(
            file.release_by_table(),
            None,
            "the test schema's tags are its own"
        );
    }

    #[test]
    fn opens_a_written_family() {
        let file = RvtFile::parse(testfile::family()).unwrap();
        assert_eq!(file.kind, Kind::Family);
        assert_eq!(
            file.part_atom().unwrap().title.as_deref(),
            Some("Test Family & Co")
        );
        assert_eq!(file.release(), Some(2020));
        assert_eq!(file.schema().unwrap().id_width, 4);
    }

    #[test]
    fn other_files_are_refused() {
        assert!(matches!(
            RvtFile::parse(b"not a compound file at all, long enough".repeat(20)),
            Err(RvtError::Container(CfbError::NotCompound))
        ));
        let mut w = cfb::Writer::new();
        w.stream("Other", b"x");
        assert!(matches!(
            RvtFile::parse(w.finish()),
            Err(RvtError::Missing(_))
        ));
    }
}
