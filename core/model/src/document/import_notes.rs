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
pub(crate) fn status_text(
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
pub(crate) fn status_message(
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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::Document;
    use crate::document_tests::block;
    use crate::features::placeholder::BUILDS_NOTHING;
    use crate::testing::MockKernel;

    const STANDS_IN: &str = "Stands in for Extrude5 (its result is not in the file's history)";

    fn item(doc: &Document<MockKernel>, name: &str) -> Value {
        let timeline: Value =
            serde_json::from_str(&doc.query(r#"{"query": "timeline"}"#).unwrap()).unwrap();
        timeline["features"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("no {name} in {timeline}"))
    }

    #[test]
    fn an_import_note_is_a_warning_with_its_message() {
        let mut b = block();
        b.doc
            .set_import_notes(&[(b.extrude, STANDS_IN.to_owned())])
            .unwrap();
        let extrude = item(&b.doc, "Extrude1");
        assert_eq!(extrude["status"], "warning");
        assert_eq!(extrude["error"], STANDS_IN);
        assert_eq!(extrude["import"], STANDS_IN);
        assert_eq!(item(&b.doc, "Sketch1")["status"], "ok");
        assert!(item(&b.doc, "Sketch1").get("import").is_none());
        let feature: Value = serde_json::from_str(
            &b.doc
                .query(&json!({"query": "feature", "uid": b.extrude}).to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(feature["status"], "warning");
        assert_eq!(feature["import"], STANDS_IN);
        // Scripts see it as the timeline does.
        let script = json!([{"expect": {"feature": "Extrude1", "status": "warning",
                                        "error_contains": "Stands in for Extrude5"}}]);
        b.doc.run_script(&script.to_string()).unwrap();
        // An undo step; an empty note removes it.
        b.doc.undo();
        assert_eq!(item(&b.doc, "Extrude1")["status"], "ok");
        b.doc.redo();
        b.doc
            .set_import_notes(&[(b.extrude, String::new())])
            .unwrap();
        assert_eq!(item(&b.doc, "Extrude1")["status"], "ok");
        assert!(
            b.doc
                .set_import_notes(&[("F99".parse().unwrap(), "x".to_owned())])
                .is_err()
        );
    }

    #[test]
    fn a_placeholder_is_suppressed_and_builds_nothing() {
        let mut b = block();
        let added = b
            .doc
            .add_import_placeholder(
                Some("Extrude2"),
                "ExtrudeFeature",
                "Suppressed in the file",
                None,
            )
            .unwrap();
        assert_eq!(added.name, "Extrude2");
        let placeholder = item(&b.doc, "Extrude2");
        assert_eq!(placeholder["type"], "placeholder");
        assert_eq!(placeholder["suppressed"], true);
        assert_eq!(placeholder["status"], "suppressed");
        assert_eq!(placeholder["error"], "Suppressed in the file");
        assert_eq!(b.doc.bodies().len(), 1);
        // Unsuppressed it still builds nothing, and says so.
        b.doc.set_suppressed(added.uid, false).unwrap();
        let placeholder = item(&b.doc, "Extrude2");
        assert_eq!(placeholder["status"], "warning");
        assert_eq!(
            placeholder["error"],
            format!("Suppressed in the file; {BUILDS_NOTHING}")
        );
        assert_eq!(b.doc.bodies().len(), 1);
        // A name that is taken gives the default one (the second
        // placeholder's).
        let other = b
            .doc
            .add_import_placeholder(Some("Extrude1"), "", "Failed in the file", None)
            .unwrap();
        assert_eq!(other.name, "Placeholder2");
    }

    #[test]
    fn import_notes_survive_save_and_reopen() {
        let mut b = block();
        b.doc
            .set_import_notes(&[(b.extrude, STANDS_IN.to_owned())])
            .unwrap();
        b.doc
            .add_import_placeholder(
                Some("Fillet2"),
                "FilletFeature",
                "Suppressed in the file",
                None,
            )
            .unwrap();
        let text = b.doc.to_json();
        let saved: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(saved["features"][1]["import_note"], STANDS_IN);
        assert_eq!(saved["features"][2]["type"], "placeholder");
        assert!(saved["features"][0].get("import_note").is_none());
        let mut reopened = Document::from_json(&text, MockKernel::default()).unwrap();
        reopened.recompute();
        assert_eq!(item(&reopened, "Extrude1")["status"], "warning");
        assert_eq!(item(&reopened, "Extrude1")["error"], STANDS_IN);
        assert_eq!(item(&reopened, "Fillet2")["status"], "suppressed");
        assert_eq!(
            item(&reopened, "Fillet2")["error"],
            "Suppressed in the file"
        );
        assert_eq!(reopened.to_json(), text);
    }

    #[test]
    fn an_edit_that_builds_clears_the_note() {
        let mut b = block();
        b.doc
            .set_import_notes(&[(b.extrude, STANDS_IN.to_owned())])
            .unwrap();
        // Renaming is no edit of the definition.
        b.doc.rename_feature(b.extrude, "Pad").unwrap();
        assert_eq!(item(&b.doc, "Pad")["status"], "warning");
        // A dimension that makes it fail keeps the note under the error.
        b.doc.set_parameter("d3", 0.0).unwrap();
        assert_eq!(item(&b.doc, "Pad")["status"], "error");
        assert_eq!(b.doc.import_note(b.extrude), Some(STANDS_IN));
        // One that builds clears it; undo brings it back.
        b.doc.set_parameter("d3", 25.0).unwrap();
        assert_eq!(item(&b.doc, "Pad")["status"], "ok");
        assert_eq!(b.doc.import_note(b.extrude), None);
        b.doc.undo();
        assert_eq!(b.doc.import_note(b.extrude), Some(STANDS_IN));
        // An edit of the definition that builds clears it too.
        let def = json!({"type": "extrude",
            "profiles": [{"sketch": b.sketch, "region": b.region}],
            "extent": {"type": "distance", "distance": 30}, "operation": "new_body"});
        b.doc
            .command(&json!({"cmd": "edit_feature", "uid": b.extrude, "def": def}).to_string())
            .unwrap();
        assert_eq!(item(&b.doc, "Pad")["status"], "ok");
        assert_eq!(b.doc.import_note(b.extrude), None);
        // Deleting the feature drops its note.
        b.doc.undo();
        b.doc.delete_feature(b.extrude, false).unwrap();
        assert_eq!(b.doc.import_notes().count(), 0);
    }
}
