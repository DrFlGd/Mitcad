// SPDX-License-Identifier: MIT
//! Features added many at once (the IFC import's, mitcad#273): a building
//! model makes thousands of features, a few per element, and a command per
//! feature would recompute and copy the state each time. A batch adds an
//! element's features, names its bodies and gives them their appearance in
//! one command: one undo step and one recompute.
//!
//! Definitions may refer to features added before them in the same batch
//! (an extrusion to its sketch, a cut to the bodies of the extrusion), as
//! [`FeatureBatch::add`] returns each feature's uid at once; profile
//! references are not checked against the sketches (they are not
//! evaluated yet), so a profile a sketch does not have fails its feature
//! when the batch recomputes, as any failing feature does. The caller
//! reads the features' statuses afterwards.
//!
//! A batch appended at the end of the timeline goes through only its own
//! features and names only its own bodies (mitcad#841): the features
//! before it keep the results the document has (see
//! `recompute::recompute_after`), so an import of thousands of elements in
//! batches does not go through the whole timeline for each batch.

use super::{Added, DocState, Document, ModelError, invalid};
use crate::features::{FeatureDef, ValueInput};
use crate::ids::{BodyUid, ComponentUid, FeatureUid};
use crate::kernel::Kernel;

/// What a batch adds, inside its command (see the module docs).
pub struct FeatureBatch<'a> {
    state: &'a mut DocState,
    added: Vec<FeatureUid>,
}

impl FeatureBatch<'_> {
    /// Adds a feature at the timeline marker, in `component`.
    pub fn add(
        &mut self,
        def: &FeatureDef<ValueInput>,
        name: Option<&str>,
        component: ComponentUid,
    ) -> Result<Added, ModelError> {
        self.state.editable(component)?;
        let name = name.filter(|n| !n.trim().is_empty());
        let added = match self.state.add_feature_in(def, name, component) {
            Err(e) if name.is_some() && e.to_string().contains("already named") => {
                self.state.add_feature_in(def, None, component)?
            }
            other => other?,
        };
        self.added.push(added.uid);
        Ok(added)
    }

    /// Names a body of a feature of the batch (the caller keeps names
    /// unique).
    pub fn name_body(&mut self, body: BodyUid, name: &str) -> Result<(), ModelError> {
        if !self.added.contains(&body.feature) {
            return Err(invalid(format!(
                "{body} is not a body of a feature of the batch"
            )));
        }
        let name = name.trim();
        if name.is_empty() {
            return Err(invalid("the body name is empty"));
        }
        self.state.body_names.insert(body, name.to_owned());
        Ok(())
    }

    /// Gives a body of a feature of the batch an appearance of the document
    /// or the library.
    pub fn set_body_appearance(
        &mut self,
        body: BodyUid,
        appearance: &str,
    ) -> Result<(), ModelError> {
        if !self.added.contains(&body.feature) {
            return Err(invalid(format!(
                "{body} is not a body of a feature of the batch"
            )));
        }
        if appearance.trim().is_empty() {
            return Err(invalid("the appearance id is empty"));
        }
        let attributes = self.state.body_attributes.entry(body).or_default();
        attributes.appearance = Some(appearance.to_owned());
        Ok(())
    }

    /// The features added so far.
    pub fn added(&self) -> &[FeatureUid] {
        &self.added
    }
}

impl<K: Kernel> Document<K> {
    /// Adds features in one command named `label` (see the module docs):
    /// `build` adds them through the batch; an error from it rejects the
    /// whole batch and leaves the document as it was.
    pub fn add_features_with<T>(
        &mut self,
        label: &str,
        build: impl FnOnce(&mut FeatureBatch<'_>) -> Result<T, ModelError>,
    ) -> Result<T, ModelError> {
        let label = label.to_owned();
        self.apply(|state| {
            let mut batch = FeatureBatch {
                state,
                added: Vec::new(),
            };
            let value = build(&mut batch)?;
            Ok((label, value))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::FeatureBatch;
    use crate::Document;
    use crate::ModelError;
    use crate::features::FeatureDef;
    use crate::ids::{BodyUid, ComponentUid};
    use crate::testing::MockKernel;

    #[test]
    fn a_batch_is_one_command() {
        let mut doc = Document::new(MockKernel::default());
        let sketch: FeatureDef<_> = serde_json::from_value(serde_json::json!({
            "type": "sketch", "plane": "xy",
            "entities": [
                {"id": "p1", "type": "point", "at": [0, 0]},
                {"id": "c2", "type": "circle", "center": "p1", "radius": 5}]
        }))
        .expect("a sketch");
        let made = doc
            .add_features_with("Import Wall", |batch| {
                let s = batch.add(&sketch, Some("Wall sketch"), ComponentUid::ROOT)?;
                let extrude: FeatureDef<_> = serde_json::from_value(serde_json::json!({
                    "type": "extrude",
                    "profiles": [{"sketch": s.uid.to_string(), "region": "r{c2}"}],
                    "extent": {"type": "distance", "distance": 10},
                    "operation": "new_body"
                }))
                .expect("an extrusion");
                let e = batch.add(&extrude, Some("Wall"), ComponentUid::ROOT)?;
                let body = BodyUid::new(e.uid, 0);
                batch.name_body(body, "Wall 1")?;
                batch.set_body_appearance(body, "paint_red")?;
                Ok(e.uid)
            })
            .expect("added");
        assert_eq!(doc.undo_depth(), 1);
        assert_eq!(doc.undo_label(), Some("Import Wall"));
        let bodies = doc.bodies();
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0].uid, BodyUid::new(made, 0));
        assert_eq!(bodies[0].name, "Wall 1");
        assert_eq!(
            doc.body_attributes(bodies[0].uid).appearance.as_deref(),
            Some("paint_red")
        );
        // An error rejects the whole batch.
        let failed = doc.add_features_with("Nothing", |batch| {
            batch.add(&sketch, None, ComponentUid::ROOT)?;
            batch.name_body(BodyUid::new(made, 0), "Taken")?;
            Ok(())
        });
        assert!(failed.is_err());
        assert_eq!(doc.undo_depth(), 1);
    }

    /// A wall as an importer adds it: a sketch, its extrusion, the body
    /// named after the wall.
    fn add_wall(batch: &mut FeatureBatch<'_>, k: usize) -> Result<(), ModelError> {
        let sketch = crate::test_support::def(serde_json::json!({
            "type": "sketch", "plane": "xy",
            "entities": [
                {"id": "p1", "type": "point", "at": [10 * k, 0]},
                {"id": "c2", "type": "circle", "center": "p1", "radius": 2 + k}]
        }));
        let s = batch.add(
            &sketch,
            Some(format!("Wall {k} sketch").as_str()),
            ComponentUid::ROOT,
        )?;
        let extrude = crate::test_support::def(serde_json::json!({
            "type": "extrude",
            "profiles": [{"sketch": s.uid.to_string(), "region": "r{c2}"}],
            "extent": {"type": "distance", "distance": 10 + k},
            "operation": "new_body"
        }));
        let e = batch.add(
            &extrude,
            Some(format!("Wall {k}").as_str()),
            ComponentUid::ROOT,
        )?;
        batch.name_body(BodyUid::new(e.uid, 0), &format!("Wall {k}"))?;
        Ok(())
    }

    /// Walls `walls` in one batch.
    fn add_walls(doc: &mut Document<MockKernel>, walls: std::ops::Range<usize>) {
        doc.add_features_with(&format!("Import {walls:?}"), |batch| {
            for k in walls.clone() {
                add_wall(batch, k)?;
            }
            Ok(())
        })
        .expect("walls added");
    }

    /// What a document holds and computed: its features, parameters,
    /// body names and attributes, each feature's status and the versions
    /// of the results and bodies.
    fn same_design(a: &Document<MockKernel>, b: &Document<MockKernel>) {
        let features = |d: &Document<MockKernel>| d.features().cloned().collect::<Vec<_>>();
        assert_eq!(features(a), features(b));
        assert!(a.parameters() == b.parameters());
        assert_eq!(a.state().body_names, b.state().body_names);
        assert_eq!(a.state().body_attributes, b.state().body_attributes);
        let statuses = |d: &Document<MockKernel>| {
            d.features()
                .map(|f| (f.uid, d.status(f.uid).cloned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(statuses(a), statuses(b));
        assert_eq!(a.versions(), b.versions());
        let bodies = |d: &Document<MockKernel>| {
            d.bodies()
                .iter()
                .map(|b| (b.uid, b.name.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(bodies(a), bodies(b));
    }

    // mitcad#841: a batch appended at the end goes through only its own
    // features, and the design is the one a single batch makes.
    #[test]
    fn batches_make_the_design_one_batch_makes() {
        let mut batched = Document::new(MockKernel::default());
        for first in [0, 3, 6] {
            add_walls(&mut batched, first..first + 3);
        }
        let lookups = batched.cache().stats.lookups;
        let recomputes = batched.recompute_count();
        add_walls(&mut batched, 9..12);
        // The last batch looked up its six features, not the eighteen
        // before them.
        assert_eq!(batched.cache().stats.lookups - lookups, 6);
        assert_eq!(batched.recompute_count(), recomputes + 1);
        assert_eq!(batched.stats().evaluated.len(), 6);
        assert_eq!(batched.undo_depth(), 4);
        assert_eq!(batched.bodies().len(), 12);

        let mut whole = Document::new(MockKernel::default());
        add_walls(&mut whole, 0..12);
        assert_eq!(whole.undo_depth(), 1);
        same_design(&batched, &whole);

        // A recompute from the start finds the same.
        let mut again = Document::new(MockKernel::default());
        for first in [0, 3, 6, 9] {
            add_walls(&mut again, first..first + 3);
        }
        again.recompute();
        same_design(&again, &whole);

        // Undo takes the batches back one by one, and redo brings them back.
        assert_eq!(batched.undo().as_deref(), Some("Import 9..12"));
        assert_eq!(batched.bodies().len(), 9);
        let mut nine = Document::new(MockKernel::default());
        add_walls(&mut nine, 0..9);
        same_design(&batched, &nine);
        batched.redo();
        same_design(&batched, &whole);
    }

    // A batch after a change of what earlier features read goes through
    // the timeline from the start, and finds the same as a document made
    // with the change from the start.
    #[test]
    fn a_batch_after_a_change_recomputes_what_changed() {
        let mut doc = Document::new(MockKernel::default());
        add_walls(&mut doc, 0..3);
        // The first wall's height, a parameter it owns.
        let wall = doc
            .features()
            .find(|f| f.name == "Wall 0")
            .map(|f| f.uid)
            .expect("the first wall");
        let height = doc
            .parameters()
            .iter()
            .find(|p| doc.parameters().owner(p.id()) == Some(wall))
            .map(|p| p.name().to_owned())
            .expect("its height");
        doc.set_parameter(&height, 25.0).expect("set");
        let before = doc.versions();
        let lookups = doc.cache().stats.lookups;
        add_walls(&mut doc, 3..5);
        // (The parameter's change recomputed already: the batch went
        // through its own features only.)
        assert_eq!(doc.cache().stats.lookups - lookups, 4);
        assert_eq!(doc.versions().0[..6], before.0[..]);
        let mut fresh = Document::new(MockKernel::default());
        add_walls(&mut fresh, 0..3);
        fresh.set_parameter(&height, 25.0).expect("set");
        add_walls(&mut fresh, 3..5);
        same_design(&doc, &fresh);
        // A rename of an earlier wall changes no result: the batch after
        // it goes through its own features too.
        doc.rename_feature(wall, "First wall").expect("renamed");
        let lookups = doc.cache().stats.lookups;
        add_walls(&mut doc, 5..6);
        assert_eq!(doc.cache().stats.lookups - lookups, 2);
        let mut renamed = Document::new(MockKernel::default());
        add_walls(&mut renamed, 0..3);
        renamed.set_parameter(&height, 25.0).expect("set");
        add_walls(&mut renamed, 3..5);
        renamed.rename_feature(wall, "First wall").expect("renamed");
        renamed.recompute();
        add_walls(&mut renamed, 5..6);
        same_design(&doc, &renamed);
    }
}
