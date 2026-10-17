// SPDX-License-Identifier: MIT
//! What the timeline shows of the import's outcomes (mitcad#170): the
//! features the import made get import notes
//! ([`mitcad_model::Document::set_import_notes`]), built from the report's
//! own notes and warnings, so that the timeline and the report say the
//! same thing:
//!
//! - a fallback's base features: "Stands in for Extrude5 (its result is not
//!   in the file's history)", every item it stands in for with its note;
//! - a base feature the import added for another reason (bringing the
//!   bodies to a history state exactly, the stored bodies at the end): the
//!   report's warning about it;
//! - an item that came in partly: "Partly imported: " and its note;
//! - an item skipped because it was suppressed or failed in the file: a
//!   suppressed placeholder at its place, its note the item's
//!   ([`Importer::placeholder`]). Items skipped for having no geometry
//!   (occurrences, groups, …) stay out.

use std::collections::{BTreeMap, HashMap};

use mitcad_model::FeatureUid;

use crate::Importer;
use crate::report::{ItemReport, Outcome};

/// A trace line with the time (`MITCAD_IMPORT_TRACE`).
macro_rules! trace {
    ($($arg:tt)*) => {
        if crate::tracing() {
            eprintln!("import: [{:.2}] {}", crate::trace_clock(), format!($($arg)*));
        }
    };
}

/// What the import recorded for the timeline's notes while it ran.
#[derive(Debug, Default)]
pub(crate) struct Notes {
    /// The base features each fallback made, by the first (one per
    /// component that changed, [`Importer::replace_bodies`]).
    groups: HashMap<FeatureUid, Vec<FeatureUid>>,
    /// The items (report entries) each fallback's base features stand in
    /// for, by the first base feature.
    stand_ins: BTreeMap<FeatureUid, Vec<usize>>,
    /// Base features made for another reason than an item's fallback, with
    /// the report's warning about them.
    warnings: Vec<(FeatureUid, String)>,
    /// The items (report entries) suppressed in the file that came in as
    /// suppressed features (mitcad#180).
    suppressed: Vec<usize>,
}

/// How many items a base feature's note names; the rest are counted.
const NAMED_ITEMS: usize = 8;

/// The note of a pending item that went into the fallback of another
/// item's base feature (`base`, its name): the report says so after the
/// item's own reason.
pub(crate) fn in_fallback_of(base: &str) -> String {
    format!("in the fallback of {base}")
}

/// A note as the start of a timeline message: its first letter capital.
pub(crate) fn sentence(note: &str) -> String {
    let mut chars = note.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The note of a base feature standing in for items: each one's name
/// with its reason, items in a row with the same reason sharing it, at
/// most [`NAMED_ITEMS`] named.
pub(crate) fn stands_in_for(items: &[(String, Option<String>)]) -> String {
    let mut runs: Vec<(Vec<&str>, Option<&str>)> = Vec::new();
    for (name, reason) in items.iter().take(NAMED_ITEMS) {
        let reason = reason.as_deref().map(str::trim).filter(|r| !r.is_empty());
        match runs.last_mut() {
            Some((names, last)) if *last == reason => names.push(name),
            _ => runs.push((vec![name], reason)),
        }
    }
    let mut text = runs
        .iter()
        .map(|(names, reason)| match reason {
            Some(reason) => format!("{} ({reason})", names.join(", ")),
            None => names.join(", "),
        })
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > NAMED_ITEMS {
        text.push_str(&format!(
            " and {} more items (see the import report)",
            items.len() - NAMED_ITEMS
        ));
    }
    format!("Stands in for {text}")
}

/// The note of an item that came in partly.
pub(crate) fn partly(note: Option<&str>) -> String {
    match note.map(str::trim).filter(|n| !n.is_empty()) {
        Some(note) => format!("Partly imported: {note}"),
        None => "Partly imported".to_owned(),
    }
}

/// Adds `note` to the notes of `uid`, after what it has.
fn push(notes: &mut BTreeMap<FeatureUid, String>, uid: FeatureUid, note: String) {
    match notes.get_mut(&uid) {
        Some(text) if !text.contains(&note) => {
            text.push_str("; ");
            text.push_str(&note);
        }
        Some(_) => {}
        None => {
            notes.insert(uid, note);
        }
    }
}

/// The notes of the features a FreeCAD import made, from its report: a
/// fallback's base feature stands in for its object, with the report's
/// notes as the reason; a sketch or feature that came in partly says what
/// was left out. (Placeholders of suppressed features get theirs when they
/// are added.)
pub(crate) fn freecad_notes(report: &crate::freecad::FcstdReport) -> Vec<(FeatureUid, String)> {
    use crate::freecad::ObjectOutcome;
    let mut notes = BTreeMap::new();
    for sketch in &report.sketches {
        if sketch.outcome != ObjectOutcome::Partial {
            continue;
        }
        let Some(uid) = sketch.feature.as_deref().and_then(|f| f.parse().ok()) else {
            continue;
        };
        let left: Vec<String> = sketch
            .dropped
            .iter()
            .map(|d| format!("left out: {d}"))
            .chain(sketch.notes.iter().cloned())
            .collect();
        push(&mut notes, uid, partly(Some(&left.join("; "))));
    }
    for feature in &report.features {
        let reason = feature.notes.join("; ");
        match feature.outcome {
            ObjectOutcome::Fallback => {
                if let Some(uid) = feature.features.first().and_then(|f| f.parse().ok()) {
                    let note = stands_in_for(&[(feature.label.clone(), Some(reason))]);
                    push(&mut notes, uid, note);
                }
            }
            ObjectOutcome::Partial => {
                for uid in feature.features.iter().filter_map(|f| f.parse().ok()) {
                    push(&mut notes, uid, partly(Some(&reason)));
                }
            }
            _ => {}
        }
    }
    notes.into_iter().collect()
}

impl<K: crate::ImportKernel> Importer<'_, K> {
    /// The base features one call of [`Importer::replace_bodies`] made.
    pub(crate) fn note_bases(&mut self, uids: &[FeatureUid]) {
        if let Some(first) = uids.first() {
            self.notes.groups.insert(*first, uids.to_vec());
        }
    }

    /// How many base features the call of [`Importer::replace_bodies`]
    /// that made `base` (its first) made: one per component it changed.
    pub(crate) fn bases_made_with(&self, base: FeatureUid) -> usize {
        self.notes.groups.get(&base).map_or(1, Vec::len)
    }

    /// Base feature `base` (the first a fallback made) stands in for the
    /// item of report entry `at`.
    pub(crate) fn stands_in(&mut self, base: FeatureUid, at: usize) {
        let items = self.notes.stand_ins.entry(base).or_default();
        if !items.contains(&at) {
            items.push(at);
        }
    }

    /// A warning of the report about base feature `base`, which its
    /// timeline note repeats.
    pub(crate) fn base_warning(&mut self, base: FeatureUid, warning: String) {
        self.notes.warnings.push((base, warning.clone()));
        self.report.warnings.push(warning);
    }

    /// A suppressed placeholder at the marker for the item of report entry
    /// `at`, skipped because it was suppressed or failed in the file (its
    /// note says which), in the item's component. Items without geometry
    /// get none.
    pub(crate) fn placeholder(&mut self, at: usize) {
        let item = &self.report.items[at];
        let note = sentence(item.note.as_deref().unwrap_or("skipped"));
        let (name, kind) = (item.name.clone(), item.object_type.clone());
        let kind = kind.trim_start_matches('?');
        if let Err(e) =
            self.doc
                .add_import_placeholder(Some(&name), kind, &note, Some(self.component))
        {
            trace!("{name}: no placeholder: {e}");
        }
    }

    /// The item of report entry `at`, suppressed in the file, came in as
    /// suppressed features (mitcad#180): they get its note.
    pub(crate) fn note_suppressed(&mut self, at: usize) {
        self.notes.suppressed.push(at);
    }

    /// Gives the features the import made their notes from the report
    /// (at the end of the import).
    pub(crate) fn timeline_notes(&mut self) {
        let notes = self.collect_notes();
        if let Err(e) = self.doc.set_import_notes(&notes) {
            trace!("the import's notes did not go on the timeline: {e}");
        }
    }

    fn collect_notes(&mut self) -> Vec<(FeatureUid, String)> {
        let items = &self.report.items;
        let mut stand_ins = std::mem::take(&mut self.notes.stand_ins);
        // A fallback item names its base feature, also where its call site
        // did not say so.
        for (i, item) in items.iter().enumerate() {
            if item.outcome == Outcome::Fallback
                && let Some(base) = item.features.first().and_then(|f| f.parse().ok())
            {
                let entry: &mut Vec<usize> = stand_ins.entry(base).or_default();
                if !entry.contains(&i) {
                    entry.push(i);
                }
            }
        }
        // (Base features only: a try undone may have left its uids to
        // other features.)
        let is_base = |uid: &FeatureUid| {
            self.doc
                .feature(*uid)
                .is_some_and(|f| f.def.type_name() == "base")
        };
        let group = |base: FeatureUid| -> Vec<FeatureUid> {
            let mut uids = self
                .notes
                .groups
                .get(&base)
                .cloned()
                .unwrap_or_else(|| vec![base]);
            uids.retain(is_base);
            uids
        };
        let mut notes = BTreeMap::new();
        for (base, mut entries) in stand_ins {
            let Some(name) = self.doc.feature(base).map(|f| f.name.clone()) else {
                continue;
            };
            entries.sort_unstable();
            let suffix = format!("; {}", in_fallback_of(&name));
            let named: Vec<(String, Option<String>)> = entries
                .iter()
                .map(|&i| {
                    let item: &ItemReport = &items[i];
                    let reason = item.note.as_deref().map(|n| n.replace(&suffix, ""));
                    (item.name.clone(), reason)
                })
                .collect();
            let note = stands_in_for(&named);
            for uid in group(base) {
                push(&mut notes, uid, note.clone());
            }
        }
        for (base, warning) in std::mem::take(&mut self.notes.warnings) {
            for uid in group(base) {
                push(&mut notes, uid, sentence(&warning));
            }
        }
        for item in items.iter().filter(|i| i.outcome == Outcome::Partial) {
            for uid in item.features.iter().filter_map(|f| f.parse().ok()) {
                push(&mut notes, uid, partly(item.note.as_deref()));
            }
        }
        // Suppressed in the file (mitcad#180): their note (a partial one's
        // says it already).
        for &at in &self.notes.suppressed {
            let item = &items[at];
            if item.outcome != Outcome::Partial
                && let Some(note) = item.note.as_deref()
            {
                for uid in item.features.iter().filter_map(|f| f.parse().ok()) {
                    push(&mut notes, uid, sentence(note));
                }
            }
        }
        // Items kept parametric with a result that deviates from the file's
        // (mitcad#379): how far off, so that it can be found later.
        for (uid, note) in self.deviation_notes() {
            push(&mut notes, uid, note);
        }
        // Only features there are (a fallback undone later is gone).
        notes.retain(|uid, _| self.doc.feature(*uid).is_some());
        notes.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_feature_names_the_items_it_stands_in_for() {
        let items = vec![
            (
                "Extrude5".to_owned(),
                Some("its result is not in the file's history".to_owned()),
            ),
            (
                "Fillet2".to_owned(),
                Some("the import was stopped".to_owned()),
            ),
            (
                "Fillet3".to_owned(),
                Some("the import was stopped".to_owned()),
            ),
            ("Shell1".to_owned(), None),
        ];
        assert_eq!(
            stands_in_for(&items),
            "Stands in for Extrude5 (its result is not in the file's history), Fillet2, Fillet3 \
             (the import was stopped), Shell1"
        );
        let many: Vec<_> = (0..11)
            .map(|i| {
                (
                    format!("Item{i}"),
                    Some("the import was stopped".to_owned()),
                )
            })
            .collect();
        assert_eq!(
            stands_in_for(&many),
            "Stands in for Item0, Item1, Item2, Item3, Item4, Item5, Item6, Item7 (the import \
             was stopped) and 3 more items (see the import report)"
        );
    }

    #[test]
    fn notes_read_as_sentences() {
        assert_eq!(sentence("suppressed in the file"), "Suppressed in the file");
        assert_eq!(
            partly(Some("left out: 3 constraints")),
            "Partly imported: left out: 3 constraints"
        );
        assert_eq!(partly(None), "Partly imported");
    }
}
