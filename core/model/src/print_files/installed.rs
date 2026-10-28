// SPDX-License-Identifier: MIT
//! The user presets of the installed Bambu-family slicers (mitcad#830):
//! where each keeps its data folder on each platform, the printer,
//! process and filament presets the user made there, and a preset with
//! the values it inherits from the slicer's system presets filled in.
//! Read only: the slicer's files are never changed.
//!
//! The layout, as the slicers' own sources make it (read 27.10.2026):
//!
//! - The data folder is named by the slicer's `SLIC3R_APP_KEY`
//!   (`version.inc` of each: `BambuStudio`, `OrcaSlicer`, `QIDIStudio`).
//!   `GUI_App::init_app_config` (`src/slic3r/GUI/GUI_App.cpp`) puts it in
//!   wxWidgets' user data folder, `%APPDATA%\<key>` on Windows and
//!   `~/Library/Application Support/<key>` on macOS, and on Linux in
//!   `$XDG_CONFIG_HOME/<key>`, else `~/.config/<key>`. A Flatpak's
//!   `XDG_CONFIG_HOME` is `~/.var/app/<app id>/config`: Flathub's
//!   `com.bambulab.BambuStudio` and `com.orcaslicer.OrcaSlicer`, and
//!   `io.github.softfever.OrcaSlicer`, the id OrcaSlicer's
//!   `migrate_flatpak_legacy_datadir` (same file) moves data from.
//! - User presets are in `<data>/user/<account>/` (`PRESET_USER_DIR`
//!   `"user"` in `src/libslic3r/Preset.hpp`; the account's id, or
//!   `DEFAULT_USER_FOLDER_NAME` `"default"` in `PresetBundle.hpp` when no
//!   one is signed in), one folder per kind: `machine`, `process`,
//!   `filament` (`PRESET_PRINTER_NAME`, `PRESET_PRINT_NAME`,
//!   `PRESET_FILAMENT_NAME`), each preset a `.json` file beside its
//!   `.info`.
//! - System presets are in `<data>/system/` (`PRESET_SYSTEM_DIR`), a
//!   folder per vendor; a preset names the one it inherits from by its
//!   `name` (`inherits`), which may inherit further.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::print_keys::{Dialect, PrintSet};

/// Which platform's folders the slicers use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
}

/// The folders a slicer's data folder is found from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetEnv {
    pub platform: Platform,
    /// The user's home folder.
    pub home: Option<PathBuf>,
    /// Windows' `%APPDATA%`.
    pub app_data: Option<PathBuf>,
    /// Linux's `$XDG_CONFIG_HOME`.
    pub config_home: Option<PathBuf>,
}

impl PresetEnv {
    /// This computer's.
    pub fn current() -> PresetEnv {
        let platform = if cfg!(windows) {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        };
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        PresetEnv {
            platform,
            home: var("HOME").or_else(|| var("USERPROFILE")),
            app_data: var("APPDATA"),
            config_home: var("XDG_CONFIG_HOME"),
        }
    }
}

/// The Bambu-family slicers: their dialect, data folder name and Flatpak
/// app ids (see the module documentation for the sources).
const SLICERS: [(Dialect, &str, &[&str]); 3] = [
    (
        Dialect::BambuStudio,
        "BambuStudio",
        &["com.bambulab.BambuStudio"],
    ),
    (
        Dialect::OrcaSlicer,
        "OrcaSlicer",
        &[
            "com.orcaslicer.OrcaSlicer",
            "io.github.softfever.OrcaSlicer",
        ],
    ),
    (Dialect::QidiStudio, "QIDIStudio", &[]),
];

/// The user presets' folder in a data folder.
const USER_DIR: &str = "user";
/// The system presets' folder in a data folder.
const SYSTEM_DIR: &str = "system";
/// The folders of each kind in an account's folder.
const KIND_DIRS: [(&str, PrintSet); 3] = [
    ("machine", PrintSet::Printer),
    ("process", PrintSet::Process),
    ("filament", PrintSet::Filament),
];

/// A slicer's data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlicerFolder {
    pub dialect: Dialect,
    pub folder: PathBuf,
    /// The slicer's Flatpak's (Linux).
    pub flatpak: bool,
}

/// Where each Bambu-family slicer keeps its data folder in `env`'s layout,
/// whether it is there or not: one per slicer, on Linux also its Flatpak's.
pub fn candidate_folders(env: &PresetEnv) -> Vec<SlicerFolder> {
    let mut out = Vec::new();
    for (dialect, key, flatpaks) in SLICERS {
        let mut add = |folder: PathBuf, flatpak: bool| {
            out.push(SlicerFolder {
                dialect,
                folder,
                flatpak,
            });
        };
        match env.platform {
            Platform::Windows => {
                if let Some(app_data) = &env.app_data {
                    add(app_data.join(key), false);
                }
            }
            Platform::MacOs => {
                if let Some(home) = &env.home {
                    add(
                        home.join("Library").join("Application Support").join(key),
                        false,
                    );
                }
            }
            Platform::Linux => {
                let config = env
                    .config_home
                    .clone()
                    .or_else(|| env.home.as_ref().map(|h| h.join(".config")));
                if let Some(config) = config {
                    add(config.join(key), false);
                }
                if let Some(home) = &env.home {
                    for id in flatpaks {
                        let folder = home.join(".var").join("app").join(id);
                        add(folder.join("config").join(key), true);
                    }
                }
            }
        }
    }
    out
}

/// The data folders of the slicers installed in `env`: those that have a
/// user presets' folder.
pub fn slicer_folders(env: &PresetEnv) -> Vec<SlicerFolder> {
    candidate_folders(env)
        .into_iter()
        .filter(|s| s.folder.join(USER_DIR).is_dir())
        .collect()
}

/// A user preset found in a slicer's data folder.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundPreset {
    /// Printer (a `machine` preset), process or filament.
    pub set: PrintSet,
    /// Its `name`, else its file's name.
    pub name: String,
    pub path: PathBuf,
    /// The account folder it is in (`default` without one).
    pub account: String,
    /// The preset it inherits from, by name.
    pub inherits: Option<String>,
}

/// The sorted entries of a folder.
fn entries(dir: &Path) -> Vec<std::fs::DirEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<_> = read.filter_map(Result::ok).collect();
    out.sort_by_key(std::fs::DirEntry::file_name);
    out
}

/// The `.json` files under `dir`, down to `depth` folders below it.
fn json_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    for entry in entries(dir) {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if depth > 0 {
                json_files(&path, depth - 1, out);
            }
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            out.push(path);
        }
    }
}

/// A preset file's fields; None when it is no JSON object.
fn read_preset_file(path: &Path) -> Option<Map<String, Value>> {
    let data = std::fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

fn text_of(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// The user presets in a slicer's data folder: every account's printers,
/// processes and filaments (also those in a folder below a kind's, such
/// as `filament/base`), by account, kind and file name. Files that are no
/// preset are left out.
pub fn user_presets(folder: &Path) -> Vec<FoundPreset> {
    let mut out = Vec::new();
    for account in entries(&folder.join(USER_DIR)) {
        if !account.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let account_name = account.file_name().to_string_lossy().into_owned();
        for (dir, set) in KIND_DIRS {
            let mut files = Vec::new();
            json_files(&account.path().join(dir), 1, &mut files);
            for path in files {
                let Some(map) = read_preset_file(&path) else {
                    continue;
                };
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push(FoundPreset {
                    set,
                    name: text_of(&map, "name").unwrap_or(stem),
                    path: path.clone(),
                    account: account_name.clone(),
                    inherits: text_of(&map, "inherits"),
                });
            }
        }
    }
    out
}

/// The kind of preset a file under an account's folder is, by its folder.
fn set_of(folder: &Path, path: &Path) -> Option<PrintSet> {
    let relative = path.strip_prefix(folder.join(USER_DIR)).ok()?;
    let kind = relative.components().nth(1)?.as_os_str().to_str()?;
    KIND_DIRS
        .iter()
        .find(|(dir, _)| *dir == kind)
        .map(|(_, set)| *set)
}

/// The presets a preset may inherit from, by name: the user's own of the
/// same account first, then the system presets.
struct Bases {
    presets: Vec<(String, PathBuf)>,
}

impl Bases {
    fn of(folder: &Path, account: Option<&Path>) -> Bases {
        let mut files = Vec::new();
        if let Some(account) = account {
            json_files(account, 2, &mut files);
        }
        json_files(&folder.join(SYSTEM_DIR), 4, &mut files);
        let presets = files
            .into_iter()
            .filter_map(|path| {
                let map = read_preset_file(&path)?;
                Some((text_of(&map, "name")?, path))
            })
            .collect();
        Bases { presets }
    }

    fn find(&self, name: &str) -> Option<&Path> {
        self.presets
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, p)| p.as_path())
    }
}

/// The deepest chain of `inherits` followed.
const MAX_INHERITS: usize = 16;

/// A user preset of a slicer's data folder `folder` with the values it
/// inherits filled in: from the preset it names in `inherits` (the user's
/// own of its account, else a system preset), and so on up the chain, the
/// nearer preset's value winning. The result is named as the preset and
/// says its kind (`type` and the slicer's id key of its kind), and has no
/// `inherits` when the whole chain was found; a base that is missing
/// leaves `inherits` naming it, so that the import reports the values
/// not read.
pub fn resolved_preset(folder: &Path, path: &Path) -> Result<Map<String, Value>, String> {
    let shown = path.display();
    let own = read_preset_file(path).ok_or_else(|| format!("{shown} is no preset file"))?;
    let set = set_of(folder, path);
    let account = path
        .strip_prefix(folder.join(USER_DIR))
        .ok()
        .and_then(|r| r.components().next())
        .map(|c| folder.join(USER_DIR).join(c.as_os_str()));
    let name = text_of(&own, "name").unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let mut chain = vec![own];
    let mut bases: Option<Bases> = None;
    let mut missing = None;
    while let Some(base) = chain.last().and_then(|m| text_of(m, "inherits")) {
        if chain.len() > MAX_INHERITS {
            return Err(format!(
                "{name}: the presets it inherits from inherit in a loop"
            ));
        }
        let index = bases.get_or_insert_with(|| Bases::of(folder, account.as_deref()));
        match index.find(&base).and_then(read_preset_file) {
            Some(map) => chain.push(map),
            None => {
                missing = Some(base);
                break;
            }
        }
    }
    // The farthest first, each nearer one's values over it.
    let mut merged = Map::new();
    for map in chain.into_iter().rev() {
        for (key, value) in map {
            merged.insert(key, value);
        }
    }
    merged.insert("name".to_owned(), json!(name));
    match missing {
        Some(base) => {
            merged.insert("inherits".to_owned(), json!(base));
        }
        None => {
            merged.remove("inherits");
        }
    }
    if let Some(set) = set {
        let (kind, id_key, id) = match set {
            PrintSet::Printer => ("machine", "printer_settings_id", json!(name)),
            PrintSet::Process => ("process", "print_settings_id", json!(name)),
            PrintSet::Filament => ("filament", "filament_settings_id", json!([name])),
        };
        merged.insert("type".to_owned(), json!(kind));
        merged.insert(id_key.to_owned(), id);
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(platform: Platform, home: &str) -> PresetEnv {
        PresetEnv {
            platform,
            home: Some(PathBuf::from(home)),
            app_data: None,
            config_home: None,
        }
    }

    fn folders(env: &PresetEnv) -> Vec<(Dialect, PathBuf)> {
        candidate_folders(env)
            .into_iter()
            .map(|s| (s.dialect, s.folder))
            .collect()
    }

    #[test]
    fn data_folders_follow_each_platform_s_layout() {
        let mut windows = env(Platform::Windows, "C:/Users/u");
        windows.app_data = Some(PathBuf::from("C:/Users/u/AppData/Roaming"));
        let roaming = Path::new("C:/Users/u/AppData/Roaming");
        assert_eq!(
            folders(&windows),
            vec![
                (Dialect::BambuStudio, roaming.join("BambuStudio")),
                (Dialect::OrcaSlicer, roaming.join("OrcaSlicer")),
                (Dialect::QidiStudio, roaming.join("QIDIStudio")),
            ]
        );
        // Without %APPDATA% none.
        assert!(folders(&env(Platform::Windows, "C:/Users/u")).is_empty());

        let support = Path::new("/Users/u/Library/Application Support");
        assert_eq!(
            folders(&env(Platform::MacOs, "/Users/u")),
            vec![
                (Dialect::BambuStudio, support.join("BambuStudio")),
                (Dialect::OrcaSlicer, support.join("OrcaSlicer")),
                (Dialect::QidiStudio, support.join("QIDIStudio")),
            ]
        );

        let home = Path::new("/home/u");
        let flatpak = |id: &str, key: &str| {
            home.join(".var")
                .join("app")
                .join(id)
                .join("config")
                .join(key)
        };
        assert_eq!(
            folders(&env(Platform::Linux, "/home/u")),
            vec![
                (
                    Dialect::BambuStudio,
                    home.join(".config").join("BambuStudio")
                ),
                (
                    Dialect::BambuStudio,
                    flatpak("com.bambulab.BambuStudio", "BambuStudio")
                ),
                (Dialect::OrcaSlicer, home.join(".config").join("OrcaSlicer")),
                (
                    Dialect::OrcaSlicer,
                    flatpak("com.orcaslicer.OrcaSlicer", "OrcaSlicer")
                ),
                (
                    Dialect::OrcaSlicer,
                    flatpak("io.github.softfever.OrcaSlicer", "OrcaSlicer")
                ),
                (Dialect::QidiStudio, home.join(".config").join("QIDIStudio")),
            ]
        );
        // $XDG_CONFIG_HOME in place of ~/.config.
        let mut xdg = env(Platform::Linux, "/home/u");
        xdg.config_home = Some(PathBuf::from("/cfg"));
        assert_eq!(
            folders(&xdg)[0],
            (Dialect::BambuStudio, PathBuf::from("/cfg/BambuStudio"))
        );
    }

    /// A fake home with the slicers' folders, in the test's own folder.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mitcad-installed-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, value: &Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, value.to_string()).unwrap();
    }

    #[test]
    fn installed_slicers_are_those_with_user_presets_in_a_fake_home() {
        // Linux: OrcaSlicer in ~/.config, Bambu Studio as a Flatpak; QIDI
        // Studio's folder without user presets is not installed.
        let home = scratch("linux");
        std::fs::create_dir_all(home.join(".config/OrcaSlicer/user/default")).unwrap();
        std::fs::create_dir_all(
            home.join(".var/app/com.bambulab.BambuStudio/config/BambuStudio/user/123"),
        )
        .unwrap();
        std::fs::create_dir_all(home.join(".config/QIDIStudio/system")).unwrap();
        let found: Vec<(Dialect, bool)> =
            slicer_folders(&env(Platform::Linux, home.to_str().unwrap()))
                .into_iter()
                .map(|s| (s.dialect, s.flatpak))
                .collect();
        assert_eq!(
            found,
            vec![(Dialect::BambuStudio, true), (Dialect::OrcaSlicer, false)]
        );

        // Windows: %APPDATA%\QIDIStudio.
        let app_data = scratch("windows");
        std::fs::create_dir_all(app_data.join("QIDIStudio/user/default")).unwrap();
        let mut windows = env(Platform::Windows, "unused");
        windows.app_data = Some(app_data.clone());
        assert_eq!(
            slicer_folders(&windows),
            vec![SlicerFolder {
                dialect: Dialect::QidiStudio,
                folder: app_data.join("QIDIStudio"),
                flatpak: false,
            }]
        );

        // macOS: ~/Library/Application Support/BambuStudio.
        let mac = scratch("macos");
        std::fs::create_dir_all(mac.join("Library/Application Support/BambuStudio/user/default"))
            .unwrap();
        let found = slicer_folders(&env(Platform::MacOs, mac.to_str().unwrap()));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].dialect, Dialect::BambuStudio);
    }

    #[test]
    fn user_presets_are_listed_and_resolved_through_their_system_presets() {
        let folder = scratch("resolve").join("BambuStudio");
        write(
            &folder.join("system/Acme/filament/fdm_filament_common.json"),
            &json!({"type": "filament", "name": "fdm_filament_common", "instantiation": "false",
                    "filament_density": ["1.24"], "nozzle_temperature": ["200"]}),
        );
        write(
            &folder.join("system/Acme/filament/Acme PLA.json"),
            &json!({"type": "filament", "name": "Acme PLA", "inherits": "fdm_filament_common",
                    "filament_type": ["PLA"], "nozzle_temperature": ["210"]}),
        );
        write(
            &folder.join("user/default/filament/My PLA.json"),
            &json!({"name": "My PLA", "inherits": "Acme PLA", "from": "User",
                    "nozzle_temperature": ["215"]}),
        );
        std::fs::write(
            folder.join("user/default/filament/My PLA.info"),
            "sync_info = \n",
        )
        .unwrap();
        write(
            &folder.join("user/default/process/Lost.json"),
            &json!({"name": "Lost", "inherits": "No such preset", "layer_height": "0.3"}),
        );
        let found = user_presets(&folder);
        let names: Vec<(PrintSet, &str)> = found.iter().map(|p| (p.set, p.name.as_str())).collect();
        assert_eq!(
            names,
            vec![(PrintSet::Process, "Lost"), (PrintSet::Filament, "My PLA")]
        );
        assert_eq!(found[1].inherits.as_deref(), Some("Acme PLA"));
        assert_eq!(found[1].account, "default");

        let resolved = resolved_preset(&folder, &found[1].path).unwrap();
        assert_eq!(resolved["name"], "My PLA");
        assert_eq!(
            resolved["nozzle_temperature"],
            json!(["215"]),
            "its own value wins"
        );
        assert_eq!(
            resolved["filament_type"],
            json!(["PLA"]),
            "from its system preset"
        );
        assert_eq!(
            resolved["filament_density"],
            json!(["1.24"]),
            "from that one's base"
        );
        assert_eq!(resolved["filament_settings_id"], json!(["My PLA"]));
        assert!(!resolved.contains_key("inherits"));

        let lost = resolved_preset(&folder, &found[0].path).unwrap();
        assert_eq!(
            lost["inherits"], "No such preset",
            "a missing base stays named"
        );
        assert_eq!(lost["type"], "process");
    }
}
