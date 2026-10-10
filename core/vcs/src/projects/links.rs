// SPDX-License-Identifier: MIT
//! Preserve component source identities when project files move (mitcad#168).
//! Work on file JSON so B-rep references and unknown optional fields survive.

use std::fs;
use std::path::{Path, PathBuf};

use mitcad_model::file::brep_references;
use serde_json::Value;

use crate::remote::{ErrorClass, RemoteError};
use crate::{ProjectRepo, VcsError, absolute, io_error, is_design_file};

pub(super) struct LinkRewrite {
    pub before: String,
    pub after: String,
    pub bytes: Vec<u8>,
}

fn refusal(message: impl Into<String>) -> VcsError {
    VcsError::Remote(RemoteError::new(ErrorClass::LocalChanges, message))
}

/// Resolve an existing ancestor too: a modified document's file may
/// already be missing, while its project folder is opened through an alias.
fn physical_path(path: &Path) -> Result<PathBuf, VcsError> {
    let path = absolute(path)?;
    for ancestor in path.ancestors() {
        if let Ok(canonical) = fs::canonicalize(ancestor) {
            return Ok(canonical.join(path.strip_prefix(ancestor).expect("ancestor")));
        }
    }
    Ok(path)
}

/// Lexical first (also works for a missing source); canonical paths also
/// recognise existing sources spelled with different case or an alias.
fn moved_path(path: &Path, from: &Path, to: &Path) -> Option<PathBuf> {
    if let Ok(tail) = path.strip_prefix(from) {
        return Some(to.join(tail));
    }
    let (Ok(path), Ok(from)) = (fs::canonicalize(path), fs::canonicalize(from)) else {
        return None;
    };
    path.strip_prefix(from).ok().map(|tail| to.join(tail))
}

fn relative_path(target: &Path, directory: &Path) -> PathBuf {
    let target: Vec<_> = target.components().collect();
    let directory: Vec<_> = directory.components().collect();
    let shared = target
        .iter()
        .zip(&directory)
        .take_while(|(a, b)| a == b)
        .count();
    // Different Windows drives: retain an absolute path.
    if shared == 0 {
        return target.iter().collect();
    }
    let mut out = PathBuf::new();
    for _ in &directory[shared..] {
        out.push("..");
    }
    for part in &target[shared..] {
        out.push(part);
    }
    out
}

fn path_text(path: &Path) -> Result<String, VcsError> {
    path.to_str()
        .map(|text| text.replace(std::path::MAIN_SEPARATOR, "/"))
        .ok_or_else(|| refusal(format!("{} is not a Unicode link path", path.display())))
}

/// Enumerate every visible working design, including unversioned ones.
/// An incomplete scan cannot establish that a move preserves its links.
fn designs_in(root: &Path) -> Result<Vec<PathBuf>, VcsError> {
    let mut designs = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).map_err(|e| io_error("read", &folder, e))? {
            let entry = entry.map_err(|e| io_error("read", &folder, e))?;
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            let kind = entry.file_type().map_err(|e| io_error("read", &path, e))?;
            if kind.is_symlink() {
                return Err(refusal(format!(
                    "cannot check component links through {}; move or remove this link first",
                    path.display()
                )));
            }
            if kind.is_dir() {
                folders.push(path);
            } else if is_design_file(&path.to_string_lossy()) {
                if !kind.is_file() {
                    return Err(refusal(format!(
                        "{} is not a regular design file",
                        path.display()
                    )));
                }
                designs.push(path);
            }
        }
    }
    designs.sort();
    Ok(designs)
}

impl ProjectRepo {
    /// Prepare all link edits before any filesystem mutation. Open owners
    /// are refused: their undo states or unsaved commands could put an old
    /// path back on disk. The application supplies every window's paths.
    pub(super) fn move_link_rewrites(
        &self,
        from: &str,
        to: &str,
        command: &Value,
    ) -> Result<Vec<LinkRewrite>, VcsError> {
        let root = self.root();
        let physical_root = physical_path(root)?;
        for path in command
            .get("modified_designs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(path) = path.as_str()
                && physical_path(Path::new(path))?.starts_with(&physical_root)
            {
                return Err(refusal(format!(
                    "{path} has unsaved changes; save it before moving project files so its component links can be checked"
                )));
            }
        }
        let from = root.join(from);
        let to = root.join(to);
        let mut rewrites = Vec::new();
        for path in designs_in(root)? {
            let before = self.relative(&path)?;
            let destination = moved_path(&path, &from, &to).unwrap_or_else(|| path.clone());
            let parent = path.parent().expect("design inside project");
            let next_parent = destination.parent().expect("destination inside project");
            let text = fs::read_to_string(&path).map_err(|e| io_error("read", &path, e))?;
            brep_references(&text).map_err(|e| refusal(format!(
                "cannot check component links in {before}: {e}; repair this design before moving project files"
            )))?;
            let mut design: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
                .map_err(|e| refusal(format!("cannot read {before}: {e}")))?;
            let mut changed = false;
            if let Some(components) = design.get_mut("components") {
                let components = components
                    .as_array_mut()
                    .ok_or_else(|| refusal(format!("{before}: components is not an array")))?;
                for component in components {
                    // Library links resolve through their library's identity.
                    if component
                        .get("library")
                        .is_some_and(|library| !library.is_null())
                    {
                        continue;
                    }
                    let Some(link) = component.get_mut("link").filter(|link| !link.is_null())
                    else {
                        continue;
                    };
                    let given = link.get("path").and_then(Value::as_str).ok_or_else(|| {
                        refusal(format!("{before}: a component has no source path"))
                    })?;
                    let source = Path::new(given);
                    let resolved = absolute(&parent.join(source))?;
                    let mapped = moved_path(&resolved, &from, &to);
                    if mapped.is_none() && (source.is_absolute() || parent == next_parent) {
                        continue;
                    }
                    let target = mapped.unwrap_or(resolved);
                    let new_path = if source.is_relative() {
                        relative_path(&target, next_parent)
                    } else {
                        target
                    };
                    let new_path = path_text(&new_path)?;
                    if new_path != given {
                        link["path"] = Value::String(new_path);
                        changed = true;
                    }
                }
            }
            if !changed {
                continue;
            }
            let open = command
                .get("open_designs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .any(|open| crate::same_folder(Path::new(open), &path));
            if open {
                return Err(refusal(format!(
                    "{before} is open and its component links must change; close this design before moving the source, then reopen it"
                )));
            }
            let mut bytes =
                serde_json::to_vec_pretty(&design).map_err(|e| VcsError::File(e.to_string()))?;
            bytes.push(b'\n');
            rewrites.push(LinkRewrite {
                before,
                after: self.relative(&destination)?,
                bytes,
            });
        }
        Ok(rewrites)
    }
}
