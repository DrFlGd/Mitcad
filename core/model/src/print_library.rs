// SPDX-License-Identifier: MIT
//! Libraries of materials and print profiles (mitcad#640, part 4).
//!
//! Three libraries complement each other:
//!
//! - **base**: Mitcad's parts library (`mitcad-parts`), whose manifest
//!   lists its materials, printers and processes; read through the
//!   library resolver ([`crate::library::LinkResolver::print_library`])
//!   from its newest version on this computer;
//! - **user**: a folder in Mitcad's data folder, beside `libraries`
//!   ([`set_user_library`], `MITCAD_PRINT_LIBRARY_DIR`);
//! - **project**: the folder `print-library` of the project the design is
//!   in, versioned with it.
//!
//! Each holds one JSON file per item ([`ItemFile`]) in `materials/`,
//! `print/printers/` and `print/processes/`: a material with its
//! appearance inline, or a printer or process profile; a vendor's item
//! `<vendor>/<product>` is a file in a folder of the vendor's. The same
//! kind and id in several libraries: the nearest wins (project, then
//! user, then base); the others are listed as shadowed.
//!
//! A design keeps complete copies of the items it uses
//! (`document/print_library.rs`) with their origin (the library, the id,
//! the version copied), so that a library change never changes a design
//! by itself; a newer version in the library is offered and taken only on
//! request.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::appearance::Appearance;
use crate::material::Material;
use crate::print_profile::{PrintProfile, ProfileKind};

/// The `format` of an item file.
pub const ITEM_FORMAT: &str = "mitcad-print-item";
/// The folders of a library's items.
pub const MATERIALS_DIR: &str = "materials";
pub const PRINTERS_DIR: &str = "print/printers";
pub const PROCESSES_DIR: &str = "print/processes";
/// A project's library folder.
pub const PROJECT_LIBRARY_DIR: &str = "print-library";
/// The largest item file read.
pub const MAX_ITEM: usize = 1 << 20;

/// The base library's id: Mitcad's parts library.
pub const BASE_LIBRARY: &str = "mitcad-parts";

/// Which library an item is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LibraryKind {
    Base,
    User,
    Project,
}

impl LibraryKind {
    pub const ALL: [LibraryKind; 3] = [LibraryKind::Base, LibraryKind::User, LibraryKind::Project];

    pub fn id(self) -> &'static str {
        match self {
            LibraryKind::Base => "base",
            LibraryKind::User => "user",
            LibraryKind::Project => "project",
        }
    }

    pub fn parse(text: &str) -> Option<LibraryKind> {
        LibraryKind::ALL.into_iter().find(|k| k.id() == text)
    }

    /// The place the interface names (mitcad#655): the base library is
    /// Mitcad's, with the built-in materials.
    pub fn place(self) -> &'static str {
        match self {
            LibraryKind::Base => "mitcad",
            LibraryKind::User => "user",
            LibraryKind::Project => "project",
        }
    }

    /// How near it is: the project's first.
    fn rank(self) -> u8 {
        match self {
            LibraryKind::Project => 0,
            LibraryKind::User => 1,
            LibraryKind::Base => 2,
        }
    }
}

/// What an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    Material,
    Printer,
    Process,
}

impl ItemKind {
    pub const ALL: [ItemKind; 3] = [ItemKind::Material, ItemKind::Printer, ItemKind::Process];

    pub fn id(self) -> &'static str {
        match self {
            ItemKind::Material => "material",
            ItemKind::Printer => "printer",
            ItemKind::Process => "process",
        }
    }

    pub fn parse(text: &str) -> Option<ItemKind> {
        ItemKind::ALL.into_iter().find(|k| k.id() == text)
    }

    /// The folder its files are in.
    pub fn dir(self) -> &'static str {
        match self {
            ItemKind::Material => MATERIALS_DIR,
            ItemKind::Printer => PRINTERS_DIR,
            ItemKind::Process => PROCESSES_DIR,
        }
    }

    pub fn of_profile(kind: ProfileKind) -> ItemKind {
        match kind {
            ProfileKind::Printer => ItemKind::Printer,
            ProfileKind::Process => ItemKind::Process,
        }
    }

    /// The kind of the folder a path is in.
    fn of_path(path: &str) -> Option<ItemKind> {
        ItemKind::ALL
            .into_iter()
            .find(|k| path.starts_with(&format!("{}/", k.dir())))
    }
}

/// An item of a library by its library and id, as commands name one
/// (`{"library": "user", "id": "acme/petg"}`, mitcad#655).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemRef {
    /// `base`, `user` or `project`.
    pub library: String,
    pub id: String,
}

/// What `create_material` and `create_print_profile` start from: an item
/// of the design (a material may also be a built-in one) by id, or a
/// library's item (Duplicate, mitcad#655).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum BasedOn {
    Design(String),
    Library(ItemRef),
}

impl BasedOn {
    /// The id of the item it names.
    pub fn id(&self) -> &str {
        match self {
            BasedOn::Design(id) => id,
            BasedOn::Library(item) => &item.id,
        }
    }
}

/// An item file: a material (with its appearance when it is not one of
/// the library's appearances) or a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemFile {
    pub format: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<Material>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<PrintProfile>,
}

/// What an item holds.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ItemData {
    Material {
        material: Material,
        /// Its appearance when the item carries it inline.
        appearance: Option<Appearance>,
    },
    Profile(PrintProfile),
}

impl ItemData {
    pub fn kind(&self) -> ItemKind {
        match self {
            ItemData::Material { .. } => ItemKind::Material,
            ItemData::Profile(p) => ItemKind::of_profile(p.kind),
        }
    }

    pub fn id(&self) -> &str {
        match self {
            ItemData::Material { material, .. } => &material.id,
            ItemData::Profile(p) => &p.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ItemData::Material { material, .. } => &material.name,
            ItemData::Profile(p) => &p.name,
        }
    }

    /// Its version in its library (0 when it names none).
    pub fn version(&self) -> u32 {
        match self {
            ItemData::Material { material, .. } => material.version,
            ItemData::Profile(p) => p.version,
        }
        .unwrap_or(0)
    }

    /// Sets its version in its library.
    pub fn set_version(&mut self, version: u32) {
        match self {
            ItemData::Material { material, .. } => material.version = Some(version),
            ItemData::Profile(p) => p.version = Some(version),
        }
    }

    /// The item as its file has it.
    pub fn file(&self) -> ItemFile {
        let (material, appearance, profile) = match self {
            ItemData::Material {
                material,
                appearance,
            } => (Some(material.clone()), appearance.clone(), None),
            ItemData::Profile(p) => (None, None, Some(p.clone())),
        };
        ItemFile {
            format: ITEM_FORMAT.to_owned(),
            version: 1,
            material,
            appearance,
            profile,
        }
    }

    /// The file's text.
    pub fn text(&self) -> String {
        let mut text = serde_json::to_string_pretty(&self.file()).expect("library items serialize");
        text.push('\n');
        text
    }
}

/// An item of a library.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryItem {
    pub library: LibraryKind,
    /// Its file in the library (`materials/pla.json`).
    pub path: String,
    pub data: ItemData,
    /// A nearer library has the same kind and id.
    pub shadowed: bool,
}

/// Where an item of a kind and id is kept in a library: `materials/pla.json`,
/// `materials/acme/petg-hf.json`.
pub fn item_path(kind: ItemKind, id: &str) -> String {
    format!("{}/{id}.json", kind.dir())
}

/// Reads an item file: the format, one material or profile, valid, of the
/// kind of its folder, its id the file's name.
pub fn parse_item(path: &str, data: &[u8]) -> Result<ItemData, String> {
    if data.len() > MAX_ITEM {
        return Err(format!("{path} is larger than an item may be"));
    }
    let file: ItemFile =
        serde_json::from_slice(data).map_err(|e| format!("{path}: not an item file: {e}"))?;
    if file.format != ITEM_FORMAT {
        return Err(format!(
            "{path}: the format is '{}', not '{ITEM_FORMAT}'",
            file.format
        ));
    }
    if file.version != 1 {
        return Err(format!(
            "{path}: version {} of the item format is not one this Mitcad reads (1)",
            file.version
        ));
    }
    let item = match (file.material, file.profile) {
        (Some(material), None) => {
            material.check().map_err(|e| format!("{path}: {e}"))?;
            if let Some(appearance) = &file.appearance {
                appearance.check().map_err(|e| format!("{path}: {e}"))?;
                if material.appearance.as_deref() != Some(appearance.id.as_str()) {
                    return Err(format!(
                        "{path}: the appearance {} is not the material's",
                        appearance.id
                    ));
                }
            }
            ItemData::Material {
                material,
                appearance: file.appearance,
            }
        }
        (None, Some(profile)) => {
            if file.appearance.is_some() {
                return Err(format!("{path}: a profile has no appearance"));
            }
            profile.check().map_err(|e| format!("{path}: {e}"))?;
            ItemData::Profile(profile)
        }
        _ => {
            return Err(format!("{path}: an item is one material or one profile"));
        }
    };
    if let Some(kind) = ItemKind::of_path(path)
        && kind != item.kind()
    {
        return Err(format!(
            "{path}: a {} in the folder of {}s",
            item.kind().id(),
            kind.id()
        ));
    }
    if path != item_path(item.kind(), item.id()) {
        return Err(format!(
            "{path}: the {} {} belongs in {}",
            item.kind().id(),
            item.id(),
            item_path(item.kind(), item.id())
        ));
    }
    Ok(item)
}

/// A library folder's item files: the `.json` files under its item
/// folders, by their paths in it (with `/`). Links are not followed and
/// files larger than an item may be are left out.
pub fn folder_files(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let path = format!("{prefix}/{name}");
            if kind.is_dir() && depth < 4 {
                walk(&entry.path(), &path, depth + 1, out);
            } else if kind.is_file() && name.ends_with(".json") {
                let small = entry.metadata().is_ok_and(|m| m.len() <= MAX_ITEM as u64);
                if small && let Ok(data) = std::fs::read(entry.path()) {
                    out.push((path, data));
                }
            }
        }
    }
    let mut out = Vec::new();
    for kind in ItemKind::ALL {
        walk(&root.join(kind.dir()), kind.dir(), 0, &mut out);
    }
    out
}

static USER_LIBRARY: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Sets the user's library folder (the application and the command line
/// set it beside the fetched libraries).
pub fn set_user_library(folder: Option<PathBuf>) {
    *USER_LIBRARY.write().unwrap_or_else(|e| e.into_inner()) = folder;
}

/// The user's library folder: the one set, else `MITCAD_PRINT_LIBRARY_DIR`.
pub fn user_library() -> Option<PathBuf> {
    USER_LIBRARY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .or_else(|| {
            std::env::var_os("MITCAD_PRINT_LIBRARY_DIR")
                .filter(|d| !d.is_empty())
                .map(PathBuf::from)
        })
}

/// A project's library folder.
pub fn project_library(project: &Path) -> PathBuf {
    project.join(PROJECT_LIBRARY_DIR)
}

/// The libraries' items, and what could not be read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Libraries {
    pub items: Vec<LibraryItem>,
    pub problems: Vec<String>,
    /// The folders read (user, project).
    pub folders: Vec<(LibraryKind, PathBuf)>,
}

impl Libraries {
    /// Reads the base library's files, the user's folder and the project's
    /// folder; marks the items a nearer library has too.
    pub fn load(
        base: Vec<(String, Vec<u8>)>,
        user: Option<&Path>,
        project: Option<&Path>,
    ) -> Libraries {
        let mut out = Libraries::default();
        let add = |library: LibraryKind, files: Vec<(String, Vec<u8>)>, out: &mut Libraries| {
            for (path, data) in files {
                match parse_item(&path, &data) {
                    Ok(data) => out.items.push(LibraryItem {
                        library,
                        path,
                        data,
                        shadowed: false,
                    }),
                    Err(problem) => out.problems.push(format!("{}: {problem}", library.id())),
                }
            }
        };
        add(LibraryKind::Base, base, &mut out);
        if let Some(user) = user {
            out.folders.push((LibraryKind::User, user.to_path_buf()));
            add(LibraryKind::User, folder_files(user), &mut out);
        }
        if let Some(project) = project {
            let folder = project_library(project);
            out.folders.push((LibraryKind::Project, folder.clone()));
            add(LibraryKind::Project, folder_files(&folder), &mut out);
        }
        let ranks: Vec<(ItemKind, String, u8)> = out
            .items
            .iter()
            .map(|i| (i.data.kind(), i.data.id().to_owned(), i.library.rank()))
            .collect();
        for item in &mut out.items {
            let rank = item.library.rank();
            item.shadowed = ranks.iter().any(|(kind, id, other)| {
                *kind == item.data.kind() && id == item.data.id() && *other < rank
            });
        }
        out
    }

    /// An item of a kind and id: in the library given, else the nearest.
    pub fn find(
        &self,
        library: Option<LibraryKind>,
        kind: ItemKind,
        id: &str,
    ) -> Option<&LibraryItem> {
        self.items
            .iter()
            .filter(|i| i.data.kind() == kind && i.data.id() == id)
            .filter(|i| library.is_none_or(|l| i.library == l))
            .min_by_key(|i| i.library.rank())
    }

    /// The folder of a library that is a folder (user, project).
    pub fn folder(&self, library: LibraryKind) -> Option<&Path> {
        self.folders
            .iter()
            .find(|(l, _)| *l == library)
            .map(|(_, f)| f.as_path())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn pla(version: u32) -> ItemData {
        ItemData::Material {
            material: Material {
                id: "pla".to_owned(),
                name: "PLA".to_owned(),
                density: 1.24,
                appearance: Some("plastic_white".to_owned()),
                filament: BTreeMap::from([("filament_type".to_owned(), serde_json::json!("PLA"))]),
                slicer: BTreeMap::new(),
                based_on: None,
                class: None,
                version: Some(version),
                origin: None,
            },
            appearance: None,
        }
    }

    #[test]
    fn items_read_back_and_are_checked() {
        let item = pla(2);
        let path = item_path(ItemKind::Material, "pla");
        assert_eq!(path, "materials/pla.json");
        assert_eq!(parse_item(&path, item.text().as_bytes()).unwrap(), item);
        // In the wrong folder, under another name, of another format.
        let error = parse_item("print/printers/pla.json", item.text().as_bytes()).unwrap_err();
        assert!(error.contains("folder of printers"), "{error}");
        let error = parse_item("materials/abs.json", item.text().as_bytes()).unwrap_err();
        assert!(error.contains("belongs in materials/pla.json"), "{error}");
        let error =
            parse_item("materials/pla.json", b"{\"format\": \"x\", \"version\": 1}").unwrap_err();
        assert!(error.contains("the format is 'x'"), "{error}");
        let profile = ItemData::Profile(PrintProfile::new("draft", "Draft", ProfileKind::Process));
        assert_eq!(
            parse_item("print/processes/draft.json", profile.text().as_bytes()).unwrap(),
            profile
        );
    }

    #[test]
    fn the_nearest_library_wins() {
        let dir = std::env::temp_dir().join(format!("mitcad-print-library-{}", std::process::id()));
        let user = dir.join("user");
        let project = dir.join("project");
        for (root, version) in [(&user, 3), (&project_library(&project), 4)] {
            std::fs::create_dir_all(root.join(MATERIALS_DIR)).unwrap();
            std::fs::write(root.join("materials/pla.json"), pla(version).text()).unwrap();
        }
        std::fs::write(user.join("materials/broken.json"), b"{").unwrap();
        let base = vec![("materials/pla.json".to_owned(), pla(1).text().into_bytes())];
        let libraries = Libraries::load(base, Some(&user), Some(&project));
        assert_eq!(libraries.items.len(), 3);
        assert_eq!(libraries.problems.len(), 1, "{:?}", libraries.problems);
        let nearest = libraries.find(None, ItemKind::Material, "pla").unwrap();
        assert_eq!(nearest.library, LibraryKind::Project);
        assert_eq!(nearest.data.version(), 4);
        assert!(!nearest.shadowed);
        let base = libraries
            .find(Some(LibraryKind::Base), ItemKind::Material, "pla")
            .unwrap();
        assert!(base.shadowed);
        assert_eq!(base.data.version(), 1);
        assert_eq!(libraries.folder(LibraryKind::User), Some(user.as_path()));
    }
}
