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
    use crate::Document;
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
}
