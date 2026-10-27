// SPDX-License-Identifier: MIT
//! The library sources of a design (`library_sources`): each library part
//! that BUILD placed, kept once whatever its row or length, with the
//! library record, the design's text and the manifest's `build`,
//! `supplier`, `part_number` and `supplier_url` as they were at that
//! version. A single-file design keeps the text inside; a design in a
//! project keeps it in the project's store, as base features keep their
//! B-rep data (`file/mod.rs`). Derived parts are made from it, so the design
//! needs no library to open or recompute.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::SourceId;
use crate::document::DocState;
use crate::file::{BlobStore, Sha256};
use crate::library::{LibraryRef, LibrarySource};

/// A source's design: its text (none while a project's reference is not
/// resolved) and the definition state read from it once.
#[derive(Clone)]
pub struct SourceDesign {
    pub sha256: Sha256,
    text: Option<Arc<str>>,
    state: Arc<OnceLock<Result<Arc<DocState>, String>>>,
}

impl SourceDesign {
    /// A design with its text.
    pub fn new(text: String) -> Self {
        Self {
            sha256: Sha256::of(text.as_bytes()),
            text: Some(Arc::from(text)),
            state: Arc::default(),
        }
    }

    /// A reference into a project's store, not yet read.
    pub fn reference(sha256: Sha256) -> Self {
        Self {
            sha256,
            text: None,
            state: Arc::default(),
        }
    }

    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn is_reference(&self) -> bool {
        self.text.is_none()
    }

    /// The design's definition state, read once.
    pub fn state(&self) -> Result<Arc<DocState>, String> {
        let Some(text) = &self.text else {
            return Err(format!(
                "its design {} is missing from the project store",
                self.sha256
            ));
        };
        self.state
            .get_or_init(|| {
                crate::file::load_state(text)
                    .map(|(state, _)| Arc::new(state))
                    .map_err(|e| format!("its design cannot be read: {e}"))
            })
            .clone()
    }
}

impl fmt::Debug for SourceDesign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceDesign")
            .field("sha256", &self.sha256)
            .field("resolved", &self.text.is_some())
            .finish()
    }
}

impl PartialEq for SourceDesign {
    fn eq(&self, other: &Self) -> bool {
        self.sha256 == other.sha256 && self.text.is_some() == other.text.is_some()
    }
}

/// One library source (`S1`).
#[derive(Debug, Clone, PartialEq)]
pub struct LibrarySourceDef {
    pub id: SourceId,
    /// The library, its version, the component and its path, licence and
    /// authors; `designation` is the library's pattern (`Profile 30x30
    /// B8 L={L}`), `config` is left out (each derived part has its row).
    pub library: LibraryRef,
    /// The component's name and standard in the library.
    pub name: String,
    pub standard: String,
    pub design: SourceDesign,
    /// The manifest's `build` block at that version.
    pub build: Option<Value>,
    pub supplier: String,
    pub part_number: String,
    pub supplier_url: String,
}

impl LibrarySourceDef {
    /// What derived parts read of it, for the recompute cache: its design
    /// and its `build`; None while its design is not resolved.
    pub(crate) fn version(&self) -> Option<u128> {
        self.design.text()?;
        let mut fingerprint = crate::fingerprint::Fingerprint::new("library source");
        fingerprint.str(&self.design.sha256.to_string());
        serde_json::to_writer(&mut fingerprint, &self.build).expect("JSON values serialize");
        Some(fingerprint.finish())
    }

    /// The source as BUILD reads it from a library (the class `remote`
    /// commands): a design whose B-rep data is in the library's store is
    /// written again as a single file, so the text holds everything.
    pub fn from_library(id: SourceId, source: &LibrarySource) -> Result<Self, String> {
        let text = if source.store.is_empty() {
            source.text.clone()
        } else {
            let (state, _) = crate::file::load_state_in(&source.text, &source.store)
                .map_err(|e| format!("{} in {}: {e}", source.path, source.library))?;
            if crate::file::has_references(&state) {
                return Err(format!(
                    "{} in {}: B-rep data it refers to is missing",
                    source.path, source.library
                ));
            }
            crate::file::single_file(&state)
        };
        Ok(Self {
            id,
            library: LibraryRef {
                library: source.library.clone(),
                url: source.url.clone(),
                rev: source.rev.clone(),
                label: source.label.clone(),
                component: source.component.clone(),
                path: source.path.clone(),
                config: None,
                license: source.license.clone(),
                authors: source.authors.clone(),
                designation: source.designation.clone(),
                supplier: source.supplier.clone().unwrap_or_default(),
                part_number: source.part_number.clone().unwrap_or_default(),
                supplier_url: source.supplier_url.clone().unwrap_or_default(),
            },
            name: source.name.clone(),
            standard: source.standard.clone(),
            design: SourceDesign::new(text),
            build: source.build.clone(),
            supplier: source.supplier.clone().unwrap_or_default(),
            part_number: source.part_number.clone().unwrap_or_default(),
            supplier_url: source.supplier_url.clone().unwrap_or_default(),
        })
    }

    /// The source's name for people: the component's name, else its id in
    /// the library.
    pub fn label(&self) -> String {
        if self.name.trim().is_empty() {
            self.library.component.clone()
        } else {
            self.name.clone()
        }
    }
}

/// The design's sources, in the order they were added.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibrarySources(pub Vec<LibrarySourceDef>);

impl LibrarySources {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &LibrarySourceDef> {
        self.0.iter()
    }

    pub fn get(&self, id: SourceId) -> Option<&LibrarySourceDef> {
        self.0.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: SourceId) -> Option<&mut LibrarySourceDef> {
        self.0.iter_mut().find(|s| s.id == id)
    }

    /// The id the next source takes.
    pub fn next_id(&self) -> SourceId {
        SourceId(self.0.iter().map(|s| s.id.0).max().unwrap_or(0) + 1)
    }

    /// The source of a library's component at a commit, if the design has
    /// it.
    pub fn find(&self, library: &str, component: &str, rev: &str) -> Option<SourceId> {
        self.0
            .iter()
            .find(|s| {
                s.library.library == library
                    && s.library.component == component
                    && s.library.rev == rev
            })
            .map(|s| s.id)
    }

    pub(crate) fn version(&self, id: SourceId) -> Option<u128> {
        self.get(id).and_then(LibrarySourceDef::version)
    }

    /// Removes the given sources.
    pub(crate) fn remove_all(&mut self, ids: &BTreeSet<SourceId>) {
        self.0.retain(|s| !ids.contains(&s.id));
    }

    /// Gives references their text from a project's store; a text the store
    /// lacks, or one that is not the design it is named after, leaves the
    /// reference (its derived parts fail) and adds a warning.
    pub(crate) fn resolve(&mut self, store: &dyn BlobStore, warnings: &mut Vec<String>) {
        for (i, source) in self.0.iter_mut().enumerate() {
            if !source.design.is_reference() {
                continue;
            }
            let sha256 = source.design.sha256;
            let problem = match store.get(&sha256) {
                Ok(Some(stored)) => match miniz_oxide::inflate::decompress_to_vec_zlib(&stored)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                {
                    Some(text) if Sha256::of(text.as_bytes()) == sha256 => {
                        source.design = SourceDesign::new(text);
                        continue;
                    }
                    _ => format!("the file {} is damaged", crate::file::brep_path(&sha256)),
                },
                Ok(None) => format!("its design {sha256} is missing from the project store"),
                Err(e) => format!("cannot read {}: {e}", crate::file::brep_path(&sha256)),
            };
            warnings.push(format!("library_sources[{i}] ({}): {problem}", source.id));
        }
    }

    /// Whether a source's design is a reference into a project's store.
    pub(crate) fn has_references(&self) -> bool {
        self.0.iter().any(|s| s.design.is_reference())
    }

    /// The sources with their texts put into `store` and kept there as
    /// references (a design in a project).
    pub(crate) fn to_references(&self, store: &dyn BlobStore) -> std::io::Result<Self> {
        let mut out = self.clone();
        for source in &mut out.0 {
            let Some(text) = source.design.text() else {
                continue;
            };
            let sha256 = source.design.sha256;
            if !store.contains(&sha256)? {
                let stored = miniz_oxide::deflate::compress_to_vec_zlib(text.as_bytes(), 6);
                store.put(&sha256, &stored)?;
            }
            source.design = SourceDesign::reference(sha256);
        }
        Ok(out)
    }
}

// The design file's `library_sources`.

#[derive(Serialize)]
pub(crate) struct SourceOut<'a> {
    id: SourceId,
    library: &'a LibraryRef,
    #[serde(skip_serializing_if = "str::is_empty")]
    name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    standard: &'a str,
    design: DesignOut<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<&'a Value>,
    #[serde(skip_serializing_if = "str::is_empty")]
    supplier: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    part_number: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    supplier_url: &'a str,
}

#[derive(Serialize)]
struct DesignOut<'a> {
    sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
}

impl<'a> SourceOut<'a> {
    pub(crate) fn of(source: &'a LibrarySourceDef) -> Self {
        Self {
            id: source.id,
            library: &source.library,
            name: &source.name,
            standard: &source.standard,
            design: DesignOut {
                sha256: source.design.sha256.to_string(),
                text: source.design.text(),
            },
            build: source.build.as_ref(),
            supplier: &source.supplier,
            part_number: &source.part_number,
            supplier_url: &source.supplier_url,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceIn {
    id: SourceId,
    library: LibraryRef,
    #[serde(default)]
    name: String,
    #[serde(default)]
    standard: String,
    design: DesignIn,
    #[serde(default)]
    build: Option<Value>,
    #[serde(default)]
    supplier: String,
    #[serde(default)]
    part_number: String,
    #[serde(default)]
    supplier_url: String,
}

/// A design's text, or its SHA-256 alone (a reference into a project's
/// store). A hand-written file may give the design itself as an object
/// in `text` and leave out `sha256`: the text is then that object
/// written out.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DesignIn {
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    text: Option<Value>,
}

impl SourceIn {
    pub(crate) fn source(self) -> Result<LibrarySourceDef, String> {
        let given = self
            .design
            .sha256
            .as_deref()
            .map(str::parse::<Sha256>)
            .transpose()?;
        let design = match (self.design.text, given) {
            (Some(Value::String(text)), given) => {
                let design = SourceDesign::new(text);
                if let Some(given) = given
                    && given != design.sha256
                {
                    return Err(format!(
                        "the design's SHA-256 is {}, not {given}",
                        design.sha256
                    ));
                }
                design
            }
            (Some(object @ Value::Object(_)), _) => SourceDesign::new(
                serde_json::to_string_pretty(&object).expect("JSON values serialize"),
            ),
            (Some(_), _) => return Err("the design's text must be a string".to_owned()),
            (None, Some(sha256)) => SourceDesign::reference(sha256),
            (None, None) => return Err("the design needs its text or its sha256".to_owned()),
        };
        Ok(LibrarySourceDef {
            id: self.id,
            library: self.library,
            name: self.name,
            standard: self.standard,
            design,
            build: self.build,
            supplier: self.supplier,
            part_number: self.part_number,
            supplier_url: self.supplier_url,
        })
    }
}
