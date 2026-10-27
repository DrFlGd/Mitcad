// SPDX-License-Identifier: MIT
//! Import notes (mitcad#170): what an importer said about a feature it
//! made, kept in the definition state and the design file so that a saved
//! import still shows it. A fallback's base feature says which items of
//! the file it stands in for and why, an item that came in partly says what
//! was left out, and a placeholder (`placeholder`, suppressed) stands where
//! the file had an item that was suppressed or failed there.
//!
//! A feature with a note has the `warning` status while it builds (a
//! suppressed placeholder stays `suppressed`); the note comes first in its
//! message, before the warnings of its evaluation. A command that changes
//! the feature's definition or one of its dimensions clears the note when
//! the feature then builds (does not fail); undo brings it back with the
//! state.
//!
//! Also here: the join tolerance an importer sets (mitcad#298,
//! [`Document::set_join_tolerance`]).

use std::collections::HashMap;
use std::sync::Arc;

use super::{Added, DocState, Document, ModelError, invalid};
use crate::features::{FeatureDef, FeatureEntry, PlaceholderDef, ValueInput};
use crate::ids::{ComponentUid, FeatureUid};
use crate::kernel::Kernel;
use crate::recompute::{FeatureStatus, Recomputed};

impl DocState {
    /// After an edit that recomputed (`result`): the notes of features that
    /// are gone go, and so do those of features whose definition or
    /// dimensions (the parameters they own) the edit changed (from
    /// `before`) and that did not fail.
    pub(crate) fn settle_import_notes<S>(&mut self, before: &DocState, result: &Recomputed<S>) {
        if self.import_notes.is_empty() {
            return;
        }
        let noted = |features: &[Arc<FeatureEntry>]| -> HashMap<FeatureUid, Arc<FeatureEntry>> {
            features
                .iter()
                .filter(|f| self.import_notes.contains_key(&f.uid))
                .map(|f| (f.uid, f.clone()))
                .collect()
        };
        let now = noted(&self.features);
        let was = noted(&before.features);
        // The expressions of the parameters each noted feature owns (its
        // dimensions), where the edit changed parameters.
        let owned = |state: &DocState| -> HashMap<FeatureUid, Vec<String>> {
            let mut owned: HashMap<FeatureUid, Vec<String>> = HashMap::new();
            if self.parameters != before.parameters {
                for p in state.parameters.iter() {
                    if let Some(owner) = state.parameters.owner(p.id())
                        && self.import_notes.contains_key(&owner)
                    {
                        owned
                            .entry(owner)
                            .or_default()
                            .push(p.expression().to_owned());
                    }
                }
            }
            owned
        };
        let (dims_now, dims_was) = (owned(self), owned(before));
        self.import_notes.retain(|uid, _| {
            let Some(entry) = now.get(uid) else {
                return false;
            };
            let Some(old) = was.get(uid) else {
                return true;
            };
            if (Arc::ptr_eq(entry, old) || entry.def == old.def)
                && dims_now.get(uid) == dims_was.get(uid)
            {
                return true;
            }
            matches!(
                result.result(*uid).map(|r| &r.status),
                Some(FeatureStatus::Failed(_))
            )
        });
    }
}

impl<K: Kernel> Document<K> {
    /// What the importer that made the feature said about it (mitcad#170).
    pub fn import_note(&self, uid: FeatureUid) -> Option<&str> {
        self.state.import_notes.get(&uid).map(String::as_str)
    }

    /// Every feature's import note, by feature.
    pub fn import_notes(&self) -> impl Iterator<Item = (FeatureUid, &str)> {
        self.state
            .import_notes
            .iter()
            .map(|(uid, note)| (*uid, note.as_str()))
    }

    /// Sets features' import notes (an empty one removes the note); one
    /// undo step that recomputes nothing. An importer calls it once its
    /// report is complete.
    pub fn set_import_notes(&mut self, notes: &[(FeatureUid, String)]) -> Result<(), ModelError> {
        if notes.is_empty() {
            return Ok(());
        }
        self.apply_with(|state| {
            for (uid, note) in notes {
                state.require(*uid)?;
                let note = note.trim();
                if note.is_empty() {
                    state.import_notes.remove(uid);
                } else {
                    state.import_notes.insert(*uid, note.to_owned());
                }
            }
            Ok(("Import notes".to_owned(), (), false))
        })
    }

    /// The distance (mm) within which the features' joins still fuse where
    /// they fail exactly (mitcad#298, [`Document::set_join_tolerance`]);
    /// None for exact joins.
    pub fn join_tolerance(&self) -> Option<f64> {
        self.state.join_tolerance
    }

    /// Sets the join tolerance: with Some, a join (of an extrusion, a base
    /// feature, a pattern's copies …) whose tool touches no target still
    /// joins the targets within that distance, and one whose result is
    /// invalid is retried with a fuzzy value up to it
    /// ([`Kernel::join_within`]); a join that works exactly stays exact.
    /// An importer whose bodies match a file's only within its geometric
    /// tolerance sets it; the design file keeps it. One undo step; the
    /// joins recompute. A value that is not finite and positive is refused.
    pub fn set_join_tolerance(&mut self, tolerance: Option<f64>) -> Result<(), ModelError> {
        if tolerance.is_some_and(|t| !t.is_finite() || t <= 0.0) {
            return Err(invalid("the join tolerance must be a positive distance"));
        }
        if self.state.join_tolerance == tolerance {
            return Ok(());
        }
        self.apply(|state| {
            state.join_tolerance = tolerance;
            Ok(("Join tolerance".to_owned(), ()))
        })
    }

    /// Adds a suppressed placeholder at the marker for an item an import
    /// did not bring in because it was suppressed or failed in the file:
    /// named `name` when that name is free (else `Placeholder<n>`), in
    /// `component` (the active one when None), with `note` (the file's
    /// state) as its import note.
    pub fn add_import_placeholder(
        &mut self,
        name: Option<&str>,
        item_type: &str,
        note: &str,
        component: Option<ComponentUid>,
    ) -> Result<Added, ModelError> {
        let component = component.unwrap_or(self.state.assembly.active);
        self.state.editable(component)?;
        let note = note.trim();
        if note.is_empty() {
            return Err(invalid("a placeholder needs a note"));
        }
        self.apply(|state| {
            let name = name
                .map(str::trim)
                .filter(|n| !n.is_empty() && state.check_feature_name(n, None).is_ok());
            let def: FeatureDef<ValueInput> =
                FeatureDef::Placeholder(PlaceholderDef::new(item_type));
            let added = state.add_feature_in(&def, name, component)?;
            let position = state.require(added.uid)?;
            Arc::make_mut(&mut state.features[position]).suppressed = true;
            state.import_notes.insert(added.uid, note.to_owned());
            Ok((format!("Add {}", added.name), added))
        })
    }
}

/// A feature's status as the queries give it: `warning` for one that
/// succeeded with warnings or with an import note.
pub fn status_text(
    status: Option<&FeatureStatus>,
    warnings: &[String],
    note: Option<&str>,
) -> &'static str {
    match status {
        Some(FeatureStatus::Ok) if note.is_some() => "warning",
        _ => super::status_text(status, warnings),
    }
}

/// A failed feature's error, or its import note and warnings joined.
pub fn status_message(
    status: Option<&FeatureStatus>,
    warnings: &[String],
    note: Option<&str>,
) -> Option<String> {
    match (note, super::status_message(status, warnings)) {
        (_, Some(error)) if status.and_then(FeatureStatus::error).is_some() => Some(error),
        (Some(note), Some(warnings)) => Some(format!("{note}; {warnings}")),
        (Some(note), None) => Some(note.to_owned()),
        (None, message) => message,
    }
}
