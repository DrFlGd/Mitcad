// SPDX-License-Identifier: MIT
//! Bodies the file cut and pasted into another component.
//!
//! A cut and paste moves bodies from one component into another one of the
//! design (an existing one, placed by the file's occurrences), and changes
//! nothing else: the bodies keep their shape and their place in the design.
//! Mitcad's Move to Component with an existing destination does the same:
//! the bodies and the features that made them go into the destination, and
//! the bodies keep their identity (`move_features.rs` of the model). The
//! item has no state of its own to be checked against, so the replay's
//! bodies must be as they were and be the destination's after the move.

use mitcad_f3d::design::ir::TimelineItem;
use mitcad_model::BodyUid;

use crate::history::bodies_distance;

impl<K: crate::ImportKernel> crate::Importer<'_, K> {
    /// A cut and paste as the Move to Component command its translation
    /// gives ([`Importer::cut_paste_command`](crate::ops)). False (nothing
    /// done) when the move is refused or changes the bodies: the item then
    /// takes the usual route.
    pub(crate) fn cut_paste_item(&mut self, at: usize, index: i64, item: &TimelineItem) -> bool {
        let name = crate::item_name(item);
        let not = |why: &str| {
            if crate::tracing() {
                eprintln!("import: {name}: not moved: {why}");
            }
            false
        };
        if !self.pending.is_empty() {
            return not("items before it are pending");
        }
        let candidates = match self.translate(index, item) {
            Ok(c) => c,
            Err(e) => return not(&e),
        };
        let before = self.current_sigs();
        let mut last = "nothing to try".to_owned();
        for candidate in candidates {
            let Some(command) = candidate.defs.first().filter(|d| d.get("cmd").is_some()) else {
                continue;
            };
            let moved: Vec<BodyUid> = command["bodies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|b| b.as_str()?.parse().ok())
                .collect();
            let into = self.component;
            // Nothing to move: the bodies are the destination's already.
            if command["cmd"] == "none" {
                if moved
                    .iter()
                    .any(|b| self.doc.body_component(*b) != Some(into))
                {
                    last = "the bodies are not the destination's".to_owned();
                    continue;
                }
                self.accepted(at, index, &[], &candidate, None);
                return true;
            }
            let depth = self.doc.undo_depth();
            let text = command.to_string();
            let result = self.interruptible(|doc| doc.command(&text));
            let features = match result {
                Ok(done) => serde_json::from_str::<serde_json::Value>(&done)
                    .ok()
                    .and_then(|v| v["components"][0]["features"].as_array().map(Vec::len))
                    .unwrap_or(0),
                Err(e) => {
                    last = e.to_string();
                    self.undo_to(depth);
                    continue;
                }
            };
            // Nothing but the bodies' component changed.
            if bodies_distance(&before, &self.current_sigs()).is_none() {
                last = "the bodies changed".to_owned();
                self.undo_to(depth);
                continue;
            }
            if moved
                .iter()
                .any(|b| self.doc.body_component(*b) != Some(into))
            {
                last = "the bodies are not the destination's".to_owned();
                self.undo_to(depth);
                continue;
            }
            let note = format!(
                "moves {} into {} with {features} features that made {}",
                if moved.len() == 1 {
                    "its body".to_owned()
                } else {
                    format!("its {} bodies", moved.len())
                },
                self.doc.assembly().name(into),
                if moved.len() == 1 { "it" } else { "them" },
            );
            let mut accepted = candidate.clone();
            accepted.note = Some(note);
            self.accepted(at, index, &[], &accepted, None);
            return true;
        }
        not(&last)
    }
}
