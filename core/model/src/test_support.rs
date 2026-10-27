// SPDX-License-Identifier: MIT
//! Fixtures the model's tests share (mitcad#777): values and feature
//! definitions, the names of an extruded rectangle's faces and edges, the
//! block document most tests start from, and a policy that lets every
//! owner attach to every other. Public under the `testing` feature, so that
//! test crates above the model use them too.

use serde_json::json;

use crate::features::{FeatureDef, SketchPlane, ValueInput};
use crate::ids::{BodyUid, ComponentUid, EntityUid, FeatureUid};
use crate::policy::Policy;
use crate::testing::MockKernel;
use crate::topo::{EdgeName, FaceName, RegionKey, SegmentKey};
use crate::{Document, FeatureStatus};

pub fn num(value: f64) -> ValueInput {
    ValueInput::Number(value)
}

pub fn def(value: serde_json::Value) -> FeatureDef<ValueInput> {
    serde_json::from_value(value).unwrap()
}

pub fn extrude(
    sketch: FeatureUid,
    regions: &[&RegionKey],
    distance: f64,
    operation: &str,
    participants: &[BodyUid],
) -> FeatureDef<ValueInput> {
    let profiles: Vec<_> = regions
        .iter()
        .map(|r| json!({"sketch": sketch, "region": r}))
        .collect();
    def(json!({"type": "extrude", "profiles": profiles,
               "extent": {"type": "distance", "distance": distance},
               "operation": operation, "participants": participants}))
}

/// Line i of a rectangle with lines c<first>..c<first+3>.
fn segment(first: u32, i: usize) -> SegmentKey {
    let c = |k: usize| EntityUid(first + ((k + 4) % 4) as u32);
    SegmentKey::between(c(i), c(i + 3), c(i + 1))
}

pub fn side(feature: FeatureUid, first: u32, i: usize) -> FaceName {
    FaceName::side(feature, segment(first, i))
}

/// The edge along corner i of the extruded rectangle.
pub fn corner(feature: FeatureUid, first: u32, i: usize) -> EdgeName {
    EdgeName::new(side(feature, first, (i + 3) % 4), side(feature, first, i))
}

/// The edge of line i at the far cap.
pub fn top_edge(feature: FeatureUid, first: u32, region: &RegionKey, i: usize) -> EdgeName {
    EdgeName::new(
        side(feature, first, i),
        FaceName::end(feature, region.clone()),
    )
}

pub struct Block {
    pub doc: Document<MockKernel>,
    pub sketch: FeatureUid,
    pub region: RegionKey,
    pub extrude: FeatureUid,
    pub body: BodyUid,
}

/// Sketch1 (F1) with a 60 x 40 rectangle (c1..c4; d1, d2) extruded 20 mm
/// (d3) by Extrude1 (F2) into Body1 (F2.b0).
pub fn block() -> Block {
    let mut doc = Document::new(MockKernel::default());
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(60.0), &num(40.0))
        .unwrap()
        .region;
    let extrude = doc
        .add_feature(&extrude(sketch, &[&region], 20.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    Block {
        doc,
        sketch,
        region,
        extrude,
        body: BodyUid::new(extrude, 0),
    }
}

impl Block {
    pub fn history(&self, body: BodyUid) -> String {
        self.doc
            .body_shape(body)
            .map_or_else(|| "none".to_owned(), |s| s.history.clone())
    }

    pub fn evaluated(&self) -> Vec<FeatureUid> {
        self.doc.stats().evaluated
    }

    pub fn status(&self, uid: FeatureUid) -> FeatureStatus {
        self.doc.status(uid).cloned().expect("recomputed")
    }

    pub fn fillet(&mut self, edges: &[&EdgeName], radius: f64) -> FeatureUid {
        let fillet =
            def(json!({"type": "fillet", "body": self.body, "edges": edges, "radius": radius}));
        self.doc.add_feature(&fillet, None).unwrap().uid
    }

    /// Sketch2 with a 20 x 10 rectangle at (10, 10) (c1..c4), joined 30 mm
    /// into the block by Extrude2.
    pub fn join_boss(&mut self) -> (FeatureUid, RegionKey, FeatureUid) {
        let sketch = self.doc.add_sketch(SketchPlane::Xy).unwrap().uid;
        let region = self
            .doc
            .add_rectangle(sketch, [10.0, 10.0], &num(20.0), &num(10.0))
            .unwrap()
            .region;
        let join = extrude(sketch, &[&region], 30.0, "join", &[self.body]);
        let uid = self.doc.add_feature(&join, None).unwrap().uid;
        (sketch, region, uid)
    }

    /// A sketch with a circle (c1) at (30, 20) and the extrude of it.
    pub fn circle_extrude(&mut self, operation: &str, participants: &[BodyUid]) -> FeatureUid {
        let sketch = self.doc.add_sketch(SketchPlane::Xy).unwrap().uid;
        let region = self
            .doc
            .add_circle(sketch, [30.0, 20.0], &num(10.0))
            .unwrap()
            .region;
        let def = extrude(sketch, &[&region], 30.0, operation, participants);
        self.doc.add_feature(&def, None).unwrap().uid
    }
}

/// Lets every owner's sketches attach to every other owner's faces and
/// planes (fixtures of linked sketches, made before the policy).
pub fn allow_attachments(doc: &mut Document<MockKernel>) {
    let owners: Vec<ComponentUid> = std::iter::once(ComponentUid::ROOT)
        .chain(doc.assembly().components.iter().map(|c| c.uid))
        .collect();
    for owner in owners {
        let policy = Policy {
            attach_to_others: true,
            attachments_from_others: true,
            ..doc.policy(owner)
        };
        doc.set_policy(owner, policy).unwrap();
    }
}
