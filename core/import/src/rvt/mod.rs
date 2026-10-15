// SPDX-License-Identifier: MIT
//! The `.rvt` import (mitcad#274, mitcad#275): a project's elements, one
//! component per level, each level's elevation a construction plane of its
//! component and each grid a construction axis (an arc grid a sketched arc).
//!
//! - **Walls, floors, roofs, columns and beams** are rebuilt from their
//!   parameters as features ([`plan`], [`footprint`]): a sketch and an
//!   extrusion each, with the cuts of the inserts they host and of the
//!   elements whose geometry cuts them, in one command per element. Each
//!   element's result is checked: against the IFC export of the same
//!   project when one is given ([`Reference`], by the element's GlobalId:
//!   its volume within 1 % and its box within 5 mm), else against its
//!   stored box. An element whose features fail or disagree is undone and
//!   comes in as below, with the reason on its report line.
//! - **Every other element** comes in as a body named `Family:Type:Id`
//!   (or by its class and id) of its level's base feature: the triangle
//!   mesh its record saves where it has one (a mesh body), else its
//!   bounding box: a family instance's type box at the instance's
//!   transform, any other element's box in model coordinates.
//! - **Materials** become appearances (colour, transparency) of the bodies
//!   built as features: a layered type's exterior layer's material, a
//!   family type's first.
//! - **Links**: each linked model found ([`RvtOptions::links`]) comes in
//!   the same way into a component of its own, placed by its link
//!   instance's transform; a link not found leaves its component empty and
//!   the report names it.
//!
//! Types, view-specific elements (annotations), members of groups'
//! definitions, elements without a box and elements of categories that are
//! no building elements (levels, grids, rooms, sketch lines, views) stay
//! out; the report counts them by reason. The reader is `mitcad_rvt`; the
//! geometry goes through the model's [`Kernel`].

pub mod footprint;
pub mod plan;

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use serde_json::json;

use mitcad_model::{
    BaseInput, ComponentUid, Document, FeatureUid, ImportBody, Kernel, ModelError, PrimitiveShape,
    PrimitiveSpec, SketchFrame, Transform,
};
use mitcad_rvt::building::{Curve, Link};
use mitcad_rvt::categories;
use mitcad_rvt::elements::{Element, FOOT_MM, Model};

use crate::Progress;
use crate::ifc::build::{Builder, Node, Op};
use plan::Plan;

/// Extents (feet) below which a box has no volume.
const THIN: f64 = 1e-6;

/// How far (feet) a family type's box at the instance's transform may lie
/// from the element's own box and still be used.
const BOX_TOLERANCE: f64 = 0.01;

/// How deep links of linked models are followed (the project's links only).
pub const LINK_DEPTH: usize = 1;

/// A level's component in the report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LevelReport {
    /// The level element; None for the component of elements without one.
    pub id: Option<i64>,
    /// The other level elements of the same name and elevation, whose
    /// elements it holds too (mitcad#310).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub merged: Vec<i64>,
    /// The component's name: the level's, numbered when another component
    /// has it.
    pub name: String,
    /// Millimetres.
    pub elevation: Option<f64>,
    pub component: Option<String>,
    /// Its base feature of the elements not built as features.
    pub feature: Option<String>,
    /// The link (an index into the report's `links`) whose model it is of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<usize>,
    /// Elements built as features.
    pub features: usize,
    /// Bodies of its base feature: saved meshes and boxes.
    pub bodies: usize,
    pub meshes: usize,
    pub boxes: usize,
    /// The elements it holds (as features or bodies).
    #[serde(skip)]
    pub elements: Vec<i64>,
}

/// One element's line of the report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ElementLine {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<usize>,
    pub global_id: Option<String>,
    /// `wall`, `floor`, `roof`, `column`, `beam`, or `other`.
    pub kind: String,
    pub category: String,
    pub name: String,
    pub level: Option<String>,
    /// `features`, `mesh` or `box`.
    pub outcome: String,
    pub features: usize,
    pub bodies: usize,
    /// The built solid's volume (mm³) and what it was checked against
    /// (`reference` or `stored box`).
    pub volume: Option<f64>,
    pub checked_against: Option<String>,
    pub reference_volume: Option<f64>,
    pub volume_difference: Option<f64>,
    /// mm.
    pub box_difference: Option<f64>,
    pub notes: Vec<String>,
}

/// A link's line of the report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LinkReport {
    /// The link instance, its type and the linked file's name.
    pub id: i64,
    pub file: String,
    /// The file read, when found.
    pub found: Option<String>,
    pub component: Option<String>,
    pub elements: usize,
    pub note: Option<String>,
}

/// What the import did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub levels: Vec<LevelReport>,
    /// Elements built as features, by kind.
    pub features: usize,
    pub built: BTreeMap<String, usize>,
    /// Elements of those kinds that came in as meshes or boxes instead.
    pub fallbacks: BTreeMap<String, usize>,
    /// The reference the features were checked against (its file name).
    pub reference: Option<String>,
    /// Elements built as features that agree with the reference, by its
    /// kind of solid (`IFCWALL`, ...).
    pub reference_agree: BTreeMap<String, usize>,
    /// The reference's walls, slabs, roofs, columns, beams and members
    /// that no element built as features agrees with: kind, name,
    /// GlobalId and why (at most 100).
    pub reference_unmatched: Vec<String>,
    /// Bodies made of the other elements: saved meshes, boxes at a
    /// transform, boxes in model coordinates.
    pub bodies: usize,
    pub meshes: usize,
    pub oriented_boxes: usize,
    pub boxes: usize,
    /// Construction planes of the levels, axes and sketches of the grids,
    /// appearances of the materials.
    pub level_planes: usize,
    pub grid_axes: usize,
    pub grid_arcs: usize,
    pub appearances: usize,
    pub links: Vec<LinkReport>,
    /// Imported elements by category.
    pub categories: BTreeMap<String, usize>,
    /// Elements left out, by reason.
    pub left_out: BTreeMap<String, usize>,
    /// Elements whose records name no level, placed on the highest level
    /// at or below their box (or the lowest one).
    pub by_elevation: usize,
    /// Elements whose body could not be built: (id, why), at most 50.
    pub not_built: Vec<(i64, String)>,
    /// Elements left out because the process ran low on memory.
    pub memory_low: usize,
    pub warnings: Vec<String>,
    /// A line for every element imported.
    pub elements: Vec<ElementLine>,
}

/// Why an element is not imported, or None.
fn left_out(model: &Model, e: &Element) -> Option<String> {
    if e.partition.is_none() {
        return Some("no records".into());
    }
    if e.is_type {
        return Some("types".into());
    }
    if e.owner_view.is_some() {
        return Some("view-specific (annotations)".into());
    }
    if e.unplaced_owner.is_some() {
        return Some("in a group's definition".into());
    }
    let has_mesh = model.meshes.contains_key(&e.id);
    let solid_box = e
        .bbox
        .is_some_and(|b| (0..3).all(|k| (b[k + 3] - b[k]).is_finite() && b[k + 3] - b[k] > THIN));
    if !has_mesh && !solid_box {
        return Some("no box".into());
    }
    match e.category {
        Some(c) if categories::is_model(c) => None,
        Some(c) => Some(format!(
            "not a building element ({})",
            categories::name(c).map_or(c.to_string(), str::to_string)
        )),
        None => Some("no category".into()),
    }
}

/// A box (feet, min and max corner) placed by a transform (rows of the
/// matrix, then the origin): the frame of its first corner and its sizes
/// (mm).
fn placed_box(local: [f64; 6], t: &[f64; 12]) -> (SketchFrame, [f64; 3]) {
    let apply = |p: [f64; 3]| -> [f64; 3] {
        std::array::from_fn(|r| {
            t[3 * r] * p[0] + t[3 * r + 1] * p[1] + t[3 * r + 2] * p[2] + t[9 + r]
        })
    };
    let column = |c: usize| [t[c], t[3 + c], t[6 + c]];
    let (x, mut y, z) = (column(0), column(1), column(2));
    let cross = [
        x[1] * y[2] - x[2] * y[1],
        x[2] * y[0] - x[0] * y[2],
        x[0] * y[1] - x[1] * y[0],
    ];
    let mirrored = cross[0] * z[0] + cross[1] * z[1] + cross[2] * z[2] < 0.0;
    // A mirrored transform: the box spans the other way along y.
    let corner = if mirrored {
        y = y.map(|v| -v);
        [local[0], local[4], local[2]]
    } else {
        [local[0], local[1], local[2]]
    };
    let origin = apply(corner).map(|v| v * FOOT_MM);
    (
        SketchFrame {
            origin,
            x_axis: x,
            y_axis: y,
        },
        [
            (local[3] - local[0]) * FOOT_MM,
            (local[4] - local[1]) * FOOT_MM,
            (local[5] - local[2]) * FOOT_MM,
        ],
    )
}

/// The axis-aligned box of a box at a transform (feet).
fn placed_bounds(local: [f64; 6], t: &[f64; 12]) -> [f64; 6] {
    let mut out = [
        f64::INFINITY,
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for i in 0..8 {
        let p = [
            if i & 1 == 0 { local[0] } else { local[3] },
            if i & 2 == 0 { local[1] } else { local[4] },
            if i & 4 == 0 { local[2] } else { local[5] },
        ];
        for r in 0..3 {
            let v = t[3 * r] * p[0] + t[3 * r + 1] * p[1] + t[3 * r + 2] * p[2] + t[9 + r];
            out[r] = out[r].min(v);
            out[r + 3] = out[r + 3].max(v);
        }
    }
    out
}

/// How an element's body is made.
enum Shape {
    Mesh,
    /// A type's box at the instance's transform.
    Oriented(SketchFrame, [f64; 3]),
    /// The element's box in model coordinates.
    Aligned(SketchFrame, [f64; 3]),
}

fn shape_of(model: &Model, e: &Element) -> Option<Shape> {
    if model.meshes.contains_key(&e.id) {
        return Some(Shape::Mesh);
    }
    let bbox = e.bbox?;
    // The instance's own symbol first (its geometry as placed), then its
    // type; used when it agrees with the element's box.
    if let Some(t) = &e.transform {
        for symbol in [e.symbol, e.type_id].into_iter().flatten() {
            let Some(local) = model.outlines.get(&symbol).copied() else {
                continue;
            };
            if (0..3).any(|k| local[k + 3] - local[k] <= THIN) {
                continue;
            }
            let placed = placed_bounds(local, t);
            if (0..6).all(|k| (placed[k] - bbox[k]).abs() <= BOX_TOLERANCE) {
                let (frame, size) = placed_box(local, t);
                return Some(Shape::Oriented(frame, size));
            }
        }
    }
    Some(Shape::Aligned(
        SketchFrame {
            origin: [bbox[0] * FOOT_MM, bbox[1] * FOOT_MM, bbox[2] * FOOT_MM],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
        },
        [
            (bbox[3] - bbox[0]) * FOOT_MM,
            (bbox[4] - bbox[1]) * FOOT_MM,
            (bbox[5] - bbox[2]) * FOOT_MM,
        ],
    ))
}

/// A box (mm) on a frame through the kernel.
fn make_box<K: Kernel>(
    document: &Document<K>,
    frame: SketchFrame,
    size: [f64; 3],
) -> Result<K::Shape, mitcad_model::KernelError> {
    document.kernel().primitive(&PrimitiveSpec {
        feature: FeatureUid(0),
        frame,
        shape: PrimitiveShape::Box {
            length: size[0],
            width: size[1],
            height: size[2],
        },
    })
}

/// The level an element is placed on: its own, else the highest level at
/// or below its box, else the lowest; whether it was by elevation.
fn level_of(model: &Model, e: &Element) -> Option<(i64, bool)> {
    if let Some(l) = e.level.filter(|l| model.level(*l).is_some()) {
        return Some((l, false));
    }
    let base = e.bbox.map(|b| b[2]).or_else(|| {
        let mesh = model.meshes.get(&e.id)?;
        mesh.iter()
            .filter_map(|m| m.bounds())
            .map(|b| b[2])
            .reduce(f64::min)
    })?;
    let below = model
        .levels
        .iter()
        .filter(|l| l.elevation <= base + 1e-6)
        .max_by(|a, b| a.elevation.total_cmp(&b.elevation));
    below.or(model.levels.first()).map(|l| (l.id, true))
}

/// An element's solid in the reference (the IFC export of the same
/// project): its volume (mm³) and its box (mm), by GlobalId.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReferenceSolid {
    pub kind: String,
    pub name: String,
    pub volume: Option<f64>,
    pub bbox: Option<([f64; 3], [f64; 3])>,
}

/// The reference the elements built as features are checked against.
#[derive(Debug, Clone, Default)]
pub struct Reference {
    /// The reference file's name.
    pub file: String,
    pub solids: HashMap<String, ReferenceSolid>,
}

/// A linked model found and read, for its link instance.
pub struct LinkedModel {
    pub link: Link,
    /// The file it was read from (its name).
    pub file: String,
    pub model: Model,
}

/// The options of [`import_rvt`].
#[derive(Default)]
pub struct RvtOptions<'a> {
    /// The memory guard (mitcad#80).
    pub progress: Option<&'a Progress>,
    /// The IFC export to check the features against (else each element's
    /// stored box).
    pub reference: Option<&'a Reference>,
    /// The linked models found, by their link instances.
    pub links: Vec<LinkedModel>,
    /// The links not found: the link and why.
    pub missing_links: Vec<(Link, String)>,
    /// Every element as its mesh or box (phase 1), no features.
    pub mesh_only: bool,
}

/// Fraction of the reference volume the built volume may differ by.
pub const VOLUME_TOLERANCE: f64 = 0.01;
/// How far (mm) the built box's sides may lie from the reference's.
pub const BOX_TOLERANCE_MM: f64 = 5.0;

type Bbox = ([f64; 3], [f64; 3]);

/// A level's component, its report and the bodies of its base feature.
struct LevelGroup<S> {
    entry: LevelReport,
    component: ComponentUid,
    bodies: Vec<ImportBody<S>>,
}

/// The volume and box (mm, the component's coordinates) of the bodies of
/// features, and how many bodies.
fn measure<K: Kernel>(
    document: &Document<K>,
    features: &[FeatureUid],
) -> (Option<f64>, Option<Bbox>, usize) {
    let kernel = document.kernel();
    let mut volume = 0.0;
    let mut measured = false;
    let mut bbox: Option<Bbox> = None;
    let mut count = 0;
    for body in document.bodies() {
        if !features.contains(&body.uid.feature) {
            continue;
        }
        count += 1;
        if let Ok(m) = kernel.mass_properties(body.shape) {
            volume += m.volume;
            measured = true;
        }
        if let Ok(Some(b)) = kernel.bounding_box(body.shape) {
            bbox = Some(match bbox {
                None => (b.min, b.max),
                Some((lo, hi)) => (
                    std::array::from_fn(|k| lo[k].min(b.min[k])),
                    std::array::from_fn(|k| hi[k].max(b.max[k])),
                ),
            });
        }
    }
    (measured.then_some(volume), bbox, count)
}

/// The largest distance (mm) between the sides of two boxes.
pub fn box_difference(a: &Bbox, b: &Bbox) -> f64 {
    (0..3)
        .map(|k| (a.0[k] - b.0[k]).abs().max((a.1[k] - b.1[k]).abs()))
        .fold(0.0, f64::max)
}

/// Whether the built solid agrees with the reference: what it was checked
/// against, or why it disagrees.
fn check(
    line: &mut ElementLine,
    e: &Element,
    reference: Option<&Reference>,
    volume: Option<f64>,
    bbox: Option<Bbox>,
) -> Result<&'static str, String> {
    let bbox = bbox.ok_or("its features made no body")?;
    let volume = volume
        .filter(|v| *v > 0.0)
        .ok_or("its body encloses no volume")?;
    let solid = reference.and_then(|r| line.global_id.as_ref().and_then(|g| r.solids.get(g)));
    if let Some(solid) = solid {
        if let Some(rv) = solid.volume.filter(|v| *v > 0.0) {
            let d = (volume - rv).abs() / rv;
            line.reference_volume = Some(rv);
            line.volume_difference = Some(d);
            if d > VOLUME_TOLERANCE {
                return Err(format!(
                    "its volume {:.4} m³ differs from the reference's {:.4} m³ by {:.2} %",
                    volume * 1e-9,
                    rv * 1e-9,
                    d * 100.0
                ));
            }
        }
        if let Some(rb) = &solid.bbox {
            let d = box_difference(&bbox, rb);
            line.box_difference = Some(d);
            if d > BOX_TOLERANCE_MM {
                return Err(format!("its box lies {d:.1} mm from the reference's"));
            }
        }
        return Ok("reference");
    }
    // No reference solid: the element's stored box must hold it.
    let stored = e
        .bbox
        .ok_or("no reference solid and no stored box to check against")?;
    let s: Bbox = (
        [stored[0], stored[1], stored[2]].map(|v| v * FOOT_MM),
        [stored[3], stored[4], stored[5]].map(|v| v * FOOT_MM),
    );
    let out = (0..3)
        .map(|k| (s.0[k] - bbox.0[k]).max(bbox.1[k] - s.1[k]))
        .fold(f64::NEG_INFINITY, f64::max);
    line.box_difference = Some(out.max(0.0));
    if out > BOX_TOLERANCE_MM {
        return Err(format!(
            "its body reaches {out:.1} mm outside the element's stored box"
        ));
    }
    Ok("stored box")
}

/// What the import does, over the project and its linked models.
struct Importer<'r, 'o, K: Kernel> {
    document: &'r mut Document<K>,
    report: Report,
    options: &'r RvtOptions<'o>,
    /// The document's appearances by material id, per model.
    appearances: HashMap<(usize, i64), Option<String>>,
    feature_names: HashSet<String>,
    body_names: HashSet<String>,
    stopped: bool,
    /// The import's undo depth and step label.
    depth: usize,
    label: String,
    /// Import notes of the base features.
    notes: Vec<(FeatureUid, String)>,
    /// The body name the element being built took (given back when its
    /// features are undone).
    reserved: Option<String>,
}

impl<K: Kernel> Importer<'_, '_, K> {
    fn memory_low(&mut self) -> bool {
        if !self.stopped && self.options.progress.is_some_and(Progress::is_memory_low) {
            self.stopped = true;
        }
        self.stopped
    }

    /// The appearance of a material (made once).
    fn appearance(&mut self, model_key: usize, model: &Model, material: i64) -> Option<String> {
        if let Some(a) = self.appearances.get(&(model_key, material)) {
            return a.clone();
        }
        let made = model
            .materials
            .iter()
            .find(|m| m.id == material)
            .and_then(|m| {
                let color = m.color?;
                let base = if m.name.trim().is_empty() {
                    format!("Material {}", m.id)
                } else {
                    m.name.clone()
                };
                let mut change = mitcad_model::appearance::AppearanceChange {
                    name: Some(base.clone()),
                    base_color: Some(color.map(|c| f64::from(c) / 255.0)),
                    opacity: Some((1.0 - m.transparency.unwrap_or(0.0) / 100.0).clamp(0.0, 1.0)),
                    ..Default::default()
                };
                // A name taken (a linked model's material, an earlier import)
                // gets a number.
                let mut made = self.document.create_appearance(&change);
                let mut n = 2;
                while made
                    .as_ref()
                    .is_err_and(|e| e.to_string().contains("an appearance is named"))
                    && n < 100
                {
                    change.name = Some(format!("{base} ({n})"));
                    made = self.document.create_appearance(&change);
                    n += 1;
                }
                match made {
                    Ok(id) => {
                        self.report.appearances += 1;
                        Some(id)
                    }
                    Err(e) => {
                        self.report
                            .warnings
                            .push(format!("appearance of material {}: {e}", m.id));
                        None
                    }
                }
            });
        self.appearances.insert((model_key, material), made.clone());
        made
    }

    fn unique_body(&mut self, name: &str) -> String {
        let mut out = name.to_string();
        let mut n = 2;
        while self.body_names.contains(&out) {
            out = format!("{name} ({n})");
            n += 1;
        }
        self.body_names.insert(out.clone());
        out
    }

    /// Builds an element's plan in its component; its features stay when
    /// they agree with the reference, else they are undone and the reason
    /// is the error.
    fn build(
        &mut self,
        model_key: usize,
        model: &Model,
        e: &Element,
        plan: Plan,
        component: ComponentUid,
        line: &mut ElementLine,
    ) -> Result<(), String> {
        let label = e.display_name();
        let appearance = plan
            .material
            .and_then(|m| self.appearance(model_key, model, m));
        let body_name = self.unique_body(&label);
        self.reserved = Some(body_name.clone());
        let kernel = self.document.kernel();
        let solid = crate::ifc::build::prepare(kernel, plan.solid);
        let cuts: Vec<(String, Node)> = plan
            .cuts
            .into_iter()
            .map(|(what, t)| (what, crate::ifc::build::prepare(kernel, t)))
            .collect();
        let names = &mut self.feature_names;
        type Built = Result<(Vec<FeatureUid>, Vec<String>), String>;
        let built: Built = self
            .document
            .add_features_with(&format!("Import {label}"), |batch| {
                let mut b = Builder::new(batch, names, component, &label);
                let bodies = match b.node(&solid, Op::New) {
                    Ok(bodies) => bodies,
                    Err(why) => return Ok(Err(why)),
                };
                let mut notes = Vec::new();
                for (what, tool) in &cuts {
                    b.label = format!("{label} cut");
                    if let Err(why) = b.node(tool, Op::With("cut", bodies.clone())) {
                        notes.push(format!("the cut of {what} is not made: {why}"));
                    }
                }
                for (k, body) in bodies.iter().enumerate() {
                    let name = if k == 0 {
                        body_name.clone()
                    } else {
                        format!("{body_name} ({})", k + 1)
                    };
                    let _ = b.batch.name_body(*body, &name);
                    if let Some(a) = &appearance {
                        let _ = b.batch.set_body_appearance(*body, a);
                    }
                }
                notes.extend(b.notes.iter().cloned());
                Ok(Ok((b.features.clone(), notes)))
            })
            .map_err(|e| format!("its features were refused: {e}"))?;
        let (features, notes) = built?;
        line.notes.extend(notes);
        // A cut that removes nothing stays, suppressed; another failing
        // feature fails the element.
        let mut failed = None;
        let mut misses = Vec::new();
        for uid in &features {
            if let Some(error) = self.document.status(*uid).and_then(|s| s.error()) {
                // (An intersection whose tool holds the whole body changes
                // nothing either: an edited wall within its footprint.)
                if crate::ifc::is_miss(error)
                    || error.contains("does not intersect any participant body")
                {
                    misses.push(*uid);
                } else if failed.is_none() {
                    failed = Some(error.to_string());
                }
            }
        }
        if failed.is_none() {
            for uid in misses {
                if self.document.set_suppressed(uid, true).is_ok() {
                    line.notes
                        .push("a cut that removes nothing is suppressed".into());
                }
            }
        }
        let result = match failed {
            Some(error) => Err(format!("a feature failed: {error}")),
            None => {
                let (volume, bbox, bodies) = measure(self.document, &features);
                line.volume = volume;
                line.bodies = bodies;
                check(line, e, self.options.reference, volume, bbox)
            }
        };
        match result {
            Ok(against) => {
                line.checked_against = Some(against.to_string());
                line.features = features.len();
                self.document.merge_undo(self.depth, &self.label);
                Ok(())
            }
            Err(why) => {
                // The element's features (and suppressions) undone.
                while self.document.undo_depth() > self.depth + 1 {
                    if self.document.undo().is_none() {
                        break;
                    }
                }
                Err(why)
            }
        }
    }

    /// An element as its saved mesh or its box, in its level's base
    /// feature.
    fn fallback(
        &mut self,
        model: &Model,
        e: &Element,
        group: &mut LevelGroup<K::Shape>,
        line: &mut ElementLine,
    ) {
        let shape = match shape_of(model, e) {
            Some(Shape::Mesh) => {
                let meshes = &model.meshes[&e.id];
                let mut vertices = Vec::new();
                let mut triangles = Vec::new();
                for m in meshes {
                    let first = vertices.len() as u32;
                    vertices.extend(m.points.iter().map(|p| p.map(|v| v * FOOT_MM)));
                    triangles.extend(m.triangles.iter().map(|t| t.map(|i| i + first)));
                }
                self.document
                    .kernel()
                    .triangle_body(&vertices, &triangles)
                    .map(|s| (s, 0))
            }
            Some(Shape::Oriented(frame, size)) => {
                make_box(self.document, frame, size).map(|s| (s, 1))
            }
            Some(Shape::Aligned(frame, size)) => {
                make_box(self.document, frame, size).map(|s| (s, 2))
            }
            None => return,
        };
        match shape {
            Ok((shape, kind)) => {
                match kind {
                    0 => {
                        group.entry.meshes += 1;
                        self.report.meshes += 1;
                        line.outcome = "mesh".into();
                    }
                    1 => {
                        group.entry.boxes += 1;
                        self.report.oriented_boxes += 1;
                        line.outcome = "box".into();
                    }
                    _ => {
                        group.entry.boxes += 1;
                        self.report.boxes += 1;
                        line.outcome = "box".into();
                    }
                }
                line.bodies = 1;
                group.entry.elements.push(e.id);
                let name = self.unique_body(&e.display_name());
                group.bodies.push(ImportBody {
                    name: Some(name),
                    color: None,
                    shape,
                });
            }
            Err(err) => {
                line.outcome = "not built".into();
                line.notes.push(format!("its body was not built: {err}"));
                if self.report.not_built.len() < 50 {
                    self.report.not_built.push((e.id, err.to_string()));
                }
            }
        }
    }

    /// The levels' construction planes and the grids' axes (an arc grid's
    /// sketched arc), in one command.
    fn levels_and_grids(
        &mut self,
        model: &Model,
        parent: ComponentUid,
        levels: &[LevelGroup<K::Shape>],
    ) {
        let planes: Vec<(ComponentUid, f64, String)> = levels
            .iter()
            .filter_map(|g| {
                Some((
                    g.component,
                    g.entry.elevation?,
                    format!("{} level", g.entry.name),
                ))
            })
            .collect();
        let grids = &model.building.grids;
        if planes.is_empty() && grids.is_empty() {
            return;
        }
        let names = &mut self.feature_names;
        let mut unique = |name: &str| {
            let mut out = name.to_string();
            let mut n = 2;
            while names.contains(&out) {
                out = format!("{name} ({n})");
                n += 1;
            }
            names.insert(out.clone());
            out
        };
        let plane_names: Vec<String> = planes.iter().map(|(_, _, n)| unique(n)).collect();
        let grid_names: Vec<String> = grids
            .iter()
            .map(|g| {
                unique(&format!(
                    "Grid {}",
                    if g.name.is_empty() {
                        g.id.to_string()
                    } else {
                        g.name.clone()
                    }
                ))
            })
            .collect();
        let mut warnings = Vec::new();
        let result = self.document.add_features_with("Import levels and grids", |batch| {
            let mut made = (0, 0, 0);
            for ((component, z, _), name) in planes.iter().zip(&plane_names) {
                let def = json!({"type": "construction_plane", "definition": {"type": "fixed",
                    "origin": [0.0, 0.0, z], "x_axis": [1.0, 0.0, 0.0], "y_axis": [0.0, 1.0, 0.0]}});
                let def = serde_json::from_value(def).expect("a plane definition");
                if batch.add(&def, Some(name), *component).is_ok() {
                    made.0 += 1;
                }
            }
            for (grid, name) in grids.iter().zip(&grid_names) {
                match &grid.curve {
                    Some(Curve::Line { start, end }) => {
                        let s = start.map(|v| v * FOOT_MM);
                        let d = [end[0] - start[0], end[1] - start[1], end[2] - start[2]].map(|v| v * FOOT_MM);
                        let def = json!({"type": "construction_axis", "definition": {"type": "fixed",
                            "origin": s, "direction": d}});
                        let def = serde_json::from_value(def).expect("an axis definition");
                        match batch.add(&def, Some(name), parent) {
                            Ok(_) => made.1 += 1,
                            Err(e) => warnings.push(format!("grid {}: {e}", grid.name)),
                        }
                    }
                    Some(arc @ Curve::Arc { .. }) => {
                        let z = arc.start()[2] * FOOT_MM;
                        let frame = mitcad_ifc::math::from_axes(
                            [0.0, 0.0, z],
                            [1.0, 0.0, 0.0],
                            [0.0, 1.0, 0.0],
                            [0.0, 0.0, 1.0],
                        );
                        let path = footprint::to_mm(&mitcad_ifc::geometry::Path {
                            segments: vec![footprint::seg(arc)],
                        });
                        match crate::ifc::sketch::path_sketch(&frame, &path) {
                            Ok(sketch) => {
                                let def = serde_json::from_value(sketch.def)
                                    .map_err(|e| e.to_string());
                                match def.map(|d| batch.add(&d, Some(name), parent)) {
                                    Ok(Ok(_)) => made.2 += 1,
                                    Ok(Err(e)) => warnings.push(format!("grid {}: {e}", grid.name)),
                                    Err(e) => warnings.push(format!("grid {}: {e}", grid.name)),
                                }
                            }
                            Err(e) => warnings.push(format!("grid {}: {e}", grid.name)),
                        }
                    }
                    None => warnings.push(format!(
                        "grid {}: {}",
                        grid.name,
                        grid.note.as_deref().unwrap_or("no curve")
                    )),
                }
            }
            Ok(made)
        });
        match result {
            Ok((p, a, s)) => {
                self.report.level_planes += p;
                self.report.grid_axes += a;
                self.report.grid_arcs += s;
            }
            Err(e) => warnings.push(format!("levels and grids not added: {e}")),
        }
        self.report.warnings.extend(warnings);
    }

    /// Imports a model's elements into `parent`, one component per level.
    fn import_model(
        &mut self,
        model_key: usize,
        file: &str,
        model: &Model,
        parent: ComponentUid,
        link: Option<usize>,
    ) -> Result<usize, ModelError> {
        // Levels of one name and elevation are one group, under the first of
        // them (a project can hold several, mitcad#310); unnamed levels stay
        // apart.
        let mut first_of: HashMap<(&str, u64), i64> = HashMap::new();
        let mut merged: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        let mut group_of: HashMap<i64, i64> = HashMap::new();
        for l in &model.levels {
            let first = if l.name.trim().is_empty() {
                l.id
            } else {
                *first_of
                    .entry((l.name.as_str(), l.elevation.to_bits()))
                    .or_insert(l.id)
            };
            if first != l.id {
                merged.entry(first).or_default().push(l.id);
            }
            group_of.insert(l.id, first);
        }

        // The elements by level, in the table's order; every level has its
        // group.
        let mut groups: BTreeMap<Option<i64>, Vec<&Element>> = BTreeMap::new();
        for l in &model.levels {
            if group_of.get(&l.id) == Some(&l.id) {
                groups.entry(Some(l.id)).or_default();
            }
        }
        for e in &model.elements {
            if let Some(why) = left_out(model, e) {
                *self.report.left_out.entry(why).or_default() += 1;
                continue;
            }
            let level = level_of(model, e);
            if level.is_some_and(|(_, by_elevation)| by_elevation) {
                self.report.by_elevation += 1;
            }
            let level = level.map(|(l, _)| group_of.get(&l).copied().unwrap_or(l));
            groups.entry(level).or_default().push(e);
        }
        let mut order: Vec<Option<i64>> = groups.keys().copied().collect();
        let elevation = |l: &Option<i64>| {
            l.and_then(|l| model.level(l))
                .map_or(f64::NEG_INFINITY, |l| l.elevation)
        };
        order.sort_by(|a, b| elevation(a).total_cmp(&elevation(b)));
        order.retain(|l| l.is_some() || !groups[l].is_empty());

        // One component per level.
        let mut levels: Vec<LevelGroup<K::Shape>> = Vec::new();
        for level in &order {
            let (name, level_elevation) = match level.and_then(|l| model.level(l)) {
                Some(l) => (
                    if l.name.trim().is_empty() {
                        format!("Level {}", l.id)
                    } else {
                        l.name.clone()
                    },
                    Some(l.elevation * FOOT_MM),
                ),
                None => ("Elements".to_string(), None),
            };
            let (component, _) =
                self.document
                    .add_component(Some(&name), parent, Transform::IDENTITY)?;
            levels.push(LevelGroup {
                entry: LevelReport {
                    id: *level,
                    merged: level
                        .and_then(|l| merged.get(&l))
                        .cloned()
                        .unwrap_or_default(),
                    name: self.document.assembly().name(component),
                    elevation: level_elevation,
                    component: Some(component.to_string()),
                    link,
                    ..LevelReport::default()
                },
                component,
                bodies: Vec::new(),
            });
        }
        self.levels_and_grids(model, parent, &levels);
        self.document.merge_undo(self.depth, &self.label);

        let global_ids = model.global_ids();
        let mut count = 0;
        for (slot, level) in order.iter().enumerate() {
            let component = levels[slot].component;
            for e in &groups[level] {
                if self.memory_low() {
                    self.report.memory_low += 1;
                    continue;
                }
                count += 1;
                let kind = if self.options.mesh_only {
                    None
                } else {
                    plan::kind_of(model, e)
                };
                let mut line = ElementLine {
                    id: e.id,
                    link,
                    global_id: global_ids.get(&e.id).cloned(),
                    kind: kind.unwrap_or("other").to_string(),
                    category: e
                        .category_name
                        .map(str::to_string)
                        .or_else(|| e.category.map(|c| format!("category {c}")))
                        .unwrap_or_default(),
                    name: e.display_name(),
                    level: Some(levels[slot].entry.name.clone()),
                    ..ElementLine::default()
                };
                *self
                    .report
                    .categories
                    .entry(line.category.clone())
                    .or_default() += 1;
                if let Some(kind) = kind {
                    self.reserved = None;
                    let built = plan::plan(model, e).and_then(|p| {
                        line.notes.extend(p.notes.iter().cloned());
                        self.build(model_key, model, e, p, component, &mut line)
                    });
                    match built {
                        Ok(()) => {
                            line.outcome = "features".into();
                            self.report.features += 1;
                            *self.report.built.entry(kind.to_string()).or_default() += 1;
                            levels[slot].entry.features += 1;
                            levels[slot].entry.elements.push(e.id);
                            self.report.elements.push(line);
                            continue;
                        }
                        Err(why) => {
                            line.notes.push(format!("not built as features: {why}"));
                            if let Some(name) = self.reserved.take() {
                                self.body_names.remove(&name);
                            }
                            *self.report.fallbacks.entry(kind.to_string()).or_default() += 1;
                        }
                    }
                } else {
                    line.notes.push(if self.options.mesh_only {
                        "its saved mesh or its box (no features asked for)".to_string()
                    } else {
                        "its saved mesh or its box: not an element of a kind built as \
                         features (family instances until their families' forms are read)"
                            .to_string()
                    });
                }
                self.fallback(model, e, &mut levels[slot], &mut line);
                self.report.elements.push(line);
            }
        }

        // The other elements' bodies: one base feature per level.
        let mut inputs: Vec<BaseInput<K::Shape>> = Vec::new();
        let mut notes_of: Vec<(usize, String)> = Vec::new();
        for (slot, group) in levels.iter_mut().enumerate() {
            if group.bodies.is_empty() {
                continue;
            }
            let entry = &mut group.entry;
            let feature_names = &mut self.feature_names;
            let feature_name = std::iter::once(format!("{} elements", entry.name))
                .chain((1..).map(|k| format!("{} elements ({k})", entry.name)))
                .find(|n| !feature_names.contains(n))
                .expect("a free name exists");
            feature_names.insert(feature_name.clone());
            entry.bodies = group.bodies.len();
            notes_of.push((
                slot,
                format!(
                    "The {} elements of {} not built as features: {} saved meshes and {} boxes (a \
                     family type's box at its instance's transform, or the element's box); the \
                     import report's element lines say why",
                    entry.bodies, entry.name, entry.meshes, entry.boxes
                ),
            ));
            inputs.push(BaseInput {
                name: Some(feature_name),
                source: Some(format!("{file}: {}", entry.name)),
                component: Some(group.component),
                ..BaseInput::new(std::mem::take(&mut group.bodies))
            });
        }
        if !inputs.is_empty() {
            let imported = self.document.add_base_features(&self.label, inputs)?;
            for (imported, (slot, note)) in imported.iter().zip(notes_of) {
                levels[slot].entry.feature = Some(imported.feature.uid.to_string());
                self.notes.push((imported.feature.uid, note));
            }
            self.document.merge_undo(self.depth, &self.label);
        }
        self.report.bodies = self.report.meshes + self.report.oriented_boxes + self.report.boxes;
        self.report
            .levels
            .extend(levels.into_iter().map(|g| g.entry));
        Ok(count)
    }
}

/// A link instance's transform (feet) as a placement (mm).
fn link_transform(t: &[f64; 12]) -> Transform {
    Transform {
        linear: [[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]],
        translation: [t[9] * FOOT_MM, t[10] * FOOT_MM, t[11] * FOOT_MM],
    }
}

/// Imports the elements of `model` (read from `file`) into the document,
/// with the linked models the options give: one undo step. The process's
/// memory guard leaves the elements not yet built out when it runs low,
/// and the report says so.
pub fn import_rvt<K: Kernel>(
    document: &mut Document<K>,
    file: &str,
    model: &Model,
    options: &RvtOptions<'_>,
) -> Result<Report, ModelError> {
    let label = format!("Import {file}");
    let depth = document.undo_depth();
    let feature_names: HashSet<String> = document.features().map(|f| f.name.clone()).collect();
    let body_names: HashSet<String> = document.bodies().iter().map(|b| b.name.clone()).collect();
    let mut importer = Importer {
        document,
        report: Report {
            reference: options.reference.map(|r| r.file.clone()),
            ..Report::default()
        },
        options,
        appearances: HashMap::new(),
        feature_names,
        body_names,
        stopped: false,
        depth,
        label: label.clone(),
        notes: Vec::new(),
        reserved: None,
    };
    importer.import_model(0, file, model, ComponentUid::ROOT, None)?;

    // The links: each one's component, filled with its model when found.
    let mut links: Vec<(&Link, Option<&LinkedModel>, Option<String>)> = Vec::new();
    for l in &model.building.links {
        let found = options.links.iter().find(|m| m.link.id == l.id);
        let missing = options
            .missing_links
            .iter()
            .find(|(m, _)| m.id == l.id)
            .map(|(_, why)| why.clone());
        links.push((l, found, missing));
    }
    for (index, (link, found, missing)) in links.into_iter().enumerate() {
        let name = link
            .file
            .clone()
            .unwrap_or_else(|| format!("Link {}", link.id));
        let mut line = LinkReport {
            id: link.id,
            file: name.clone(),
            ..LinkReport::default()
        };
        let transform = link
            .transform
            .map(|t| link_transform(&t))
            .unwrap_or(Transform::IDENTITY);
        let component_name = if link.instance_name.is_empty() {
            name.clone()
        } else {
            format!("{name} : {}", link.instance_name)
        };
        match importer
            .document
            .add_component(Some(&component_name), ComponentUid::ROOT, transform)
        {
            Ok((component, _)) => {
                line.component = Some(component.to_string());
                importer.document.merge_undo(depth, &label);
                if let Some(linked) = found {
                    line.found = Some(linked.file.clone());
                    importer.report.links.push(line);
                    match importer.import_model(
                        index + 1,
                        &linked.file,
                        &linked.model,
                        component,
                        Some(index),
                    ) {
                        Ok(n) => importer.report.links[index].elements = n,
                        Err(e) => {
                            importer.report.links[index].note = Some(format!("not imported: {e}"))
                        }
                    }
                    if !linked.model.building.links.is_empty() {
                        importer.report.links[index].note = Some(format!(
                            "its own {} links are not followed",
                            linked.model.building.links.len()
                        ));
                    }
                    continue;
                }
                line.note =
                    Some(missing.unwrap_or_else(|| {
                        format!("{name} was not found: its component is empty")
                    }));
            }
            Err(e) => line.note = Some(format!("no component: {e}")),
        }
        importer.report.links.push(line);
    }
    if importer.stopped {
        let n = importer.report.memory_low;
        importer.report.warnings.push(format!(
            "the process ran low on memory: {n} elements were left out"
        ));
    }
    if !importer.notes.is_empty()
        && let Err(e) = importer.document.set_import_notes(&importer.notes)
    {
        importer.report.warnings.push(format!("import notes: {e}"));
    }
    let mut report = importer.report;
    if let Some(reference) = options.reference {
        reference_summary(&mut report, reference);
    }
    document.merge_undo(depth, &label);
    Ok(report)
}

/// The reference's kinds of solids the features are built for.
const REFERENCE_KINDS: [&str; 8] = [
    "IFCWALL",
    "IFCWALLSTANDARDCASE",
    "IFCSLAB",
    "IFCROOF",
    "IFCCOLUMN",
    "IFCBEAM",
    "IFCMEMBER",
    "IFCSTAIRFLIGHT",
];

/// How the reference's solids fared: those agreed with by kind, and the
/// walls, slabs, roofs, columns and beams no feature agrees with.
fn reference_summary(report: &mut Report, reference: &Reference) {
    let lines: HashMap<&str, &ElementLine> = report
        .elements
        .iter()
        .filter(|l| l.link.is_none())
        .filter_map(|l| Some((l.global_id.as_deref()?, l)))
        .collect();
    let mut solids: Vec<(&String, &ReferenceSolid)> = reference.solids.iter().collect();
    solids.sort_by(|a, b| a.0.cmp(b.0));
    for (gid, solid) in solids {
        let line = lines.get(gid.as_str());
        if line.is_some_and(|l| l.checked_against.as_deref() == Some("reference")) {
            *report
                .reference_agree
                .entry(solid.kind.clone())
                .or_default() += 1;
        } else if REFERENCE_KINDS.contains(&solid.kind.as_str())
            && report.reference_unmatched.len() < 100
        {
            let why = match line {
                None => "no element of the project has its GlobalId".to_string(),
                Some(l) => l
                    .notes
                    .iter()
                    .rev()
                    .find(|n| n.starts_with("not built"))
                    .cloned()
                    .unwrap_or_else(|| format!("{} ({})", l.outcome, l.kind)),
            };
            report
                .reference_unmatched
                .push(format!("{} {} {gid}: {why}", solid.kind, solid.name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mitcad_model::testing::MockKernel;
    use mitcad_rvt::{RvtFile, testfile};

    #[test]
    fn the_test_project_comes_in_by_level() {
        let file = RvtFile::parse(testfile::project()).unwrap();
        let schema = file.schema().unwrap();
        let model = Model::read(&file, &schema).unwrap();
        let mut document = Document::new(MockKernel::default());
        let report =
            import_rvt(&mut document, "project.rvt", &model, &RvtOptions::default()).unwrap();
        // The column, the wall and the surface (a saved mesh); the levels,
        // the family, the types and the material stay out.
        assert_eq!(report.bodies, 3, "{report:?}");
        assert_eq!(report.meshes, 1);
        assert_eq!(report.oriented_boxes, 1);
        assert_eq!(report.boxes, 1);
        assert_eq!(report.left_out["types"], 2);
        assert!(report.left_out.contains_key("no records"));
        // One component per level that has elements, lowest first.
        let named: Vec<(&str, usize)> = report
            .levels
            .iter()
            .map(|l| (l.name.as_str(), l.bodies))
            .collect();
        assert_eq!(named, vec![("Ground", 2), ("Upper", 1)]);
        assert_eq!(report.levels[1].elevation, Some(10.0 * FOOT_MM));
        let bodies: Vec<String> = document.bodies().iter().map(|b| b.name.clone()).collect();
        assert!(
            bodies.contains(&format!("Square Column:24 x 24:{}", testfile::COLUMN)),
            "{bodies:?}"
        );
        assert!(bodies.contains(&format!("Basic Wall:Generic 8:{}", testfile::WALL)));
        // One undo step, and the timeline's notes.
        assert_eq!(document.undo_depth(), 1);
        assert_eq!(document.import_notes().count(), 2);
    }

    #[test]
    fn levels_of_one_name_come_in_once_per_elevation() {
        // Two levels named alike at one elevation, a third at another, and
        // a level named like the first one's numbered component (mitcad#310).
        let level = |id, name: &str, elevation| mitcad_rvt::elements::Level {
            id,
            name: name.into(),
            elevation,
            building_story: None,
        };
        let wall = |id, level, z: f64| Element {
            id,
            partition: Some(1),
            category: Some(categories::OST_WALLS),
            level: Some(level),
            bbox: Some([0.0, 0.0, z, 1.0, 1.0, z + 1.0]),
            ..Element::default()
        };
        let model = Model {
            levels: vec![
                level(10, "Story", 0.0),
                level(11, "Story", 0.0),
                level(12, "Story (1)", 5.0),
                level(13, "Story", 10.0),
            ],
            elements: vec![
                wall(1, 10, 0.0),
                wall(2, 11, 0.0),
                wall(3, 12, 5.0),
                wall(4, 13, 10.0),
            ],
            ..Model::default()
        };
        let mut document = Document::new(MockKernel::default());
        let report =
            import_rvt(&mut document, "project.rvt", &model, &RvtOptions::default()).unwrap();
        let named: Vec<(&str, usize, &[i64])> = report
            .levels
            .iter()
            .map(|l| (l.name.as_str(), l.bodies, l.merged.as_slice()))
            .collect();
        assert_eq!(
            named,
            vec![
                ("Story", 2, &[11][..]),
                ("Story (1)", 1, &[][..]),
                ("Story (2)", 1, &[][..]),
            ]
        );
        let features: Vec<&str> = document.features().map(|f| f.name.as_str()).collect();
        assert_eq!(
            features,
            vec![
                "Story level",
                "Story (1) level",
                "Story (2) level",
                "Story elements",
                "Story (1) elements",
                "Story (2) elements"
            ]
        );
        // Imported again: new components and features, numbered.
        let again =
            import_rvt(&mut document, "project.rvt", &model, &RvtOptions::default()).unwrap();
        assert_eq!(again.levels[0].name, "Story (3)");
        assert_eq!(document.features().count(), 12);
    }

    #[test]
    fn a_family_box_at_its_transform() {
        // A 2 x 2 x 9 box turned a quarter about z and moved.
        let t = [
            0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 10.0, 20.0, 5.0,
        ];
        let local = [-1.0, -1.0, 0.0, 1.0, 1.0, 9.0];
        assert_eq!(placed_bounds(local, &t), [9.0, 19.0, 5.0, 11.0, 21.0, 14.0]);
        let (frame, size) = placed_box(local, &t);
        assert_eq!(
            frame.origin,
            [11.0 * FOOT_MM, 19.0 * FOOT_MM, 5.0 * FOOT_MM]
        );
        assert_eq!(frame.x_axis, [0.0, 1.0, 0.0]);
        assert_eq!(size, [2.0 * FOOT_MM, 2.0 * FOOT_MM, 9.0 * FOOT_MM]);
        // Mirrored: the box spans the other way along y.
        let m = [1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
        let (frame, _) = placed_box([0.0, 0.0, 0.0, 1.0, 2.0, 3.0], &m);
        assert_eq!(frame.y_axis, [0.0, 1.0, 0.0]);
        assert_eq!(frame.origin, [0.0, -2.0 * FOOT_MM, 0.0]);
    }

    #[test]
    fn the_building_project_comes_in_as_features() {
        use mitcad_rvt::testfile::building as t;
        let read = || {
            let file = RvtFile::parse(t::project()).unwrap();
            let schema = file.schema().unwrap();
            Model::read(&file, &schema).unwrap()
        };
        let model = read();
        let kernel = MockKernel::default();
        kernel.measures.set(true);
        let mut document = Document::new(kernel);
        // The linked model: the same project, read again.
        let linked = LinkedModel {
            link: model.building.links[0].clone(),
            file: t::LINKED.into(),
            model: read(),
        };
        let options = RvtOptions {
            links: vec![linked],
            ..RvtOptions::default()
        };
        let report = import_rvt(&mut document, "building.rvt", &model, &options).unwrap();
        // The walls and the floor as features, checked against their stored
        // boxes (the mock kernel measures boxes).
        let line = |id| {
            report
                .elements
                .iter()
                .find(|l| l.id == id && l.link.is_none())
                .unwrap()
        };
        for id in [t::WALL_A, t::WALL_B, t::FLOOR] {
            let l = line(id);
            assert_eq!(l.outcome, "features", "{l:?}");
            assert_eq!(l.checked_against.as_deref(), Some("stored box"));
        }
        assert_eq!(line(t::WALL_A).notes, vec!["its start extends"]);
        assert_eq!(line(t::WALL_B).notes, vec!["its start stops"]);
        // Wall A runs on to wall B's far face, wall B stops at A's near face.
        let bodies = document.bodies();
        let body = |name: &str| {
            bodies
                .iter()
                .find(|b| b.name.starts_with(name))
                .unwrap()
                .shape
        };
        let kernel = document.kernel();
        let a = kernel
            .bounding_box(body("Basic Wall:Brick 6:30"))
            .unwrap()
            .unwrap();
        assert!((a.min[0] + 0.25 * FOOT_MM).abs() < 1e-6, "{a:?}");
        let b = kernel
            .bounding_box(body("Basic Wall:Brick 6:31"))
            .unwrap()
            .unwrap();
        assert!((b.min[1] - 0.25 * FOOT_MM).abs() < 1e-6, "{b:?}");
        assert_eq!(report.built["wall"], 4, "the project's and the link's");
        // The levels' planes, the grid's axis, the material's appearance.
        assert_eq!(report.level_planes, 4);
        assert_eq!(report.grid_axes, 2);
        assert_eq!(report.appearances, 2);
        // The link's component, at its transform, holds its model's levels.
        assert_eq!(report.links.len(), 1);
        assert_eq!(report.links[0].found.as_deref(), Some(t::LINKED));
        assert!(report.links[0].elements >= 3, "{:?}", report.links[0]);
        assert!(
            report.links[0]
                .note
                .as_deref()
                .unwrap()
                .contains("not followed")
        );
        assert!(report.levels.iter().any(|l| l.link == Some(0)));
        // A link not found: its component stays empty, the report names it.
        let mut document = Document::new(MockKernel::default());
        let missing = RvtOptions {
            missing_links: vec![(
                model.building.links[0].clone(),
                "linked.rvt was not found".into(),
            )],
            ..RvtOptions::default()
        };
        let report = import_rvt(&mut document, "building.rvt", &model, &missing).unwrap();
        assert_eq!(report.links[0].elements, 0);
        assert!(report.links[0].component.is_some());
        assert_eq!(
            report.links[0].note.as_deref(),
            Some("linked.rvt was not found")
        );
        // Without measures the features cannot be checked: the walls come in
        // as boxes, with the reason.
        assert_eq!(report.fallbacks["wall"], 2);
        let l = report.elements.iter().find(|l| l.id == t::WALL_A).unwrap();
        assert_eq!(l.outcome, "box");
        assert!(
            l.notes.last().unwrap().contains("encloses no volume"),
            "{l:?}"
        );
    }
}
