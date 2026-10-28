// SPDX-License-Identifier: MIT
//! Features found by uid without a search through the timeline
//! (mitcad#841): checks, evaluations and the design file look up the
//! features a definition names once per feature, and a search per lookup
//! made a large design's import and load quadratic in its features.
//!
//! The index keeps each feature's position in a timeline (a slice of
//! entries in timeline order). It is not kept in step with the timeline:
//! every position it gives is checked against the slice it is asked
//! about, features added after the last lookup are indexed when a uid is
//! not found, and the index is made anew when a feature it has is no
//! longer where it was (a move, a delete). A uid the timeline does not
//! have is searched for as before, so the answers are those of a search
//! from the start of the timeline whatever the index holds (uids are
//! unique in a timeline).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::features::FeatureEntry;
use crate::ids::FeatureUid;

#[derive(Default, Clone)]
struct Positions {
    positions: HashMap<FeatureUid, usize>,
    /// The number of entries of the timeline indexed from its start.
    indexed: usize,
}

/// Positions of features in a timeline by uid (see the module docs). A
/// copy takes what the index holds; an index compares equal to any other.
#[derive(Default)]
pub(crate) struct FeatureIndex(Mutex<Positions>);

impl FeatureIndex {
    fn lock(&self) -> MutexGuard<'_, Positions> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The position of the feature `uid` in `features`, as
    /// `features.iter().position(|f| f.uid == uid)` finds it.
    pub(crate) fn position(
        &self,
        features: &[Arc<FeatureEntry>],
        uid: FeatureUid,
    ) -> Option<usize> {
        let at = |i: usize| features.get(i).is_some_and(|f| f.uid == uid);
        let mut index = self.lock();
        if let Some(&i) = index.positions.get(&uid)
            && at(i)
        {
            return Some(i);
        }
        // Features added after the index was made.
        if index.indexed < features.len() {
            for (i, f) in features.iter().enumerate().skip(index.indexed) {
                index.positions.insert(f.uid, i);
            }
            index.indexed = features.len();
            if let Some(&i) = index.positions.get(&uid)
                && at(i)
            {
                return Some(i);
            }
        }
        // Moved or removed features, or a uid the timeline does not have.
        let found = features.iter().position(|f| f.uid == uid)?;
        let mut positions = HashMap::with_capacity(features.len());
        for (i, f) in features.iter().enumerate() {
            positions.entry(f.uid).or_insert(i);
        }
        *index = Positions {
            positions,
            indexed: features.len(),
        };
        Some(found)
    }

    /// The feature `uid` of `features`.
    pub(crate) fn find<'f>(
        &self,
        features: &'f [Arc<FeatureEntry>],
        uid: FeatureUid,
    ) -> Option<&'f Arc<FeatureEntry>> {
        self.position(features, uid).map(|i| &features[i])
    }

    /// Forgets the positions (a state kept for undo need not hold them;
    /// they are found again when it comes back).
    pub(crate) fn clear(&mut self) {
        *self.0.get_mut().unwrap_or_else(|e| e.into_inner()) = Positions::default();
    }
}

impl Clone for FeatureIndex {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.lock().clone()))
    }
}

impl PartialEq for FeatureIndex {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for FeatureIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FeatureIndex")
    }
}

#[cfg(test)]
mod tests {
    use super::FeatureIndex;
    use crate::Document;
    use crate::features::FeatureEntry;
    use crate::ids::FeatureUid;
    use crate::testing::MockKernel;
    use std::sync::Arc;

    /// Nine sketches of a document.
    fn entries() -> Vec<Arc<FeatureEntry>> {
        let mut doc = Document::new(MockKernel::default());
        let sketch = crate::test_support::def(serde_json::json!({
            "type": "sketch", "plane": "xy",
            "entities": [
                {"id": "p1", "type": "point", "at": [0, 0]},
                {"id": "c2", "type": "circle", "center": "p1", "radius": 5}]
        }));
        for _ in 0..9 {
            doc.add_feature(&sketch, None).expect("a sketch");
        }
        doc.state().features.clone()
    }

    /// What a search from the start finds, for every uid of `features`
    /// and a few it does not have.
    fn check(index: &FeatureIndex, features: &[Arc<FeatureEntry>]) {
        for uid in (0..16).map(FeatureUid) {
            assert_eq!(
                index.position(features, uid),
                features.iter().position(|f| f.uid == uid),
                "{uid}"
            );
        }
    }

    #[test]
    fn positions_follow_appends_deletes_and_moves() {
        let index = FeatureIndex::default();
        let all = entries();
        let mut features: Vec<_> = all[..5].to_vec();
        check(&index, &features);
        // Appended one by one, each looked up at once (a batch's features).
        for added in &all[5..8] {
            features.push(added.clone());
            assert_eq!(
                index.position(&features, added.uid),
                Some(features.len() - 1)
            );
        }
        check(&index, &features);
        // Deleted from the middle: the features after it moved up.
        features.remove(2);
        check(&index, &features);
        // Moved: the last to the front.
        let last = features.pop().expect("features");
        features.insert(0, last);
        check(&index, &features);
        // Deleted and one added in its place: the same length.
        features.remove(1);
        features.push(all[8].clone());
        check(&index, &features);
        // A copy keeps its positions, and both follow their own timelines.
        let copy = index.clone();
        let mut other = features.clone();
        other.truncate(3);
        check(&copy, &other);
        check(&index, &features);
        assert!(copy == index);
    }

    #[test]
    fn a_cleared_index_finds_the_features_again() {
        let mut index = FeatureIndex::default();
        let features: Vec<_> = entries()[..4].to_vec();
        check(&index, &features);
        index.clear();
        check(&index, &features);
        let third = features[2].uid;
        assert_eq!(index.find(&features, third).map(|f| f.uid), Some(third));
        assert!(index.find(&features, FeatureUid(42)).is_none());
    }

    /// Every feature of the document found where a search finds it, with
    /// its last result.
    fn check_document(doc: &Document<MockKernel>) {
        let features = &doc.state().features;
        for (i, entry) in features.iter().enumerate() {
            assert_eq!(doc.state().position(entry.uid), Some(i));
            assert_eq!(doc.feature(entry.uid).map(|f| f.uid), Some(entry.uid));
            assert_eq!(
                doc.status(entry.uid),
                doc.last_result().result(entry.uid).map(|r| &r.status)
            );
        }
        assert!(doc.feature(FeatureUid(999)).is_none());
        assert!(doc.status(FeatureUid(999)).is_none());
    }

    fn circle(radius: f64) -> crate::features::FeatureDef<crate::ValueInput> {
        crate::test_support::def(serde_json::json!({
            "type": "sketch", "plane": "xy",
            "entities": [
                {"id": "p1", "type": "point", "at": [0, 0]},
                {"id": "c2", "type": "circle", "center": "p1", "radius": radius}]
        }))
    }

    fn extrude(sketch: FeatureUid) -> crate::features::FeatureDef<crate::ValueInput> {
        crate::test_support::def(serde_json::json!({
            "type": "extrude",
            "profiles": [{"sketch": sketch.to_string(), "region": "r{c2}"}],
            "extent": {"type": "distance", "distance": 10},
            "operation": "new_body"
        }))
    }

    // mitcad#841: the document's lookups by uid after edits, deletes and
    // moves, and the checks of references that use them.
    #[test]
    fn lookups_follow_edits_deletes_and_moves() {
        let mut doc = Document::new(MockKernel::default());
        let s1 = doc.add_feature(&circle(2.0), None).expect("s1").uid;
        let s2 = doc.add_feature(&circle(3.0), None).expect("s2").uid;
        let s3 = doc.add_feature(&circle(4.0), None).expect("s3").uid;
        let e1 = doc.add_feature(&extrude(s1), None).expect("e1").uid;
        let e2 = doc.add_feature(&extrude(s2), None).expect("e2").uid;
        check_document(&doc);
        // Moved to the front.
        doc.move_feature(s3, 0).expect("moved");
        assert_eq!(doc.state().position(s3), Some(0));
        check_document(&doc);
        // A feature that uses a sketch cannot come before it.
        let early = doc.move_feature(e2, 1).expect_err("before its sketch");
        assert!(early.to_string().contains("cannot move there"), "{early}");
        check_document(&doc);
        // Deleted: gone from the lookups, and a reference to it is refused.
        doc.delete_feature(e1, false).expect("deleted");
        assert!(doc.feature(e1).is_none());
        check_document(&doc);
        doc.delete_feature(s1, false).expect("deleted");
        let missing = doc.add_feature(&extrude(s1), None).expect_err("s1 is gone");
        assert!(missing.to_string().contains("does not exist"), "{missing}");
        check_document(&doc);
        // Edited: the features after it are checked against it again.
        doc.edit_feature(s2, &circle(5.0)).expect("edited");
        check_document(&doc);
        // Undo brings the deleted features back where they were.
        doc.undo();
        doc.undo();
        doc.undo();
        assert_eq!(doc.state().position(e1), Some(3));
        check_document(&doc);
        // A new feature found at once, also by the checks of the next.
        let s4 = doc.add_feature(&circle(6.0), None).expect("s4").uid;
        let e4 = doc.add_feature(&extrude(s4), None).expect("e4").uid;
        assert_eq!(
            doc.state().position(e4),
            Some(doc.state().features.len() - 1)
        );
        check_document(&doc);
    }
}
