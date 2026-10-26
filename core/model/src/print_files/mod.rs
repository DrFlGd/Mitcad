// SPDX-License-Identifier: MIT
//! Slicers' settings files (mitcad#640, part 5): reading a slicer's print
//! settings into printer and process profiles and materials, and writing
//! a design's effective settings into the 3MF a slicer opens, in the
//! slicer's dialect ([`crate::print_keys`]).
//!
//! **Import** ([`read_file`]) reads settings only, never geometry:
//!
//! - a 3MF project: the Bambu Studio family's
//!   `Metadata/project_settings.config` (JSON; the dialect by the model's
//!   `Application`), PrusaSlicer's `Metadata/Slic3r_PE.config` (`; key =
//!   value` lines) or UltiMaker Cura's stacks under `Cura/` (INI: the
//!   global and extruder stacks and the instance containers the file
//!   carries, the nearest container first, and the materials' own files);
//! - a Bambu-family preset (`.json`) or exported bundle (a zip of
//!   presets), a PrusaSlicer configuration (`.ini`) or config bundle
//!   (`[print:...]`, `[filament:...]`, `[printer:...]` sections, with
//!   `inherits` within the bundle).
//!
//! Each gives one printer and one process profile and one material per
//! filament; keys the map has become Mitcad's values, other keys are kept
//! per slicer as text (and a Bambu-family printer's whole block, raw).
//! Per-object settings and plates do not target Mitcad's geometry: the
//! report counts them as left out.
//!
//! **Export** ([`export`]) writes each part's effective settings: the
//! values all parts share as the project's, the others per part, a
//! filament slot per distinct material in the order of first use. The
//! Bambu family gets `project_settings.config` with the application
//! metadata its loader needs (see the key map's module documentation) and
//! `model_settings.config` with each part's filament and overrides;
//! PrusaSlicer `Slic3r_PE.config` and `Slic3r_PE_model.config` (each body
//! an object of its own there); Cura each part's settings as the mesh
//! object's `cura:` metadata (Cura takes per-model settings; its printer
//! and the global settings stay the user's choice in Cura).
//!
//! **Purge volumes** (mitcad#705, [`crate::print_purge`]): the export writes
//! the table of the filament slots, computed from the materials and the
//! multiplier with the design's overrides, as the Bambu family's
//! `flush_volumes_matrix` (divided by the multiplier, which it writes
//! beside it as `flush_multiplier`; one table per extruder) and
//! `flush_volumes_vector`, and as PrusaSlicer's `wiping_volumes_matrix`
//! (with `wiping_volumes_use_custom_matrix` on) and
//! `wiping_volumes_extruders`. Cura has no table, and its per-material
//! prime volumes are extruder settings, which its export does not write.
//! The import reads a project's table ([`Imported::purge`]) and keeps none
//! of these keys as the slicer's text.

mod bambu;
#[cfg(test)]
mod corpus;
mod cura;
mod prusa;
mod source;
#[cfg(test)]
mod tests;
// Purge volumes on filament changes (mitcad#705).
#[cfg(test)]
mod purge_tests;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::appearance::{Appearance, library_appearance};
use crate::document::EffectivePrint;
use crate::material::{self, Material};
use crate::print_keys::{self, Dialect, KeyDef, PrintSet};
use crate::print_profile::{PrintProfile, ProfileKind, dialect_value, free_id, slug, value_of};
use crate::print_purge::{self, PurgeOverride};

/// What an import read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Imported {
    pub dialect: Option<Dialect>,
    pub printers: Vec<PrintProfile>,
    pub processes: Vec<PrintProfile>,
    /// Materials with the appearance each carries (from its colour).
    pub materials: Vec<(Material, Option<Appearance>)>,
    pub report: ImportReport,
    /// The project's purge volumes (mitcad#705): from a filament to another
    /// (indices of `materials`), in mm³ as the slicer purges them.
    pub purge: Vec<(usize, usize, f64)>,
}

/// The slicers' keys of the purge table (mitcad#705): read into the
/// design's overrides, written from its table, never kept as text.
pub(crate) const PURGE_KEYS: &[&str] = &[
    "flush_volumes_matrix",
    "flush_volumes_vector",
    "wiping_volumes_matrix",
    "wiping_volumes_extruders",
    "wiping_volumes_use_custom_matrix",
];

/// The numbers of a table's entries (texts), when every entry is one.
pub(crate) fn table_numbers<'a>(entries: impl Iterator<Item = &'a str>) -> Option<Vec<f64>> {
    entries.map(|e| print_keys::parse_number(e).ok()).collect()
}

/// What an import did with the file's keys.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportReport {
    /// The slicer's keys read into Mitcad's values.
    pub mapped: BTreeSet<String>,
    /// The slicer's keys kept as its own text.
    pub kept: BTreeSet<String>,
    /// What was left out and why (per-object settings, plates, formulas).
    pub left_out: Vec<String>,
    /// Values that could not be read.
    pub problems: Vec<String>,
}

impl ImportReport {
    pub fn to_json(&self) -> Value {
        json!({
            "mapped": self.mapped.len(),
            "mapped_keys": self.mapped,
            "kept": self.kept.len(),
            "kept_keys": self.kept,
            "left_out": self.left_out,
            "problems": self.problems,
        })
    }
}

/// Reads a slicer's settings file: a 3MF project, a Bambu-family preset or
/// bundle, a PrusaSlicer configuration or bundle. `name` (the file's name)
/// tells the kind where the content does not; `dialect` names the Bambu
/// family's member when the file does not say.
pub fn read_file(name: &str, data: &[u8], dialect: Option<Dialect>) -> Result<Imported, String> {
    let lower = name.to_ascii_lowercase();
    if data.starts_with(b"PK") {
        let package = mitcad_3mf::read_package(data).map_err(|e| e.to_string())?;
        if package.file(bambu::PROJECT_FILE).is_some() {
            return bambu::read_project(&package, dialect);
        }
        if package.file(prusa::PROJECT_FILE).is_some() {
            return prusa::read_project(&package);
        }
        if package
            .files
            .iter()
            .any(|(p, _)| p.starts_with("Cura/") && p.ends_with(".global.cfg"))
        {
            return cura::read_project(&package);
        }
        if package
            .files
            .iter()
            .any(|(p, _)| p.to_ascii_lowercase().ends_with(".json"))
            && !package
                .files
                .iter()
                .any(|(p, _)| p.eq_ignore_ascii_case("[Content_Types].xml"))
        {
            return bambu::read_bundle(&package, dialect);
        }
        return Err(format!(
            "{name} has no print settings of a slicer Mitcad reads (Bambu Studio, OrcaSlicer, \
             QIDI Studio, PrusaSlicer, UltiMaker Cura)"
        ));
    }
    let text = std::str::from_utf8(data).map_err(|_| format!("{name} is not text"))?;
    if lower.ends_with(".json") || text.trim_start().starts_with('{') {
        return bambu::read_preset(text, dialect);
    }
    if lower.ends_with(".ini") || lower.ends_with(".config") || text.contains(" = ") {
        return prusa::read_config(text);
    }
    Err(format!(
        "{name} is no slicer's settings file Mitcad reads (a 3MF project, a preset .json or \
         bundle, a PrusaSlicer .ini)"
    ))
}

/// Which layers a settings file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Layers {
    pub printer: bool,
    pub process: bool,
    pub filament: bool,
}

impl Layers {
    pub const ALL: Layers = Layers {
        printer: true,
        process: true,
        filament: true,
    };

    pub fn only(set: PrintSet) -> Layers {
        Layers {
            printer: set == PrintSet::Printer,
            process: set == PrintSet::Process,
            filament: set == PrintSet::Filament,
        }
    }

    /// The one layer it holds, if it holds one.
    fn single(self) -> Option<PrintSet> {
        match (self.printer, self.process, self.filament) {
            (true, false, false) => Some(PrintSet::Printer),
            (false, true, false) => Some(PrintSet::Process),
            (false, false, true) => Some(PrintSet::Filament),
            _ => None,
        }
    }
}

/// Reads a flat settings file (every layer's keys together, per-filament
/// values as lists): a printer and a process profile (`names`: their
/// presets' names, when the file gives them) and a material per filament,
/// as `layers` says; keys the map does not have are kept as the slicer's
/// text in the layer their name says (a file of one layer: in that one).
pub(crate) fn read_flat(
    dialect: Dialect,
    source: &dyn source::Source,
    filaments: usize,
    layers: Layers,
    names: (Option<String>, Option<String>),
    out: &mut Imported,
) {
    let mapped = source::mapped_keys(dialect);
    let filaments = filaments.max(1);
    let mut printer_kept = BTreeMap::new();
    let mut process_kept = BTreeMap::new();
    let mut filament_kept: Vec<BTreeMap<String, String>> = vec![BTreeMap::new(); filaments];
    for key in source.keys() {
        if source::BOOKKEEPING.contains(&key.as_str()) {
            continue;
        }
        // The purge table: the reader's own (mitcad#705).
        if mapped.contains(key.as_str()) || PURGE_KEYS.contains(&key.as_str()) {
            out.report.mapped.insert(key);
            continue;
        }
        let Some(raw) = source.raw(&key) else {
            continue;
        };
        let set = layers
            .single()
            .unwrap_or_else(|| source::leftover_set(&key));
        match set {
            PrintSet::Filament if layers.filament => {
                let per_filament = filaments > 1 && source.count(&key) == Some(filaments);
                for (i, kept) in filament_kept.iter_mut().enumerate() {
                    let text = if per_filament {
                        source.text(&key, true, i).unwrap_or_default()
                    } else {
                        raw.clone()
                    };
                    kept.insert(key.clone(), text);
                }
            }
            PrintSet::Printer if layers.printer => {
                printer_kept.insert(key.clone(), raw);
            }
            PrintSet::Process if layers.process => {
                process_kept.insert(key.clone(), raw);
            }
            _ => continue,
        }
        out.report.kept.insert(key);
    }
    let mut problems = Vec::new();
    if layers.printer {
        let values = source::read_layer(dialect, PrintSet::Printer, source, 0, &mut problems);
        out.printers.push(profile_from(
            dialect,
            ProfileKind::Printer,
            values,
            printer_kept,
            names.0.clone(),
            "Imported printer",
        ));
    }
    if layers.process {
        let values = source::read_layer(dialect, PrintSet::Process, source, 0, &mut problems);
        out.processes.push(profile_from(
            dialect,
            ProfileKind::Process,
            values,
            process_kept,
            names.1.clone(),
            "Imported process",
        ));
    }
    if layers.filament {
        for (i, kept) in filament_kept.into_iter().enumerate() {
            let values = source::read_layer(dialect, PrintSet::Filament, source, i, &mut problems);
            let taken: Vec<String> = out.materials.iter().map(|(m, _)| m.id.clone()).collect();
            out.materials
                .push(material_from_filament(dialect, values, kept, i, &|id| {
                    taken.iter().any(|t| t == id)
                }));
        }
    }
    out.report.problems.extend(problems);
}

/// Text for XML attributes and elements.
pub(crate) fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A filament's values read from a slicer as a material: named after its
/// preset (or its type), its density the filament's (else the built-in of
/// its type's), its colour as a plastic appearance, the rest as its
/// filament values and the slicer's own keys.
pub(crate) fn material_from_filament(
    dialect: Dialect,
    mut values: BTreeMap<String, Value>,
    slicer: BTreeMap<String, String>,
    index: usize,
    taken: &dyn Fn(&str) -> bool,
) -> (Material, Option<Appearance>) {
    let preset = values
        .get("filament_settings_id")
        .and_then(|v| dialect_value(v, dialect))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned);
    let filament_type = values
        .get("filament_type")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let name = preset.clone().unwrap_or_else(|| match &filament_type {
        Some(t) => format!("{t} filament {}", index + 1),
        None => format!("Filament {}", index + 1),
    });
    let based_on = filament_type.as_deref().and_then(|t| {
        material::built_in()
            .iter()
            .find(|m| {
                m.filament
                    .get("filament_type")
                    .and_then(Value::as_str)
                    .is_some_and(|own| own.eq_ignore_ascii_case(t))
            })
            .map(|m| m.id.clone())
    });
    let density = values
        .remove("filament_density")
        .and_then(|v| v.as_f64())
        .filter(|d| *d > 0.0 && d.is_finite())
        .or_else(|| {
            based_on
                .as_deref()
                .and_then(material::built_in_material)
                .map(|m| m.density)
        })
        .unwrap_or(1.24);
    let id = free_id(&slug(&name), taken);
    let appearance = values
        .get("filament_colour")
        .and_then(Value::as_str)
        .and_then(hex_rgb)
        .map(|color| plastic_appearance(&id, &name, color));
    // The filament's values a material keeps.
    values.retain(|key, value| material::check_filament_value(key, value).is_ok());
    let material = Material {
        id,
        name,
        density,
        appearance: appearance.as_ref().map(|a| a.id.clone()),
        filament: values,
        slicer: if slicer.is_empty() {
            BTreeMap::new()
        } else {
            BTreeMap::from([(dialect.id().to_owned(), slicer)])
        },
        based_on,
        class: None,
        version: None,
        origin: None,
    };
    (material, appearance)
}

/// `#RRGGBB` (or `#RRGGBBAA`) as sRGB components in [0, 1].
pub(crate) fn hex_rgb(text: &str) -> Option<[f64; 3]> {
    let hex = print_keys::color(text)?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some([byte(1)?, byte(3)?, byte(5)?].map(|c| f64::from(c) / 255.0))
}

/// A plastic appearance of a colour for an imported material: the library's
/// white plastic in that colour, its id the material's.
pub(crate) fn plastic_appearance(material: &str, name: &str, color: [f64; 3]) -> Appearance {
    let mut look = library_appearance("plastic_white")
        .cloned()
        .unwrap_or_default();
    look.id = format!("{}_colour", material.replace(['-', '/'], "_"));
    look.name = format!("{name} colour");
    look.base_color = color;
    look.texture = None;
    look
}

/// A profile from values read: named after its preset (`printer_settings_id`
/// or `print_settings_id` for the dialect), else `fallback`.
pub(crate) fn profile_from(
    dialect: Dialect,
    kind: ProfileKind,
    values: BTreeMap<String, Value>,
    slicer: BTreeMap<String, String>,
    name: Option<String>,
    fallback: &str,
) -> PrintProfile {
    let id_key = match kind {
        ProfileKind::Printer => "printer_settings_id",
        ProfileKind::Process => "print_settings_id",
    };
    let name = name
        .filter(|n| !n.trim().is_empty())
        .or_else(|| {
            values
                .get(id_key)
                .and_then(|v| dialect_value(v, dialect))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned());
    let mut profile = PrintProfile::new(&slug(&name), name.trim(), kind);
    profile.values = values;
    if !slicer.is_empty() {
        profile.slicer.insert(dialect.id().to_owned(), slicer);
    }
    profile.normalize();
    profile
}

/// The settings written for a slicer: the 3MF's application, its more
/// metadata and namespaces, more files, and each part's own metadata.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExportOutput {
    pub application: Option<String>,
    pub metadata: Vec<(String, String)>,
    pub namespaces: Vec<(String, String)>,
    pub files: Vec<(String, Vec<u8>)>,
    pub part_metadata: Vec<Vec<(String, String)>>,
    /// The filament slot of each part (0 the first).
    pub slots: Vec<usize>,
    pub warnings: Vec<String>,
}

/// A part of an export: its name, settings, number of triangles and of
/// placements.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportPart<'a> {
    pub name: &'a str,
    pub settings: &'a EffectivePrint,
    pub triangles: usize,
    pub placements: usize,
}

/// What every export shares: the slots and the values all parts share.
pub(crate) struct Shared<'a> {
    pub dialect: Dialect,
    pub parts: &'a [ExportPart<'a>],
    /// Each part's slot.
    pub slots: Vec<usize>,
    /// Each slot's first part.
    pub slot_parts: Vec<usize>,
    /// Printer and process values every part has alike.
    pub values: BTreeMap<String, Value>,
    /// Each part's values that are not shared.
    pub overrides: Vec<BTreeMap<String, Value>>,
    /// The purge table of the slots (mitcad#705).
    pub purge: print_purge::Matrix,
}

impl<'a> Shared<'a> {
    fn new(dialect: Dialect, parts: &'a [ExportPart<'a>], purge: &[PurgeOverride]) -> Self {
        let mut materials: Vec<&str> = Vec::new();
        let mut slot_parts = Vec::new();
        let slots = parts
            .iter()
            .enumerate()
            .map(
                |(i, p)| match materials.iter().position(|m| *m == p.settings.material) {
                    Some(slot) => slot,
                    None => {
                        materials.push(&p.settings.material);
                        slot_parts.push(i);
                        materials.len() - 1
                    }
                },
            )
            .collect();
        let mut values = BTreeMap::new();
        if let Some(first) = parts.first() {
            for (key, (value, _)) in &first.settings.values {
                if parts[1..]
                    .iter()
                    .all(|p| p.settings.values.get(key).is_some_and(|(v, _)| v == value))
                {
                    values.insert(key.clone(), value.clone());
                }
            }
        }
        let overrides = parts
            .iter()
            .map(|p| {
                p.settings
                    .values
                    .iter()
                    .filter(|(key, _)| !values.contains_key(*key))
                    .map(|(key, (value, _))| (key.clone(), value.clone()))
                    .collect()
            })
            .collect();
        let firsts: Vec<&EffectivePrint> = slot_parts.iter().map(|&i| parts[i].settings).collect();
        let purge = purge_matrix(&firsts, purge);
        Shared {
            dialect,
            parts,
            slots,
            slot_parts,
            values,
            overrides,
            purge,
        }
    }

    /// The settings of the design's printer: the first part's.
    fn first(&self) -> Option<&EffectivePrint> {
        self.parts.first().map(|p| p.settings)
    }

    /// A slot's filament values.
    fn filament(&self, slot: usize) -> &BTreeMap<String, (Value, crate::document::PrintFrom)> {
        &self.parts[self.slot_parts[slot]].settings.filament
    }

    /// The printer's filament slots, when it says.
    fn printer_slots(&self) -> Option<i64> {
        match self.first()?.value("filament_slots") {
            Some(print_keys::Value::Integer(n)) => Some(n),
            _ => None,
        }
    }
}

/// A Mitcad value as the dialect's keys and texts: none when the dialect
/// has no counterpart, or the value does not convert (a warning).
pub(crate) fn dialect_texts(
    dialect: Dialect,
    def: &'static KeyDef,
    value: &Value,
    context: &BTreeMap<String, Value>,
    warnings: &mut Vec<String>,
) -> Vec<(&'static str, String)> {
    let Some(dk) = def.dialect(dialect) else {
        return Vec::new();
    };
    if def.per_dialect {
        return dialect_value(value, dialect)
            .and_then(Value::as_str)
            .map(|text| vec![(dk.key, text.to_owned())])
            .unwrap_or_default();
    }
    let parsed = match value_of(def, value) {
        Ok(parsed) => parsed,
        Err(e) => {
            warnings.push(format!("{}: {e}", def.key));
            return Vec::new();
        }
    };
    let lookup = |key: &str| -> Option<print_keys::Value> {
        let other = print_keys::key(key)?;
        value_of(other, context.get(key)?).ok()
    };
    match print_keys::write(def, dialect, &parsed, &lookup) {
        Ok(pairs) => pairs,
        Err(e) => {
            warnings.push(format!("{} for {dialect}: {e}", def.key));
            Vec::new()
        }
    }
}

/// The purge table of filament slots, each the settings of its first
/// part (in slot order): the materials' values, the first slot's
/// multiplier (the design's printer and process are the first part's) and
/// `overrides`.
pub fn purge_matrix(slots: &[&EffectivePrint], overrides: &[PurgeOverride]) -> print_purge::Matrix {
    let values: Vec<(&str, print_purge::PurgeValues)> = slots
        .iter()
        .map(|settings| {
            let values = print_purge::filament_values(|k| settings.filament.get(k).map(|(v, _)| v));
            (settings.material.as_str(), values)
        })
        .collect();
    let multiplier = print_purge::multiplier_of(
        slots
            .first()
            .and_then(|s| s.values.get(print_purge::MULTIPLIER))
            .map(|(v, _)| v),
    );
    print_purge::Matrix::new(&values, multiplier, overrides)
}

/// Writes the parts' settings for a slicer, with the design's purge
/// volumes set by hand (mitcad#705).
pub fn export(
    dialect: Dialect,
    model_name: &str,
    parts: &[ExportPart<'_>],
    purge: &[PurgeOverride],
) -> ExportOutput {
    let shared = Shared::new(dialect, parts, purge);
    let mut out = match dialect {
        Dialect::BambuStudio | Dialect::OrcaSlicer | Dialect::QidiStudio => {
            bambu::export(&shared, model_name)
        }
        Dialect::PrusaSlicer => prusa::export(&shared),
        Dialect::Cura => cura::export(&shared),
    };
    let materials = shared.slot_parts.len();
    if let Some(slots) = shared.printer_slots()
        && materials as i64 > slots
    {
        out.warnings.push(format!(
            "the parts are of {materials} materials, and the printer has {slots} filament slots"
        ));
    }
    out.slots = shared.slots.clone();
    out
}

/// A filament key's entries per slot for the Bambu family's and
/// PrusaSlicer's lists: an override of the printer's extruder settings
/// that some slots set takes `nil` in the others (the printer's value
/// there), so that the list is whole and is written. The answer says which
/// slots set the key themselves.
pub(crate) fn fill_overrides(def: &KeyDef, entries: &mut [Option<String>]) -> Vec<bool> {
    let set: Vec<bool> = entries.iter().map(Option::is_some).collect();
    if def.overrides_printer() && set.contains(&true) {
        for entry in entries.iter_mut() {
            entry.get_or_insert_with(|| "nil".to_owned());
        }
    }
    set
}

/// The keys of the map in a set that a values map has, in the table's
/// order.
pub(crate) fn defs_in(
    set: PrintSet,
    values: &BTreeMap<String, Value>,
) -> impl Iterator<Item = (&'static KeyDef, &Value)> {
    print_keys::keys_in(set).filter_map(move |def| values.get(def.key).map(|v| (def, v)))
}
