// SPDX-License-Identifier: MIT
//! What the timeline shows of a FreeCAD import's outcomes (mitcad#170): the
//! features it made get import notes from its report.

use std::collections::BTreeMap;

use mitcad_import_base::notes::{partly, push, stands_in_for};
use mitcad_model::FeatureUid;

/// The notes of the features a FreeCAD import made, from its report: a
/// fallback's base feature stands in for its object, with the report's
/// notes as the reason; a sketch or feature that came in partly says what
/// was left out. (Placeholders of suppressed features get theirs when they
/// are added.)
pub(crate) fn freecad_notes(report: &crate::FcstdReport) -> Vec<(FeatureUid, String)> {
    use crate::ObjectOutcome;
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
