// SPDX-License-Identifier: MIT
//! Bodies the file took out of the design (a removal item).
//!
//! A removal has no history state of its own and the history keeps the
//! removed body as it was, so it cannot be checked against a state as other
//! items are. It names its body: the replay's body that is the stored body
//! of that name goes, and the stored body leaves the history's states from
//! the replay's state on ([`Oracle::retire`]), as the tools a combine
//! consumed do (`producers.rs`).

use mitcad_dump::TimelineItem;

use crate::history::{Sig, bodies_distance};

impl<K: crate::ImportKernel> crate::Importer<'_, K> {
    /// A removal as a `remove_body` feature. The stored counterpart of the
    /// removed body is the stored body of the state the replay stands for
    /// that has its measures; the bodies left must be that state's others.
    /// False (nothing done) when the history does not settle it: the item
    /// then takes the usual route for items without a state.
    pub(crate) fn remove_body_item(&mut self, at: usize, index: i64, item: &TimelineItem) -> bool {
        let name = crate::item_name(item);
        let not = |why: &str| {
            if crate::tracing() {
                eprintln!("import: {name}: not matched to the state: {why}");
            }
            false
        };
        if !self.pending.is_empty() {
            return not("items before it are pending");
        }
        let Some(q) = self.oracle.cursor else {
            return not("the replay stands for no state");
        };
        let kernel = self.doc.kernel();
        let Ok(stored) = self.oracle.state(kernel, q).map(<[_]>::to_vec) else {
            return not("the state was not rebuilt");
        };
        let stored_sigs: Vec<Sig> = stored.iter().map(|(_, s)| *s).collect();
        let current = self.current_bodies();
        let current_sigs: Vec<Sig> = current.iter().map(|(_, s)| *s).collect();
        if bodies_distance(&stored_sigs, &current_sigs).is_none() {
            return not("the bodies are not the state's");
        }
        let candidate = match self.translate(index, item) {
            Ok(mut c) if !c.is_empty() => c.remove(0),
            Ok(_) => return not("nothing to try"),
            Err(e) => return not(&e),
        };
        let removed = candidate.defs[0]["bodies"][0].as_str().unwrap_or_default();
        if crate::tracing() {
            eprintln!(
                "import: {name}: state {q} has {:?}; the replay has {:?}; removes {removed}",
                stored_sigs
                    .iter()
                    .map(|s| (s.volume, s.faces))
                    .collect::<Vec<_>>(),
                current_sigs
                    .iter()
                    .map(|s| (s.volume, s.faces))
                    .collect::<Vec<_>>()
            );
        }
        let Some(sig) = current
            .iter()
            .find(|(uid, _)| uid.to_string() == removed)
            .map(|(_, s)| *s)
        else {
            return not("the removed body is not among the replay's");
        };
        // The stored body with the same faces and measures, else the
        // nearest within the tolerance (a replayed split piece can have its
        // faces split otherwise than the file's).
        let nearest = |near: &dyn Fn(&Sig) -> bool| {
            stored
                .iter()
                .filter(|(b, s)| b.id.is_some() && near(s))
                .min_by(|(_, a), (_, b)| a.distance(&sig).total_cmp(&b.distance(&sig)))
                .and_then(|(b, _)| b.id)
        };
        let Some(id) = nearest(&|s| s.same(&sig)).or_else(|| nearest(&|s| s.matches(&sig))) else {
            return not("no stored body is the removed one");
        };
        let depth = self.doc.undo_depth();
        let uids = match self.try_candidate(index, &candidate, item.name()) {
            Ok(uids) => uids,
            Err(e) => {
                self.undo_to(depth);
                return not(&e);
            }
        };
        let expected: Vec<Sig> = stored
            .iter()
            .filter(|(b, _)| b.id != Some(id))
            .map(|(_, s)| *s)
            .collect();
        if bodies_distance(&expected, &self.current_sigs()).is_none() {
            self.undo_to(depth);
            return not("the bodies left are not the state's others");
        }
        self.oracle.retire(q, &[id]);
        self.report.history.matched += 1;
        self.accepted(at, index, &uids, &candidate, Some(true));
        true
    }
}
