// SPDX-License-Identifier: MIT
//! The `.rvt` import, phase 1 (mitcad#274): a project's elements as
//! reference geometry, one component per level. Each building element
//! comes in as a body named `Family:Type:Id` (or by its class and id): the
//! triangle mesh its record saves where it has one (a mesh body), else its
//! bounding box: a family instance's type box at the instance's transform,
//! any other element's box in model coordinates. The elements' real shapes
//! (walls, floors, roofs, columns and beams rebuilt from their parameters)
//! are phase 2.
//!
//! Types, view-specific elements (annotations), members of groups'
//! definitions, elements without a box and elements of categories that are
//! no building elements (levels, grids, rooms, sketch lines, views) stay
//! out; the report counts them by reason. The reader is `mitcad_rvt`; the
//! geometry goes through the model's [`Kernel`] (`primitive`,
//! `triangle_body`).

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use mitcad_model::{
    BaseInput, ComponentUid, Document, FeatureUid, ImportBody, Kernel, ModelError, PrimitiveShape,
    PrimitiveSpec, SketchFrame, Transform,
};
use mitcad_rvt::categories;
use mitcad_rvt::elements::{Element, FOOT_MM, Model};

use crate::Progress;

/// Extents (feet) below which a box has no volume.
const THIN: f64 = 1e-6;

/// How far (feet) a family type's box at the instance's transform may lie
/// from the element's own box and still be used.
const BOX_TOLERANCE: f64 = 0.01;

/// A level's component in the report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LevelReport {
    /// The level element; None for the component of elements without one.
    pub id: Option<i64>,
    pub name: String,
    /// Millimetres.
    pub elevation: Option<f64>,
    pub component: Option<String>,
    pub feature: Option<String>,
    pub bodies: usize,
    pub meshes: usize,
    pub boxes: usize,
    /// The elements whose bodies it holds.
    #[serde(skip)]
    pub elements: Vec<i64>,
}

/// What the import did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub levels: Vec<LevelReport>,
    /// Bodies made: saved meshes, boxes at a transform, boxes in model
    /// coordinates.
    pub bodies: usize,
    pub meshes: usize,
    pub oriented_boxes: usize,
    pub boxes: usize,
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

/// Imports the elements of `model` (read from `file`) into the document:
/// one undo step. `progress` tells when the process runs low on memory: the
/// elements not yet built are then left out, and the report says so.
pub fn import_rvt<K: Kernel>(
    document: &mut Document<K>,
    file: &str,
    model: &Model,
    progress: Option<&Progress>,
) -> Result<Report, ModelError> {
    let mut report = Report::default();
    let label = format!("Import {file}");
    let depth = document.undo_depth();

    // The elements by level, in the table's order.
    let mut groups: BTreeMap<Option<i64>, Vec<&Element>> = BTreeMap::new();
    for e in &model.elements {
        if let Some(why) = left_out(model, e) {
            *report.left_out.entry(why).or_default() += 1;
            continue;
        }
        let level = level_of(model, e);
        if level.is_some_and(|(_, by_elevation)| by_elevation) {
            report.by_elevation += 1;
        }
        groups.entry(level.map(|(l, _)| l)).or_default().push(e);
    }
    let mut order: Vec<Option<i64>> = groups.keys().copied().collect();
    let elevation = |l: &Option<i64>| {
        l.and_then(|l| model.level(l))
            .map_or(f64::NEG_INFINITY, |l| l.elevation)
    };
    order.sort_by(|a, b| elevation(a).total_cmp(&elevation(b)));

    let mut inputs = Vec::new();
    let mut notes_of: Vec<String> = Vec::new();
    let mut stopped = false;
    for level in order {
        let elements = &groups[&level];
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
        let mut entry = LevelReport {
            id: level,
            name: name.clone(),
            elevation: level_elevation,
            ..LevelReport::default()
        };
        let mut bodies = Vec::new();
        for e in elements {
            if stopped || progress.is_some_and(Progress::is_memory_low) {
                stopped = true;
                report.memory_low += 1;
                continue;
            }
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
                    document
                        .kernel()
                        .triangle_body(&vertices, &triangles)
                        .map(|s| (s, 0))
                }
                Some(Shape::Oriented(frame, size)) => {
                    make_box(document, frame, size).map(|s| (s, 1))
                }
                Some(Shape::Aligned(frame, size)) => {
                    make_box(document, frame, size).map(|s| (s, 2))
                }
                None => continue,
            };
            match shape {
                Ok((shape, kind)) => {
                    match kind {
                        0 => {
                            entry.meshes += 1;
                            report.meshes += 1;
                        }
                        1 => {
                            entry.boxes += 1;
                            report.oriented_boxes += 1;
                        }
                        _ => {
                            entry.boxes += 1;
                            report.boxes += 1;
                        }
                    }
                    let category = e
                        .category_name
                        .map(str::to_string)
                        .or_else(|| e.category.map(|c| format!("category {c}")))
                        .unwrap_or_default();
                    *report.categories.entry(category).or_default() += 1;
                    entry.elements.push(e.id);
                    bodies.push(ImportBody {
                        name: Some(e.display_name()),
                        color: None,
                        shape,
                    });
                }
                Err(err) => {
                    if report.not_built.len() < 50 {
                        report.not_built.push((e.id, err.to_string()));
                    }
                }
            }
        }
        if bodies.is_empty() {
            report.levels.push(entry);
            continue;
        }
        let (component, _) =
            document.add_component(Some(&name), ComponentUid::ROOT, Transform::IDENTITY)?;
        entry.component = Some(component.to_string());
        entry.bodies = bodies.len();
        notes_of.push(format!(
            "The {} elements of {name}: {} saved meshes and {} boxes (a family type's box at its \
             instance's transform, or the element's box); the elements' own shapes are not read yet",
            bodies.len(),
            entry.meshes,
            entry.boxes
        ));
        inputs.push(BaseInput {
            name: Some(format!("{name} elements")),
            source: Some(format!("{file}: {name}")),
            component: Some(component),
            ..BaseInput::new(bodies)
        });
        report.levels.push(entry);
    }
    if stopped {
        report.warnings.push(format!(
            "the process ran low on memory: {} elements were left out",
            report.memory_low
        ));
    }
    report.bodies = report.meshes + report.oriented_boxes + report.boxes;
    if !inputs.is_empty() {
        let imported = document.add_base_features(&label, inputs)?;
        let mut feature_of: HashMap<usize, String> = HashMap::new();
        let mut notes = Vec::new();
        for (i, (imported, note)) in imported.iter().zip(notes_of).enumerate() {
            feature_of.insert(i, imported.feature.uid.to_string());
            notes.push((imported.feature.uid, note));
        }
        document.set_import_notes(&notes)?;
        for (i, entry) in report
            .levels
            .iter_mut()
            .filter(|l| l.bodies > 0)
            .enumerate()
        {
            entry.feature = feature_of.get(&i).cloned();
        }
    }
    document.merge_undo(depth, &label);
    Ok(report)
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
        let report = import_rvt(&mut document, "project.rvt", &model, None).unwrap();
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
}
