// SPDX-License-Identifier: MIT
//! The documents an `.f3d` design refers to (its components of other
//! documents, mitcad#196), found in local files:
//!
//! - a file given for a document (`reference_files`: the application's
//!   file dialog, by the document's URN);
//! - the reference folder next to the design (`<design> vN.refs/`, or a
//!   folder given with `references`): its `references.json` maps the
//!   documents' version ids (the URNs the design names them by) to their
//!   files, for the whole tree of references;
//! - another design of the same `.f3z` package;
//! - any `.f3d` file of those folders whose `Manifest.dat` lists the GUID
//!   the design keeps after the document's URN (so a file is found without
//!   `references.json`, and a chosen file's folder is searched for the
//!   other documents).
//!
//! The import (`mitcad-ffi`) then imports each found document on its own,
//! with its own references, and fills the components standing for its
//! components ([`crate::references`]). [`list`] lists the documents and
//! where they are without importing anything, for the application to ask
//! for the missing ones (`list_references`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use mitcad_f3d::F3dFile;
use mitcad_f3d::design::{build::urn_version, manifest_guids, referenced_documents};

/// Whether `references.json`'s `format` names a reference folder: the
/// exporter that writes these folders names it `mitcad-<source>-references`.
fn is_reference_format(name: &str) -> bool {
    name.starts_with("mitcad-") && name.ends_with("-references")
}

/// How deep references of references are followed.
pub const MAX_DEPTH: usize = 16;

/// The most `.f3d` files of the folders whose manifests are read to find a
/// document by its GUID, and the largest such file (bytes).
const MAX_SCANNED: usize = 500;
const MAX_SCANNED_SIZE: u64 = 512 << 20;

/// A node of `references.json`'s tree (unknown keys are left alone).
#[derive(Debug, Deserialize)]
struct Node {
    name: Option<String>,
    #[serde(rename = "versionId")]
    version_id: Option<String>,
    version: Option<u32>,
    file: Option<String>,
    #[serde(default)]
    children: Vec<Node>,
}

#[derive(Debug, Deserialize)]
struct FolderFile {
    format: Option<String>,
    design: Option<Node>,
}

/// A document's entry of a reference folder.
#[derive(Debug, Clone)]
struct Entry {
    name: Option<String>,
    version: Option<u32>,
    /// Its file, where the folder has it.
    file: Option<PathBuf>,
}

/// Where a document's file was found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    /// The file (None: not found).
    pub file: Option<PathBuf>,
    /// The design of an `.f3z` package to import.
    pub design: Option<String>,
    /// How it was found.
    pub how: Option<&'static str>,
    /// Its name and version, where a reference folder gives them.
    pub name: Option<String>,
    pub version: Option<u32>,
    /// Where the file is of another version of the document than the
    /// design uses.
    pub note: Option<String>,
}

/// A design of an `.f3z` package: its label and its manifest's GUIDs.
type PackageDesign = (String, Vec<String>);

/// Finds the documents of one import (shared by the imports of the
/// documents it refers to).
#[derive(Debug, Default)]
pub struct Resolver {
    folders: Vec<PathBuf>,
    files: HashMap<String, PathBuf>,
    entries: HashMap<String, Entry>,
    /// The scanned files' manifest GUIDs.
    scanned: HashMap<PathBuf, Vec<String>>,
    /// An `.f3z` package's designs with their manifest GUIDs.
    package: Option<(PathBuf, Vec<PackageDesign>)>,
    warnings: Vec<String>,
}

impl Resolver {
    /// The reference folder next to `path` (`<design> vN.refs`), then
    /// `folders`, and the files given by URN.
    pub fn new(path: &Path, folders: &[String], files: &HashMap<String, String>) -> Self {
        let mut r = Resolver::default();
        for f in folders {
            r.add_folder(Path::new(f));
        }
        let next = path.with_extension("refs");
        if next.is_dir() {
            r.add_folder(&next);
        }
        for (urn, file) in files {
            r.files.insert(urn.clone(), PathBuf::from(file));
            if let Some(folder) = Path::new(file).parent() {
                r.add_folder(folder);
            }
        }
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("f3z"))
            && let Ok(documents) = mitcad_f3d::design::documents(path)
        {
            let designs = documents
                .iter()
                .map(|(label, doc)| (label.clone(), manifest_guids(doc)))
                .collect();
            r.package = Some((path.to_path_buf(), designs));
        }
        r
    }

    /// Searches `folder` too (its `references.json` and its `.f3d` files).
    pub fn add_folder(&mut self, folder: &Path) {
        let folder = folder.to_path_buf();
        if self.folders.contains(&folder) || !folder.is_dir() {
            return;
        }
        let list = folder.join("references.json");
        if list.is_file() {
            match std::fs::read_to_string(&list)
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str::<FolderFile>(&t).map_err(|e| e.to_string()))
            {
                Ok(f) if f.format.as_deref().is_none_or(is_reference_format) => {
                    if let Some(design) = &f.design {
                        self.read_node(&folder, design);
                    }
                }
                Ok(f) => self.warnings.push(format!(
                    "{}: unknown format {}",
                    list.display(),
                    f.format.unwrap_or_default()
                )),
                Err(e) => self.warnings.push(format!("{}: {e}", list.display())),
            }
        }
        self.folders.push(folder);
    }

    fn read_node(&mut self, folder: &Path, node: &Node) {
        if let Some(id) = &node.version_id {
            let file = node
                .file
                .as_ref()
                .map(|f| folder.join(f))
                .filter(|f| f.is_file());
            let entry = self.entries.entry(id.clone()).or_insert(Entry {
                name: None,
                version: None,
                file: None,
            });
            entry.name = entry.name.take().or_else(|| node.name.clone());
            entry.version = entry.version.or(node.version);
            entry.file = entry.file.take().or(file);
        }
        for child in &node.children {
            self.read_node(folder, child);
        }
    }

    /// The folders searched, as given to the imports of the documents.
    pub fn folders(&self) -> Vec<String> {
        self.folders
            .iter()
            .map(|f| f.to_string_lossy().into_owned())
            .collect()
    }

    /// The files given by URN.
    pub fn files(&self) -> HashMap<String, String> {
        self.files
            .iter()
            .map(|(u, f)| (u.clone(), f.to_string_lossy().into_owned()))
            .collect()
    }

    /// Problems with the reference folders.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Where the document `urn` (whose file's manifest lists
    /// `document_guid`) is.
    pub fn resolve(&mut self, urn: &str, document_guid: Option<&str>) -> Found {
        let entry = self.entries.get(urn).cloned();
        let mut found = Found {
            name: entry.as_ref().and_then(|e| e.name.clone()),
            version: entry
                .as_ref()
                .and_then(|e| e.version)
                .or_else(|| urn_version(urn)),
            ..Found::default()
        };
        if let Some(file) = self.files.get(urn).filter(|f| f.is_file()) {
            found.file = Some(file.clone());
            found.how = Some("chosen");
            return found;
        }
        if let Some(file) = entry.and_then(|e| e.file) {
            found.file = Some(file);
            found.how = Some("reference folder");
            return found;
        }
        // The same document at another version (a folder made for another
        // version of the design).
        let base = urn.split('?').next().unwrap_or(urn);
        let mut others: Vec<(&String, &Entry)> = self
            .entries
            .iter()
            .filter(|(id, e)| id.split('?').next() == Some(base) && e.file.is_some())
            .collect();
        others.sort_by_key(|(_, e)| e.version);
        if let Some((_, other)) = others.last() {
            found.name = found.name.take().or_else(|| other.name.clone());
            found.note = other_version(other.version, found.version);
            found.file = other.file.clone();
            found.how = Some("reference folder");
            return found;
        }
        let Some(guid) = document_guid
            .map(str::to_lowercase)
            .filter(|g| !g.is_empty())
        else {
            return found;
        };
        if let Some((path, designs)) = &self.package
            && let Some((label, _)) = designs.iter().find(|(_, g)| g.contains(&guid))
        {
            found.file = Some(path.clone());
            found.design = Some(label.clone());
            found.how = Some("package");
            return found;
        }
        if let Some(file) = self.by_guid(&guid, found.version) {
            let named = stem_version(&file.to_string_lossy());
            found.note = named.and_then(|v| other_version(Some(v), found.version));
            found.file = Some(file);
            found.how = Some("by its GUID");
        }
        found
    }

    /// The `.f3d` file of the folders whose manifest lists `guid`; files
    /// named for `version` (`… v<version>.f3d`) first.
    fn by_guid(&mut self, guid: &str, version: Option<u32>) -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        for folder in &self.folders {
            let Ok(dir) = std::fs::read_dir(folder) else {
                continue;
            };
            for e in dir.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("f3d"))
                    && e.metadata()
                        .is_ok_and(|m| m.is_file() && m.len() <= MAX_SCANNED_SIZE)
                {
                    candidates.push(p);
                }
            }
        }
        let suffix = version.map(|v| format!(" v{v}"));
        candidates.sort_by_key(|p| {
            let named = suffix.as_ref().is_some_and(|s| {
                p.file_stem()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(s.as_str()))
            });
            (!named, p.clone())
        });
        candidates.dedup();
        for p in candidates.into_iter().take(MAX_SCANNED) {
            if !self.scanned.contains_key(&p) {
                let guids = F3dFile::open(&p)
                    .map(|f| manifest_guids(&f))
                    .unwrap_or_default();
                self.scanned.insert(p.clone(), guids);
            }
            if self.scanned[&p].iter().any(|g| g == guid) {
                return Some(p);
            }
        }
        None
    }
}

/// The documents a design refers to and where their files are, those
/// found followed into their own references (`list_references`; the
/// application asks for the missing ones): `{"documents": [{"document",
/// "name", "version", "occurrences", "file", "found", "parent",
/// "depth"}], "folders": [...]}`.
pub fn list(
    path: &str,
    folders: &[String],
    files: &HashMap<String, String>,
) -> Result<Value, String> {
    let mut resolver = Resolver::new(Path::new(path), folders, files);
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    list_file(
        &mut resolver,
        Path::new(path),
        None,
        None,
        0,
        &mut seen,
        &mut out,
    )?;
    Ok(json!({
        "documents": out,
        "folders": resolver.folders(),
        "warnings": resolver.warnings(),
    }))
}

#[allow(clippy::too_many_arguments)]
fn list_file(
    resolver: &mut Resolver,
    path: &Path,
    design: Option<&str>,
    parent: Option<&str>,
    depth: usize,
    seen: &mut Vec<String>,
    out: &mut Vec<Value>,
) -> Result<(), String> {
    let documents = mitcad_f3d::design::documents(path)?;
    let doc = match design {
        Some(label) => documents.iter().find(|(l, _)| l == label),
        None => documents.first(),
    };
    let Some((_, doc)) = doc else {
        return Ok(());
    };
    for r in referenced_documents(doc)? {
        if r.occurrences == 0 || seen.contains(&r.urn) {
            continue;
        }
        seen.push(r.urn.clone());
        let found = resolver.resolve(&r.urn, Some(&r.document_guid));
        out.push(json!({
            "document": r.urn,
            "name": found.name.clone().or_else(|| found.file.as_ref().map(|f| stem_name(&f.to_string_lossy()))),
            "note": found.note,
            "version": found.version.or(r.version),
            "occurrences": r.occurrences,
            "file": found.file.as_ref().map(|f| f.to_string_lossy().into_owned()),
            "design": found.design,
            "found": found.how,
            "parent": parent,
            "depth": depth,
        }));
        if depth + 1 < MAX_DEPTH
            && let Some(file) = &found.file
        {
            // A file that cannot be read is reported by the import.
            let _ = list_file(
                resolver,
                file,
                found.design.as_deref(),
                Some(&r.urn),
                depth + 1,
                seen,
                out,
            );
        }
    }
    Ok(())
}

/// The version a file's name gives: `Part v2.f3d` is version 2.
fn stem_version(file: &str) -> Option<u32> {
    let stem = Path::new(file).file_stem()?.to_string_lossy().into_owned();
    stem.rsplit_once(" v")?.1.parse().ok()
}

/// The note of a file of version `file` of a document of which the
/// design uses version `wanted`.
fn other_version(file: Option<u32>, wanted: Option<u32>) -> Option<String> {
    match (file, wanted) {
        (Some(f), Some(w)) if f != w => Some(format!(
            "the file is version {f} of the document; the design uses version {w}"
        )),
        _ => None,
    }
}

/// A document's name from its file's: `Part v2.f3d` is `Part`.
pub fn stem_name(file: &str) -> String {
    let stem = Path::new(file)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match stem.rsplit_once(" v") {
        Some((name, v)) if !name.is_empty() && v.chars().all(|c| c.is_ascii_digit()) => {
            name.to_owned()
        }
        _ => stem,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mitcad_f3d::testreferences::{self as t, ASSEMBLY, HOLDER, PART};

    /// The test assembly and its reference folder in a new folder.
    fn write_files(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mitcad-references-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, data) in t::files() {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, data).unwrap();
        }
        dir
    }

    #[test]
    fn documents_are_found_in_the_reference_folder_and_by_their_guids() {
        let dir = write_files("found");
        let assembly = dir.join(t::file_name(&ASSEMBLY));
        let listed = list(&assembly.to_string_lossy(), &[], &HashMap::new()).unwrap();
        let docs = listed["documents"].as_array().unwrap();
        // The part and the holder, then the holder's part (listed once).
        assert_eq!(docs.len(), 2, "{listed}");
        assert_eq!(docs[0]["document"], PART.urn);
        assert_eq!(docs[0]["name"], "Part");
        assert_eq!(docs[0]["version"], 2);
        assert_eq!(docs[0]["found"], "reference folder");
        assert_eq!(docs[1]["document"], HOLDER.urn);
        assert_eq!(docs[1]["occurrences"], 2);

        // Without references.json the manifests' GUIDs tell the files.
        let refs = dir.join("Assembly v1.refs");
        std::fs::remove_file(refs.join("references.json")).unwrap();
        let mut resolver = Resolver::new(&assembly, &[], &HashMap::new());
        let found = resolver.resolve(HOLDER.urn, Some(HOLDER.document_guid));
        assert_eq!(found.how, Some("by its GUID"));
        assert_eq!(found.file, Some(refs.join(t::file_name(&HOLDER))));
        assert_eq!(found.name, None);
        assert_eq!(found.version, Some(1));

        // A folder elsewhere: nothing found until a file of it is chosen.
        let elsewhere = dir.join("elsewhere");
        std::fs::rename(&refs, &elsewhere).unwrap();
        let mut resolver = Resolver::new(&assembly, &[], &HashMap::new());
        assert_eq!(
            resolver.resolve(PART.urn, Some(PART.document_guid)).file,
            None
        );
        let chosen = HashMap::from([(
            HOLDER.urn.to_owned(),
            elsewhere
                .join(t::file_name(&HOLDER))
                .to_string_lossy()
                .into_owned(),
        )]);
        let mut resolver = Resolver::new(&assembly, &[], &chosen);
        assert_eq!(resolver.resolve(HOLDER.urn, None).how, Some("chosen"));
        let part = resolver.resolve(PART.urn, Some(PART.document_guid));
        assert_eq!(part.file, Some(elsewhere.join(t::file_name(&PART))));
        // A GUID no file has.
        assert_eq!(resolver.resolve("urn:x?version=1", Some("0000")).file, None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
