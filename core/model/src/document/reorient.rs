// SPDX-License-Identifier: MIT
//! Reorient Design (mitcad#502): the whole design turned by a multiple of
//! 90 degrees about an origin axis in one undo step. How each part of the
//! definition turns is in [`crate::reorient`]; this is the command: the
//! next state, the sketch frames that only an evaluation can tell, and the
//! checks that the turned design is the design turned.
//!
//! The command computes the turned design before taking it and refuses it
//! when a body would not be its old self turned (its bounding box) or a
//! feature that built would fail, naming them; the document is then as it
//! was.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{DocState, Document, ModelError, invalid};
use crate::features::sketch::FrameDef;
use crate::features::{FeatureDef, FeatureEntry};
use crate::ids::{BodyUid, ComponentUid, FeatureUid};
use crate::kernel::{BoundingBox, Kernel};
use crate::profile::{SketchFrame, cross, dot};
use crate::recompute::Recomputed;
use crate::reorient::{Turn, Turned};

/// What Reorient Design changed, for the command's reply.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReorientReport {
    /// The features whose definitions changed, by name, in timeline order.
    pub features: Vec<String>,
    /// The Moves added after base features (imported bodies), by name.
    pub moves: Vec<String>,
    /// The occurrences whose placements changed, by name.
    pub occurrences: Vec<String>,
    /// Named views, section analyses and lights that turned.
    pub views: usize,
    pub analyses: usize,
    pub lights: usize,
}

/// A body as the checks compare it.
struct BodyFacts {
    component: ComponentUid,
    bounds: Option<BoundingBox>,
    center: Option<[f64; 3]>,
}

/// A sketch frame compares within this (unit vectors; positions scaled by
/// their size).
const FRAME_TOLERANCE: f64 = 1e-9;

impl<K: Kernel> Document<K> {
    /// Turns the whole design (see the module documentation); one undo
    /// step named `Reorient Design`.
    pub fn reorient_design(&mut self, turn: Turn) -> Result<ReorientReport, ModelError> {
        if turn.is_identity() {
            return Err(invalid(
                "Reorient Design: the turn leaves the design as it is",
            ));
        }
        let all = self.state.features.len();
        // The bodies at the marker as they are, and every sketch's frame
        // (the marker at the end, so that sketches after it are known too).
        let old_boxes = self.boxes_of(&self.result)?;
        let old_ok = ok_features(&self.result);
        let old_frames = if self.state.marker >= all {
            sketch_frames(&self.result)
        } else {
            let mut whole = self.state.clone();
            whole.marker = all;
            sketch_frames(&self.compute(&whole)?)
        };

        let mut next = self.state.clone();
        let turned = turned_components(&next);
        let mut report = ReorientReport::default();
        turn_state(&mut next, turn, &turned, &mut report)?;

        // Sketches on faces, on construction planes and on other
        // components' planes: the frame they get from the turned design,
        // corrected where a rule of the model axes gives the plane another
        // in-plane direction. A corrected sketch can move what is built on
        // it, so the next round looks again.
        let sketches = next
            .features
            .iter()
            .filter(|f| matches!(f.def, FeatureDef::Sketch(_)))
            .count();
        for _ in 0..=sketches {
            let mut whole = next.clone();
            whole.marker = all.max(next.features.len());
            let result = self.compute(&whole)?;
            let fixes = frame_fixes(&next, &result, &old_frames, turn, &turned);
            if fixes.is_empty() {
                break;
            }
            for (uid, frame) in fixes {
                set_sketch_frame(&mut next, uid, frame);
                let name = next.entry(uid).map(|f| f.name.clone()).unwrap_or_default();
                if !report.features.contains(&name) {
                    report.features.push(name);
                }
            }
        }
        sort_by_timeline(&next, &mut report.features);

        // The turned design at the marker: the same bodies turned, and
        // every feature that built still builds.
        let result = self.compute(&next)?;
        let failed: Vec<String> = next
            .features
            .iter()
            .filter(|f| old_ok.contains(&f.uid))
            .filter(|f| result.result(f.uid).is_some_and(|r| !r.status.is_ok()))
            .map(|f| f.name.clone())
            .collect();
        if !failed.is_empty() {
            return Err(invalid(format!(
                "Reorient Design cannot turn the design: {} would fail",
                failed.join(", ")
            )));
        }
        let new_boxes = self.boxes_of(&result)?;
        let mut moved = Vec::new();
        for (body, old) in &old_boxes {
            let q = if turned.contains(&old.component) {
                turn
            } else {
                Turn::IDENTITY
            };
            let expected = old.bounds.map(|b| turn_box(q, &b));
            let now = new_boxes.get(body);
            let centred = match (old.center, now.and_then(|n| n.center)) {
                (Some(was), Some(is)) => {
                    let size = expected.as_ref().map_or(1.0, box_size);
                    let was = q.vector(was);
                    (0..3).all(|i| (was[i] - is[i]).abs() <= 1e-6 * size)
                }
                _ => true,
            };
            if !centred || !same_box(expected.as_ref(), now.and_then(|n| n.bounds).as_ref()) {
                moved.push(self.body_label(&next, *body));
            }
        }
        // Components where their placements take them (joints solve them
        // anew), in the turned coordinates of both sides.
        let context = crate::reorient::Turned {
            turn,
            turned: &turned,
            assembly: &next.assembly,
        };
        for (occurrence, was) in &self.result.placements {
            let Some(o) = next.assembly.occurrence(*occurrence) else {
                continue;
            };
            let expected = context.placed_in(o.parent, o.component, was);
            let same = result.placements.get(occurrence).is_some_and(|now| {
                let size = was.translation.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
                (0..3).all(|r| {
                    (expected.translation[r] - now.translation[r]).abs() <= 1e-6 * size
                        && (0..3).all(|c| (expected.linear[r][c] - now.linear[r][c]).abs() <= 1e-9)
                })
            });
            if !same {
                moved.push(next.assembly.occurrence_name(*occurrence));
            }
        }
        if !moved.is_empty() {
            return Err(invalid(format!(
                "Reorient Design cannot turn the design exactly: {} would not be where the turn \
                 takes {}",
                moved.join(", "),
                if moved.len() == 1 { "it" } else { "them" }
            )));
        }
        self.apply(move |state| {
            *state = next;
            Ok(("Reorient Design".to_owned(), report))
        })
    }

    /// Each body of a recompute with its component, bounding box and
    /// centre of mass (when the kernel measures it).
    fn boxes_of(
        &self,
        result: &Recomputed<K::Shape>,
    ) -> Result<BTreeMap<BodyUid, BodyFacts>, ModelError> {
        result
            .bodies
            .iter()
            .map(|(uid, shape)| {
                let component = result
                    .owners
                    .get(uid)
                    .copied()
                    .unwrap_or(ComponentUid::ROOT);
                let bounds = self
                    .kernel
                    .bounding_box(&shape.value)
                    .map_err(|e| invalid(format!("Reorient Design: {uid}: {e}")))?;
                let center = self
                    .kernel
                    .mass_properties(&shape.value)
                    .ok()
                    .map(|m| m.center)
                    .filter(|c| c.iter().all(|v| v.is_finite()));
                Ok((
                    *uid,
                    BodyFacts {
                        component,
                        bounds,
                        center,
                    },
                ))
            })
            .collect()
    }

    fn body_label(&self, state: &DocState, body: BodyUid) -> String {
        let name = state
            .body_names
            .get(&body)
            .cloned()
            .unwrap_or_else(|| body.to_string());
        match state.entry(body.feature) {
            Some(feature) => format!("{name} (made by {})", feature.name),
            None => name,
        }
    }
}

/// The components that turn: all but those linked from other files.
fn turned_components(state: &DocState) -> BTreeSet<ComponentUid> {
    let mut turned = BTreeSet::from([ComponentUid::ROOT]);
    turned.extend(
        state
            .assembly
            .components
            .iter()
            .filter(|c| c.link.is_none())
            .map(|c| c.uid),
    );
    turned
}

/// Rewrites the state (see [`crate::reorient`]): definitions, the Moves of
/// base features, occurrences, named views, section analyses and lights.
fn turn_state(
    state: &mut DocState,
    turn: Turn,
    turned: &BTreeSet<ComponentUid>,
    report: &mut ReorientReport,
) -> Result<(), ModelError> {
    let assembly = state.assembly.clone();
    let context = Turned {
        turn,
        turned,
        assembly: &assembly,
    };
    // Definitions.
    let mut bases = Vec::new();
    let mut negated = Vec::new();
    for i in 0..state.features.len() {
        let entry = &state.features[i];
        if !turned.contains(&entry.component) {
            continue;
        }
        let (def, negate) = crate::reorient::turn_definition(&entry.def, entry.component, &context)
            .map_err(|e| invalid(format!("Reorient Design cannot turn {}: {e}", entry.name)))?;
        negated.extend(negate.into_iter().map(|id| (entry.uid, id)));
        if let FeatureDef::Base(base) = &entry.def {
            bases.push((entry.uid, base.clone()));
        }
        if def != entry.def {
            report.features.push(entry.name.clone());
            let mut changed = (**entry).clone();
            changed.def = def;
            state.features[i] = Arc::new(changed);
        }
    }
    // Values along the model axes (a Move's x, y, z) went with their axes;
    // those of an axis that now runs the other way are negated.
    negate_parameters(state, &negated)?;
    // Imported bodies: a Move right after each base feature turns them.
    for (uid, base) in bases {
        let name = add_base_move(state, uid, &base, turn)?;
        report.moves.push(name);
    }
    // Occurrences: their placements, in both coordinates they join.
    for o in &mut state.assembly.occurrences {
        if !turned.contains(&o.parent) {
            continue;
        }
        let placed = context.placed_in(o.parent, o.component, &o.transform);
        if placed != o.transform {
            o.transform = placed;
            report
                .occurrences
                .push(assembly.occurrence_name(o.uid).to_string());
        }
    }
    // Named views, section analyses and the lights in the design.
    for view in &mut state.named_views {
        view.eye = turn.vector(view.eye);
        view.target = turn.vector(view.target);
        view.up = turn.vector(view.up);
        report.views += 1;
    }
    for analysis in &mut state.analyses {
        if crate::reorient::turn_analysis(&mut analysis.def, &context) {
            report.analyses += 1;
        }
    }
    report.lights += crate::reorient::turn_lights(&mut state.render, turn);
    // What split features remember of their pieces (centres of mass).
    crate::reorient::turn_split_pieces(state, turn, turned);
    Ok(())
}

/// Negates the dimension parameters of features' values along the model
/// axes whose axis the turn reverses. Only a feature's own parameter that
/// nothing else uses can change so; otherwise the command is refused, as
/// the other users would change with it.
fn negate_parameters(
    state: &mut DocState,
    negated: &[(FeatureUid, crate::parameters::ParamId)],
) -> Result<(), ModelError> {
    let mut expressions: Vec<(crate::parameters::ParamId, String)> = Vec::new();
    for (feature, id) in negated {
        let Some(parameter) = state.parameters.get(*id) else {
            continue;
        };
        if parameter.value() == 0.0 {
            continue;
        }
        let name = parameter.name().to_owned();
        let feature_name = state
            .entry(*feature)
            .map(|f| f.name.clone())
            .unwrap_or_default();
        let uses = state
            .features
            .iter()
            .flat_map(|f| f.def.params())
            .filter(|p| p == id)
            .count();
        let shared = state.parameters.owner(*id) != Some(*feature)
            || uses != 1
            || !state.parameters.table().dependents(*id).is_empty()
            || state.configurations.parameters.contains(&name);
        if shared {
            return Err(invalid(format!(
                "Reorient Design cannot turn {feature_name}: its value {name} runs along an axis \
                 the turn reverses, and other values or features use it"
            )));
        }
        let expression = parameter.expression().trim();
        let negative = if parameter.dependencies().is_empty() {
            match expression.strip_prefix('-') {
                Some(rest) => rest.trim().to_owned(),
                None => format!("-{expression}"),
            }
        } else {
            format!("-({expression})")
        };
        expressions.push((*id, negative));
    }
    if expressions.is_empty() {
        return Ok(());
    }
    let pairs: Vec<(crate::parameters::ParamId, &str)> = expressions
        .iter()
        .map(|(id, text)| (*id, text.as_str()))
        .collect();
    state
        .parameters
        .set_expressions(&pairs)
        .map_err(ModelError::Parameter)?;
    Ok(())
}

/// Adds a Move of the base feature's new bodies right after it, turning
/// them about the origin. Returns its name.
fn add_base_move(
    state: &mut DocState,
    base: FeatureUid,
    def: &crate::features::BaseDef,
    turn: Turn,
) -> Result<String, ModelError> {
    let index = state
        .features
        .iter()
        .position(|f| f.uid == base)
        .expect("the base feature is in the timeline");
    let entry = state.features[index].clone();
    if def.operation != crate::features::Operation::NewBody {
        return Err(invalid(format!(
            "Reorient Design cannot turn {}: its bodies {} other bodies, so a Move cannot turn \
             them alone",
            entry.name,
            match def.operation {
                crate::features::Operation::Join => "join",
                crate::features::Operation::Cut => "cut",
                _ => "intersect",
            }
        )));
    }
    let bodies: Vec<BodyUid> = (0..def.bodies.len())
        .map(|i| {
            def.replaces
                .get(i)
                .copied()
                .unwrap_or(BodyUid::new(base, i as u32))
        })
        .collect();
    let matrix = crate::assembly::matrix_rows(&turn.transform());
    let mv: FeatureDef<crate::features::ValueInput> = FeatureDef::Move(crate::features::MoveDef {
        bodies,
        transform: crate::features::moves::MoveSpec::Free { matrix },
        copy: false,
    });
    // In at the base feature's place, the marker kept after the features
    // it was after.
    let marker = state.marker;
    state.marker = index + 1;
    let added = state.add_feature_in(&mv, None, entry.component)?;
    state.marker = if index < marker { marker + 1 } else { marker };
    // A suppressed base feature's Move is suppressed with it.
    if entry.suppressed
        && let Some(i) = state.position(added.uid)
    {
        let mut moved: FeatureEntry = (*state.features[i]).clone();
        moved.suppressed = true;
        state.features[i] = Arc::new(moved);
    }
    Ok(added.name)
}

/// The features that built at the marker.
fn ok_features<S>(result: &Recomputed<S>) -> BTreeSet<FeatureUid> {
    result
        .results
        .iter()
        .filter(|r| r.status.is_ok())
        .map(|r| r.uid)
        .collect()
}

fn sketch_frames<S>(result: &Recomputed<S>) -> BTreeMap<FeatureUid, SketchFrame> {
    result
        .sketches
        .iter()
        .map(|(uid, output)| (*uid, output.value.frame))
        .collect()
}

/// The sketches whose turned frame is not their old one turned, with the
/// own frame ([`FrameDef`]) that makes it so.
fn frame_fixes<S>(
    state: &DocState,
    result: &Recomputed<S>,
    old: &BTreeMap<FeatureUid, SketchFrame>,
    turn: Turn,
    turned: &BTreeSet<ComponentUid>,
) -> Vec<(FeatureUid, Option<FrameDef>)> {
    let mut fixes = Vec::new();
    for entry in &state.features {
        let FeatureDef::Sketch(sketch) = &entry.def else {
            continue;
        };
        if !turned.contains(&entry.component) {
            continue;
        }
        let (Some(was), Some(now)) = (old.get(&entry.uid), result.sketches.get(&entry.uid)) else {
            continue;
        };
        let target = turn.frame(was);
        let now = now.value.frame;
        if same_frame(&now, &target) {
            continue;
        }
        // The target in the current frame's coordinates, then in the
        // plane's through the own frame the sketch has.
        let correction = relative(&now, &target);
        let own = match &sketch.frame {
            Some(own) => compose(own, &correction),
            None => correction,
        };
        // A sketch that stands on one corrected in this round is looked
        // at again in the next.
        fixes.push((entry.uid, (!is_identity(&own)).then_some(own)));
    }
    fixes
}

fn set_sketch_frame(state: &mut DocState, uid: FeatureUid, frame: Option<FrameDef>) {
    if let Some(i) = state.features.iter().position(|f| f.uid == uid) {
        let mut entry: FeatureEntry = (*state.features[i]).clone();
        if let FeatureDef::Sketch(sketch) = &mut entry.def {
            sketch.frame = frame.map(Box::new);
        }
        state.features[i] = Arc::new(entry);
    }
}

fn sort_by_timeline(state: &DocState, names: &mut [String]) {
    let order: BTreeMap<&str, usize> = state
        .features
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.as_str(), i))
        .collect();
    names.sort_by_key(|name| order.get(name.as_str()).copied().unwrap_or(usize::MAX));
}

fn same_frame(a: &SketchFrame, b: &SketchFrame) -> bool {
    let near = |u: [f64; 3], v: [f64; 3], scale: f64| {
        (0..3).all(|i| (u[i] - v[i]).abs() <= FRAME_TOLERANCE * scale)
    };
    let size = a
        .origin
        .iter()
        .chain(&b.origin)
        .fold(1.0_f64, |m, v| m.max(v.abs()));
    near(a.origin, b.origin, size) && near(a.x_axis, b.x_axis, 1.0) && near(a.y_axis, b.y_axis, 1.0)
}

/// `target` in the coordinates of `frame` (x, y, normal).
fn relative(frame: &SketchFrame, target: &SketchFrame) -> FrameDef {
    let n = frame.normal();
    let local = |v: [f64; 3]| [dot(v, frame.x_axis), dot(v, frame.y_axis), dot(v, n)];
    let offset: [f64; 3] = std::array::from_fn(|i| target.origin[i] - frame.origin[i]);
    FrameDef {
        origin: local(offset),
        x_axis: local(target.x_axis),
        y_axis: local(target.y_axis),
    }
}

/// The own frame `inner` (in the sketch frame `outer` makes) as one in
/// the plane's coordinates.
fn compose(outer: &FrameDef, inner: &FrameDef) -> FrameDef {
    let n = cross(outer.x_axis, outer.y_axis);
    let place = |v: [f64; 3]| -> [f64; 3] {
        std::array::from_fn(|i| v[0] * outer.x_axis[i] + v[1] * outer.y_axis[i] + v[2] * n[i])
    };
    let o = place(inner.origin);
    FrameDef {
        origin: std::array::from_fn(|i| outer.origin[i] + o[i]),
        x_axis: place(inner.x_axis),
        y_axis: place(inner.y_axis),
    }
}

fn is_identity(frame: &FrameDef) -> bool {
    let near = |u: [f64; 3], v: [f64; 3]| (0..3).all(|i| (u[i] - v[i]).abs() <= 1e-12);
    near(frame.origin, [0.0; 3])
        && near(frame.x_axis, [1.0, 0.0, 0.0])
        && near(frame.y_axis, [0.0, 1.0, 0.0])
}

fn turn_box(turn: Turn, b: &BoundingBox) -> BoundingBox {
    let (p, q) = (turn.vector(b.min), turn.vector(b.max));
    BoundingBox {
        min: std::array::from_fn(|i| p[i].min(q[i])),
        max: std::array::from_fn(|i| p[i].max(q[i])),
    }
}

/// The largest side of a box, at least 1 mm.
fn box_size(b: &BoundingBox) -> f64 {
    (0..3).fold(1.0_f64, |m, i| m.max(b.max[i] - b.min[i]))
}

/// Boxes within 1e-6 of their size (at least 1e-6 mm).
fn same_box(a: Option<&BoundingBox>, b: Option<&BoundingBox>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let tolerance = 1e-6 * box_size(a);
            (0..3).all(|i| {
                (a.min[i] - b.min[i]).abs() <= tolerance && (a.max[i] - b.max[i]).abs() <= tolerance
            })
        }
        _ => false,
    }
}
