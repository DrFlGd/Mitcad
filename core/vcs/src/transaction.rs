// SPDX-License-Identifier: MIT
//! Replacing several project files: stage their new bytes and backups
//! before publishing any of them, and keep the backups until the caller
//! has finished its operation (for example, moving and recording them).

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use mitcad_model::file::{rename_retrying, write_atomically};

use crate::{VcsError, io_error};

struct Replacement {
    path: PathBuf,
    staged: PathBuf,
    backup: Option<PathBuf>,
    applied: bool,
}

/// Backups live in a hidden folder directly under `root`, outside any
/// subfolder the operation moves. A caller moving files must reverse the
/// move before rolling this back. If that fails, `retain_backups` stops
/// Drop from restoring files at the old locations and keeps recovery data.
pub(crate) struct FileTransaction {
    root: PathBuf,
    directory: PathBuf,
    files: Vec<Replacement>,
    directories: Vec<PathBuf>,
    finished: bool,
}

impl FileTransaction {
    pub(crate) fn prepare(root: &Path, files: &[(PathBuf, Vec<u8>)]) -> Result<Self, VcsError> {
        let mut paths = BTreeSet::new();
        for (path, _) in files {
            check_destination(root, path)?;
            if !paths.insert(path) {
                return Err(VcsError::InvalidPath(format!(
                    "{} is listed more than once for replacement",
                    path.display()
                )));
            }
        }
        let directory = staging_directory(root)?;
        let mut transaction = Self {
            root: root.to_path_buf(),
            directory,
            files: Vec::new(),
            directories: Vec::new(),
            finished: false,
        };
        for (i, (path, data)) in files.iter().enumerate() {
            let staged = transaction.directory.join(format!("new-{i}"));
            write_atomically(&staged, data).map_err(|e| io_error("stage", path, e))?;
            let backup = match fs::symlink_metadata(path) {
                Ok(metadata) if metadata.is_file() => {
                    let backup = transaction.directory.join(format!("old-{i}"));
                    fs::copy(path, &backup).map_err(|e| io_error("back up", path, e))?;
                    fs::OpenOptions::new()
                        .write(true)
                        .open(&backup)
                        .and_then(|file| file.sync_all())
                        .map_err(|e| io_error("flush the backup of", path, e))?;
                    fs::set_permissions(&staged, metadata.permissions())
                        .map_err(|e| io_error("preserve the permissions of", path, e))?;
                    Some(backup)
                }
                Ok(_) => {
                    return Err(VcsError::InvalidPath(format!(
                        "{} is not a regular file",
                        path.display()
                    )));
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(io_error("inspect", path, e)),
            };
            transaction.files.push(Replacement {
                path: path.clone(),
                staged,
                backup,
                applied: false,
            });
        }
        // If rollback itself meets an I/O failure, the saved originals
        // remain identifiable without guessing which path each belonged to.
        let recovery: Vec<_> = transaction
            .files
            .iter()
            .map(|file| {
                serde_json::json!({
                    "path": file.path.to_string_lossy(),
                    "backup": file.backup.as_ref().map(|path| path.to_string_lossy()),
                    "staged": file.staged.to_string_lossy(),
                })
            })
            .collect();
        let recovery_path = transaction.directory.join("files.json");
        write_atomically(
            &recovery_path,
            serde_json::to_string_pretty(&recovery)
                .expect("file recovery information serializes")
                .as_bytes(),
        )
        .map_err(|e| io_error("write", &recovery_path, e))?;
        Ok(transaction)
    }

    /// Publishes the staged files. A failure rolls back those already
    /// replaced; if restoration also fails, the error names the backups.
    pub(crate) fn apply(&mut self) -> Result<(), VcsError> {
        let applied = (|| -> Result<(), VcsError> {
            for file in &mut self.files {
                check_destination(&self.root, &file.path)?;
                make_parents(&self.root, &file.path, &mut self.directories)?;
                rename_retrying(&file.staged, &file.path)
                    .map_err(|e| io_error("replace", &file.path, e))?;
                file.applied = true;
            }
            Ok(())
        })();
        match applied {
            Ok(()) => Ok(()),
            Err(error) => match self.rollback() {
                Ok(()) => Err(error),
                Err(rollback) => Err(VcsError::Io(format!("{error}; {rollback}"))),
            },
        }
    }

    pub(crate) fn rollback(&mut self) -> Result<(), VcsError> {
        let mut errors = Vec::new();
        for file in self.files.iter_mut().rev().filter(|file| file.applied) {
            let restored = match &file.backup {
                Some(backup) => rename_retrying(backup, &file.path),
                None => match fs::remove_file(&file.path) {
                    Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                    result => result,
                },
            };
            match restored {
                Ok(()) => file.applied = false,
                Err(e) => errors.push(io_error("restore", &file.path, e).to_string()),
            }
        }
        if !errors.is_empty() {
            return Err(VcsError::Io(format!(
                "rollback could not finish: {}; recovery files remain in {}",
                errors.join("; "),
                self.directory.display()
            )));
        }
        for directory in self.directories.iter().rev() {
            // Only directories this transaction made, and only if empty.
            let _ = fs::remove_dir(directory);
        }
        self.finished = true;
        let _ = fs::remove_dir_all(&self.directory);
        Ok(())
    }

    /// The caller's operation succeeded. Failure to remove temporary
    /// backups is a warning, never a failed publication.
    pub(crate) fn commit(mut self) -> Vec<String> {
        self.finished = true;
        match fs::remove_dir_all(&self.directory) {
            Ok(()) => Vec::new(),
            Err(e) => vec![format!(
                "the files were replaced, but temporary backups remain in {}: {e}",
                self.directory.display()
            )],
        }
    }

    /// An outer operation could not move files back. Preserve its recovery
    /// copies instead of writing the old locations from Drop.
    pub(crate) fn retain_backups(&mut self) -> PathBuf {
        self.finished = true;
        self.directory.clone()
    }
}

impl Drop for FileTransaction {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.rollback();
        }
    }
}

/// Reject directories and symlinks before taking backups or publishing.
pub(crate) fn check_destination(root: &Path, path: &Path) -> Result<(), VcsError> {
    let relative = path.strip_prefix(root).map_err(|_| {
        VcsError::InvalidPath(format!("{} is outside {}", path.display(), root.display()))
    })?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(VcsError::InvalidPath(format!(
            "{} is not a file path inside {}",
            path.display(),
            root.display()
        )));
    }
    let mut at = root.to_path_buf();
    for part in relative.components() {
        at.push(part);
        match fs::symlink_metadata(&at) {
            Ok(metadata)
                if metadata.file_type().is_symlink()
                    || (at != path && !metadata.is_dir())
                    || (at == path && !metadata.is_file()) =>
            {
                return Err(VcsError::InvalidPath(format!(
                    "{} is not a regular file or folder",
                    at.display()
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error("inspect", &at, e)),
        }
    }
    Ok(())
}

fn make_parents(root: &Path, path: &Path, created: &mut Vec<PathBuf>) -> Result<(), VcsError> {
    let parent = path.parent().expect("a checked file path");
    let mut at = root.to_path_buf();
    for part in parent
        .strip_prefix(root)
        .expect("a checked path")
        .components()
    {
        at.push(part);
        match fs::create_dir(&at) {
            Ok(()) => created.push(at.clone()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let metadata =
                    fs::symlink_metadata(&at).map_err(|e| io_error("inspect", &at, e))?;
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(VcsError::InvalidPath(format!(
                        "{} is not a regular folder",
                        at.display()
                    )));
                }
            }
            Err(e) => return Err(io_error("make the folder", &at, e)),
        }
    }
    Ok(())
}

fn staging_directory(root: &Path) -> Result<PathBuf, VcsError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    loop {
        let path = root.join(format!(
            ".mitcad-write.{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io_error("stage files in", root, e)),
        }
    }
}
