// SPDX-License-Identifier: MIT
//! Commands and queries of print profiles and print settings (mitcad#640,
//! `document/print_settings.rs`): the design's printer and process
//! profiles, what a body, occurrence or component prints with, and the
//! design printer's build volume. See `commands.md`, "Print profiles".

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use super::dispatch::named_enum;
use crate::document::{Document, PrintTarget, writable_library};
use crate::ids::ComponentUid;
use crate::kernel::Kernel;
use crate::print_keys::Dialect;
use crate::print_library::{ItemKind, LibraryKind};
use crate::print_profile::{ProfileChange, ProfileKind};
use crate::properties::PropertyOwner;

named_enum! {
    #[derive(Debug, Deserialize)]
    #[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
    pub(crate) enum PrintProfileCommand {
        "create_print_profile" => CreatePrintProfile(ProfileChange),
        "edit_print_profile" => EditPrintProfile(ProfileChange),
        "delete_print_profile" => DeletePrintProfile {
            id: String,
            /// `user` or `project`: the item is deleted from that library
            /// (mitcad#655).
            #[serde(default)]
            library: Option<String>,
            /// `printer` or `process`, when the library has both of the id.
            #[serde(default)]
            kind: Option<String>,
            #[serde(default)]
            project: Option<String>,
        },
        // The libraries (part 4).
        "copy_from_print_library" => CopyFromPrintLibrary {
            /// `base`, `user` or `project`; the nearest when left out.
            #[serde(default)]
            library: Option<String>,
            /// `material`, `printer` or `process`.
            kind: String,
            id: String,
            /// The project's folder, when the design is in one.
            #[serde(default)]
            project: Option<String>,
        },
        "update_from_print_library" => UpdateFromPrintLibrary {
            kind: String,
            /// The design's copy.
            id: String,
            #[serde(default)]
            project: Option<String>,
        },
        /// Keeps the design's copy over its library's newer version
        /// (mitcad#655).
        "decline_print_library_update" => DeclinePrintLibraryUpdate {
            kind: String,
            /// The design's copy.
            id: String,
            #[serde(default)]
            project: Option<String>,
        },
        "save_to_print_library" => SaveToPrintLibrary {
            /// `user` or `project`.
            library: String,
            kind: String,
            id: String,
            #[serde(default)]
            project: Option<String>,
        },
        // Slicers' files (part 5).
        "import_print_profile" => ImportPrintProfile {
            /// A 3MF project, a preset or bundle, a PrusaSlicer .ini.
            path: String,
            /// The Bambu family's member when the file does not say.
            #[serde(default)]
            dialect: Option<String>,
            /// `design` (the default), `user` or `project`.
            #[serde(default)]
            into: Option<String>,
            #[serde(default)]
            project: Option<String>,
            /// Into the design: the root chooses the printer and process
            /// read (the default).
            #[serde(default)]
            choose: Option<bool>,
        },
    }
}

/// The `print_library` query.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrintLibraryQuery {
    /// The project's folder, when the design is in one.
    #[serde(default)]
    project: Option<String>,
}

/// The `library_items` query (mitcad#655).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LibraryItemsQuery {
    /// `material`, `printer` or `process`.
    kind: String,
    /// The project's folder, when the design is in one.
    #[serde(default)]
    project: Option<String>,
}

fn item_kind(text: &str) -> Result<ItemKind, ApiError> {
    ItemKind::parse(text).ok_or_else(|| {
        ApiError(format!(
            "'{text}' is no kind of library item: material, printer or process"
        ))
    })
}

fn library_kind(text: &str) -> Result<LibraryKind, ApiError> {
    LibraryKind::parse(text)
        .ok_or_else(|| ApiError(format!("'{text}' is no library: base, user or project")))
}

/// The `print_settings` query: a body (in an occurrence path), an
/// occurrence, a component, or nothing (the design's own).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrintSettingsQuery {
    /// A body by uid or name.
    #[serde(default)]
    body: Option<String>,
    /// The occurrence path (`O1/O4`, or names): of the body, or the
    /// occurrence asked for.
    #[serde(default)]
    occurrence: Option<String>,
    /// A component by uid (`C0` the root) or name.
    #[serde(default)]
    component: Option<String>,
}

impl<K: Kernel> Document<K> {
    pub(crate) fn run_print_profile_command(
        &mut self,
        command: PrintProfileCommand,
    ) -> Result<Value, ApiError> {
        Ok(match command {
            PrintProfileCommand::CreatePrintProfile(change) => match change.library.as_deref() {
                Some(library) => {
                    let library = writable_library(library)?;
                    let libraries = self.print_libraries(change.project.as_deref().map(Path::new));
                    let (id, path, version) =
                        self.create_library_profile(&libraries, library, &change)?;
                    json!({"id": id, "library": library.id(), "version": version,
                           "path": path.to_string_lossy()})
                }
                None => json!({"id": self.create_print_profile(&change)?}),
            },
            PrintProfileCommand::EditPrintProfile(change) => {
                let id = change.id.clone().ok_or_else(|| {
                    ApiError("edit_print_profile needs the profile's id".to_owned())
                })?;
                match change.library.as_deref() {
                    Some(library) => {
                        let library = writable_library(library)?;
                        let libraries =
                            self.print_libraries(change.project.as_deref().map(Path::new));
                        let (path, version) =
                            self.edit_library_profile(&libraries, library, &id, &change)?;
                        json!({"version": version, "path": path.to_string_lossy()})
                    }
                    None => {
                        self.edit_print_profile(&id, &change)?;
                        json!({})
                    }
                }
            }
            PrintProfileCommand::DeletePrintProfile {
                id,
                library,
                kind,
                project,
            } => match library.as_deref() {
                Some(library) => {
                    let library = writable_library(library)?;
                    let kind = kind
                        .as_deref()
                        .map(|text| {
                            ProfileKind::parse(text).ok_or_else(|| {
                                ApiError(format!(
                                    "'{text}' is no kind of profile: printer or process"
                                ))
                            })
                        })
                        .transpose()?;
                    let libraries = self.print_libraries(project.as_deref().map(Path::new));
                    let (path, kept, removed) =
                        self.delete_library_profile(&libraries, library, kind, &id)?;
                    json!({"path": path.to_string_lossy(), "kept": kept, "removed": removed})
                }
                None => json!({"owners": self.delete_print_profile(&id)?}),
            },
            PrintProfileCommand::CopyFromPrintLibrary {
                library,
                kind,
                id,
                project,
            } => {
                let library = library.as_deref().map(library_kind).transpose()?;
                let kind = item_kind(&kind)?;
                let libraries = self.print_libraries(project.as_deref().map(Path::new));
                let item = libraries.find(library, kind, &id).ok_or_else(|| {
                    ApiError(format!(
                        "{} has no {} '{id}'",
                        library.map_or("no library".to_owned(), |l| format!(
                            "the {} library",
                            l.id()
                        )),
                        kind.id()
                    ))
                })?;
                let copied = self.copy_from_print_library(item)?;
                json!({"id": copied, "library": item.library.id(), "version": item.data.version()})
            }
            PrintProfileCommand::UpdateFromPrintLibrary { kind, id, project } => {
                let kind = item_kind(&kind)?;
                let libraries = self.print_libraries(project.as_deref().map(Path::new));
                json!({"version": self.update_from_print_library(&libraries, kind, &id)?})
            }
            PrintProfileCommand::DeclinePrintLibraryUpdate { kind, id, project } => {
                let kind = item_kind(&kind)?;
                let libraries = self.print_libraries(project.as_deref().map(Path::new));
                json!({"version": self.decline_print_library_update(&libraries, kind, &id)?})
            }
            PrintProfileCommand::SaveToPrintLibrary {
                library,
                kind,
                id,
                project,
            } => {
                let library = library_kind(&library)?;
                let kind = item_kind(&kind)?;
                let libraries = self.print_libraries(project.as_deref().map(Path::new));
                let (path, version) = self.save_to_print_library(&libraries, library, kind, &id)?;
                json!({"path": path.to_string_lossy(), "version": version})
            }
            PrintProfileCommand::ImportPrintProfile {
                path,
                dialect,
                into,
                project,
                choose,
            } => {
                let dialect = dialect
                    .as_deref()
                    .map(|id| {
                        Dialect::from_id(id).ok_or_else(|| {
                            ApiError(format!(
                                "'{id}' is no slicer: bambu_studio, orcaslicer, qidi_studio, prusaslicer or cura"
                            ))
                        })
                    })
                    .transpose()?;
                let into = match into.as_deref() {
                    None | Some("design") => None,
                    Some(text) => Some(library_kind(text)?),
                };
                self.import_print_settings(
                    &path,
                    dialect,
                    into,
                    project.as_deref().map(Path::new),
                    choose.unwrap_or(true),
                )?
            }
        })
    }

    pub(super) fn print_library_query(&self, query: &PrintLibraryQuery) -> Value {
        let libraries = self.print_libraries(query.project.as_deref().map(Path::new));
        self.print_library_json(&libraries)
    }

    pub(super) fn library_items_query(&self, query: &LibraryItemsQuery) -> Result<Value, ApiError> {
        let kind = item_kind(&query.kind)?;
        let libraries = self.print_libraries(query.project.as_deref().map(Path::new));
        Ok(self.library_items_json(&libraries, kind))
    }

    /// What a `print_settings` query asks for.
    pub(crate) fn print_target(&self, query: &PrintSettingsQuery) -> Result<PrintTarget, ApiError> {
        let path = match &query.occurrence {
            Some(text) => Some(
                self.assembly()
                    .find_path(text)
                    .ok_or_else(|| ApiError(format!("no occurrence path {text}")))?,
            ),
            None => None,
        };
        if let Some(text) = &query.body {
            let body = match self.property_owner_ref(text)? {
                PropertyOwner::Body(b) => b,
                _ => return Err(ApiError(format!("'{text}' is no body"))),
            };
            return Ok(PrintTarget::Body(body, path));
        }
        if let Some(path) = path {
            return Ok(PrintTarget::Occurrence(path));
        }
        Ok(PrintTarget::Component(match &query.component {
            Some(text) => self.component_ref(text)?,
            None => ComponentUid::ROOT,
        }))
    }

    pub(super) fn print_settings_query(
        &self,
        query: &PrintSettingsQuery,
    ) -> Result<Value, ApiError> {
        let target = self.print_target(query)?;
        Ok(self.print_settings_json(&target))
    }
}
