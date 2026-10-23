// SPDX-License-Identifier: MIT
//! A mock geometry kernel for the model tests. Shapes record the operations
//! that built them and carry face names and face-pair edges the way the
//! OCCT facade names them, so references, recompute and the body state can
//! be tested without OCCT. Tests steer booleans and failures through the
//! kernel's settings.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::datum::{CurveGeometry, SurfaceGeometry};
use crate::exchange::{BodyKind, ExportBody, ExportFormat, ExportOptions, ImportBody, ReadOptions};
use crate::ids::FeatureUid;
use crate::kernel::{
    Axis, Cylinder, ExtrudeFeatureSpec, ExtrudeSide, ExtrudeStart, HoleEnd, HoleSpec, Plane,
    RevolveSpec, SideEnd, Target, ThreadSpec, WallLocation, simple_extrude,
};
use crate::kernel::{
    BooleanOp, BooleanOutput, BooleanPiece, BoundingBox, Chamfer, ChamferCorner, ExtrudeSpec,
    Kernel, KernelError, MassProperties,
};
use crate::kernel::{DraftSpec, FilletSet, FilletSize, ShellSpec, ToolInput};
use crate::kernel::{FontGlyphs, FontRequest, Glyph};
use crate::monitor::RecomputeMonitor;
use crate::sweeps::{CoilSpec, LoftSection, LoftSpec, PipeSection, PipeSpec, RibSpec, SweepSpec};
use crate::sweeps::{LoftEndKind, PathCurve};
use crate::topo::{EdgeName, FaceName, RoleKey, SegmentKey};
use crate::transform::{Instance, PrimitiveShape, PrimitiveSpec, Transform};

#[derive(Debug, Clone, PartialEq)]
pub struct MockShape {
    /// The operations that built the shape, e.g. `cut(prism(F2:0..20),prism(F4:0..30))`.
    pub history: String,
    pub faces: Vec<FaceName>,
    /// Pairs of faces that meet in an edge.
    pub edges: Vec<(FaceName, FaceName)>,
    pub bounds: Option<BoundingBox>,
    /// The solids of a shape made of several, else empty.
    pub parts: Vec<MockShape>,
}

impl MockShape {
    fn has_face(&self, reference: &FaceName) -> bool {
        self.faces.iter().any(|f| f.matches(reference))
    }

    fn matching_edges(&self, edge: &EdgeName) -> Vec<&(FaceName, FaceName)> {
        let [a, b] = edge.faces();
        self.edges
            .iter()
            .filter(|(x, y)| (x.matches(a) && y.matches(b)) || (x.matches(b) && y.matches(a)))
            .collect()
    }

    fn merged(history: String, shapes: &[&MockShape], lost: &[FaceName]) -> MockShape {
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        let mut bounds: Option<BoundingBox> = None;
        for shape in shapes {
            faces.extend(shape.faces.iter().filter(|f| !lost.contains(f)).cloned());
            edges.extend(
                shape
                    .edges
                    .iter()
                    .filter(|(a, b)| !lost.contains(a) && !lost.contains(b))
                    .cloned(),
            );
            bounds = match (bounds, shape.bounds) {
                (Some(a), Some(b)) => Some(a.union(&b)),
                (a, b) => a.or(b),
            };
        }
        MockShape {
            history,
            faces,
            edges,
            bounds,
            parts: Vec::new(),
        }
    }
}

/// A box primitive's bottom plane: a point on it and its normal.
pub type BoxPlane = ([f64; 3], [f64; 3]);

#[derive(Default)]
pub struct MockKernel {
    /// Measures shapes (`mass_properties` of their bounding boxes; scaled
    /// patterns, mitcad#59); otherwise it measures nothing.
    pub measures: Cell<bool>,
    /// Operation name -> number of calls.
    pub calls: RefCell<BTreeMap<&'static str, usize>>,
    /// Operations that fail: "extrude", "join", "cut", "intersect", "fillet", "chamfer".
    pub fail: RefCell<BTreeSet<&'static str>>,
    /// A boolean fails only when one target has this exact shape history.
    pub boolean_fail_targets: RefCell<BTreeSet<String>>,
    /// A boolean fails when its tool has this exact shape history.
    pub boolean_fail_tools: RefCell<BTreeSet<String>>,
    /// Operations that fail as one whose allocation failed does (mitcad#80).
    pub out_of_memory: RefCell<BTreeSet<&'static str>>,
    /// Which targets a boolean touches, by index; all when None.
    pub touch: RefCell<Option<Vec<bool>>>,
    /// Pieces a cut or intersection leaves of each touched target: 1 unless
    /// set, 0 removes the target.
    pub split: Cell<Option<usize>>,
    /// Faces that booleans remove from their results.
    pub lose: RefCell<Vec<FaceName>>,
    /// Tools of faces are pockets (Cut) instead of bosses (Join).
    pub voids: Cell<bool>,
    /// What `face_geometry` returns, by face name (planar ends and starts
    /// of prisms otherwise).
    pub surfaces: RefCell<BTreeMap<String, SurfaceGeometry>>,
    /// Data exchange: the bodies `read_file` returns by path, and the files
    /// `write_file` wrote (path, format, body names and colours).
    pub files: RefCell<BTreeMap<String, Vec<ImportBody<MockShape>>>>,
    #[allow(clippy::type_complexity)]
    pub written: RefCell<Vec<(String, ExportFormat, Vec<(String, Option<[f64; 3]>)>)>>,
    /// The placements of each body of each `write_file` (mitcad#19).
    pub written_placements: RefCell<Vec<Vec<Vec<crate::exchange::BodyPlacement>>>>,
    /// The STEP and IGES unit of each `write_file` and the millimetres per
    /// unit of each `read_file` (P11: metric defaults).
    pub write_units: RefCell<Vec<crate::exchange::LengthUnit>>,
    pub read_units: RefCell<Vec<f64>>,
    /// Notes of the results of operations (P9): "fillet" -> what a smaller
    /// fillet would say; `notes` gives them for shapes the operation made.
    pub notes: RefCell<BTreeMap<&'static str, String>>,
    /// Profile features: what the extrude, revolve, hole and thread calls
    /// were asked for, one line each.
    pub specs: RefCell<Vec<String>>,
    /// Sketches (S1): what `curves_of` lists for any name on a shape that
    /// has faces.
    pub curves: RefCell<Vec<crate::kernel::Curve3>>,
    /// Paths of sweeps (F3): the curve `curves_of` gives for a name, before
    /// `curves`.
    pub named_curves: RefCell<BTreeMap<String, crate::kernel::Curve3>>,
    /// Recompute cancellation (P7): the operation that cancels the monitor
    /// when it is called, as a cancel that comes during an evaluation.
    pub cancel_on: RefCell<Option<(&'static str, Arc<RecomputeMonitor>)>>,
    /// Cancellation inside an operation (P7e): the monitor of the
    /// evaluation that runs (`interruptible`). With `stop_on_cancel`, an
    /// operation called once it is cancelled fails, as a kernel operation
    /// stopped inside.
    pub interrupt: RefCell<Option<Arc<RecomputeMonitor>>>,
    pub stop_on_cancel: Cell<bool>,
    /// 3MF export (mitcad#13): the tolerances `triangle_mesh` was asked for.
    pub mesh_tolerances: RefCell<Vec<crate::exchange::MeshTolerance>>,
    /// Joints (mitcad#55): what `edge_geometry` returns by edge name
    /// (lines otherwise), and `vertex_point` by vertex name.
    pub edge_curves: RefCell<BTreeMap<String, CurveGeometry>>,
    pub vertex_points: RefCell<BTreeMap<String, crate::datum::Vec3>>,
    /// Fillets repeated on a pattern's copies (mitcad#105): edge middles
    /// by edge name; `edge_middles` lists the shape's edges that have one
    /// (along y, 1 mm long).
    pub middles: RefCell<BTreeMap<String, [f64; 3]>>,
    /// Copies of added material that stand on nothing (`supported_copies`):
    /// their index among the copies of one call.
    pub unsupported: RefCell<BTreeSet<usize>>,
    /// Body edges and vertices in a sketch's plane (mitcad#256): the
    /// vertices `indexed_element` lists, by name and place, on any shape that
    /// has faces.
    pub vertices: RefCell<Vec<(String, [f64; 3])>>,
    /// Pieces a cut or intersection leaves of each touched target, as their
    /// bounding boxes in geometric order (mitcad#408): `#k` pieces with
    /// these bounds instead of `split`'s.
    pub piece_boxes: RefCell<Option<Vec<BoundingBox>>>,
    /// The radius of each hole wall `hole_tool` made, by face name, which
    /// `face_cylinder` gives such a wall (half the bounds' width otherwise;
    /// mitcad#436).
    pub hole_radii: RefCell<BTreeMap<String, f64>>,
    /// A cut by a box primitive takes the box as the half-space beyond its
    /// bottom face and clips the target's bounds by that plane (a derived
    /// part's end cuts, mitcad#443); the box planes are kept by history.
    pub half_space_cuts: Cell<bool>,
    pub box_planes: RefCell<BTreeMap<String, BoxPlane>>,
    /// A cut keeps its target's bounds: it takes material from inside the
    /// box, as a member's plumb cuts and birdsmouth notches do (mitcad#521).
    pub cuts_keep_bounds: Cell<bool>,
    /// A box's faces lie on its bounds (mitcad#524): `face_plane` gives
    /// `bottom` and `top` (-z, +z) and `side0` to `side3` (-y, +x, +y, -x)
    /// there, `face_boxes` each face's slab of the bounds, and
    /// `segment_crossings` where a segment enters and leaves the bounds.
    pub box_faces: Cell<bool>,
    /// A stitch's faces close up into a solid (mitcad#456); a surface body
    /// otherwise.
    pub stitch_closes: Cell<bool>,
    /// Faces (by name) a cut splits in two (mitcad#537): one piece at the
    /// centre of the tool's bounds, the other at its mirror image through
    /// the centre of the target's, numbered `#0` and `#1` in the order of
    /// those points (x, then y, then z) as the OCCT kernel numbers pieces
    /// in a geometric order; each piece meets the faces the face met.
    pub split_faces: RefCell<BTreeSet<String>>,
    /// The pieces' points by the bounds of the cut's result and the
    /// piece's name: `face_boxes` gives a piece the box of its point (and
    /// other faces the shape's bounds) when `box_faces` is off.
    pub piece_points: RefCell<Vec<(BoundingBox, String, [f64; 3])>>,
}

/// The bounds of what of `b` lies on or below the plane through `point`
/// with the normal `normal` (the corners kept and where the box's edges
/// cross the plane); None when nothing is left.
fn clipped(b: &BoundingBox, point: [f64; 3], normal: [f64; 3]) -> Option<BoundingBox> {
    let side = |p: [f64; 3]| (0..3).map(|i| (p[i] - point[i]) * normal[i]).sum::<f64>();
    let corners: Vec<[f64; 3]> = b.corners().collect();
    let mut kept: Vec<[f64; 3]> = corners
        .iter()
        .copied()
        .filter(|c| side(*c) <= 1e-9)
        .collect();
    for (i, a) in corners.iter().enumerate() {
        for (j, c) in corners.iter().enumerate().skip(i + 1) {
            // An edge: corners that differ in one axis.
            if (i ^ j).count_ones() != 1 {
                continue;
            }
            let (sa, sc) = (side(*a), side(*c));
            if (sa < 0.0 && sc > 0.0) || (sa > 0.0 && sc < 0.0) {
                let t = sa / (sa - sc);
                kept.push(std::array::from_fn(|k| a[k] + (c[k] - a[k]) * t));
            }
        }
    }
    let first = *kept.first()?;
    Some(kept.iter().fold(
        BoundingBox {
            min: first,
            max: first,
        },
        |acc, p| BoundingBox {
            min: std::array::from_fn(|k| acc.min[k].min(p[k])),
            max: std::array::from_fn(|k| acc.max[k].max(p[k])),
        },
    ))
}

impl MockShape {
    /// A body of `faces` faces in a ring, as an importer would build it;
    /// `history` "mesh..." makes a mesh body.
    pub fn imported(history: &str, faces: usize) -> MockShape {
        let face =
            |i: usize| FaceName::new(FeatureUid(0), "import", Some(RoleKey::Index(i as u32)));
        MockShape {
            history: history.to_owned(),
            faces: (0..faces).map(face).collect(),
            edges: (0..faces)
                .map(|i| (face(i), face((i + 1) % faces)))
                .collect(),
            bounds: Some(BoundingBox {
                min: [0.0; 3],
                max: [10.0; 3],
            }),
            parts: Vec::new(),
        }
    }
}

impl MockKernel {
    fn call(&self, op: &'static str) -> Result<(), KernelError> {
        *self.calls.borrow_mut().entry(op).or_default() += 1;
        if let Some((on, monitor)) = &*self.cancel_on.borrow()
            && *on == op
        {
            monitor.cancel();
        }
        if self.stop_on_cancel.get()
            && self
                .interrupt
                .borrow()
                .as_ref()
                .is_some_and(|monitor| monitor.is_cancelled())
        {
            return Err(KernelError::failed(format!(
                "{op}: the operation was cancelled"
            )));
        }
        if self.out_of_memory.borrow().contains(op) {
            Err(KernelError::failed(format!(
                "{op}: {}",
                KernelError::OUT_OF_MEMORY
            )))
        } else if self.fail.borrow().contains(op) {
            Err(KernelError::failed(format!("{op} failed")))
        } else {
            Ok(())
        }
    }

    pub fn count(&self, op: &str) -> usize {
        self.calls.borrow().get(op).copied().unwrap_or(0)
    }

    /// The cut's result with the faces of `split_faces` split in two.
    fn split_named_faces(&self, shape: &mut MockShape, target: &MockShape, tool: &MockShape) {
        let split = self.split_faces.borrow();
        if split.is_empty() {
            return;
        }
        let centre =
            |b: &BoundingBox| -> [f64; 3] { std::array::from_fn(|i| (b.min[i] + b.max[i]) / 2.0) };
        let (Some(target_box), Some(tool_box), Some(bounds)) =
            (target.bounds, tool.bounds, shape.bounds)
        else {
            return;
        };
        let (c, t) = (centre(&target_box), centre(&tool_box));
        let mut points = [t, std::array::from_fn(|i| 2.0 * c[i] - t[i])];
        points.sort_by(|a, b| {
            a.iter()
                .zip(b)
                .map(|(x, y)| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal))
                .find(|o| o.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let faces: Vec<FaceName> = shape
            .faces
            .iter()
            .filter(|f| split.contains(&f.to_string()))
            .cloned()
            .collect();
        for face in faces {
            let pieces = [face.clone().piece(0), face.clone().piece(1)];
            shape.faces.retain(|f| *f != face);
            shape.faces.extend(pieces.iter().cloned());
            let mut edges = Vec::new();
            for (a, b) in std::mem::take(&mut shape.edges) {
                if a == face || b == face {
                    for piece in &pieces {
                        let other = if a == face { b.clone() } else { a.clone() };
                        edges.push((piece.clone(), other));
                    }
                } else {
                    edges.push((a, b));
                }
            }
            shape.edges = edges;
            for (piece, point) in pieces.iter().zip(points) {
                self.piece_points
                    .borrow_mut()
                    .push((bounds, piece.to_string(), point));
            }
        }
    }

    fn dress(
        &self,
        op: &'static str,
        role: fn(FeatureUid, EdgeName) -> FaceName,
        feature: FeatureUid,
        body: &MockShape,
        edges: &[EdgeName],
        size: String,
    ) -> Result<MockShape, KernelError> {
        self.call(op)?;
        let mut shape = MockShape {
            history: format!("{op}({},{size})", body.history),
            ..body.clone()
        };
        for edge in edges {
            let matched: Vec<_> = body.matching_edges(edge).into_iter().cloned().collect();
            if matched.is_empty() {
                return Err(KernelError::failed(format!("the body has no edge {edge}")));
            }
            for (a, b) in matched {
                let face = role(feature, EdgeName::new(a.clone(), b.clone()));
                // The new face meets both faces of the edge and the faces at
                // its ends (those next to both).
                let next_to = |x: &FaceName| -> Vec<FaceName> {
                    body.edges
                        .iter()
                        .filter_map(|(p, q)| {
                            if p == x {
                                Some(q.clone())
                            } else if q == x {
                                Some(p.clone())
                            } else {
                                None
                            }
                        })
                        .collect()
                };
                let ends: Vec<FaceName> = next_to(&a)
                    .into_iter()
                    .filter(|f| next_to(&b).contains(f) && *f != a && *f != b)
                    .collect();
                shape.edges.retain(|e| *e != (a.clone(), b.clone()));
                for neighbour in [a, b].into_iter().chain(ends) {
                    shape.edges.push((face.clone(), neighbour));
                }
                shape.faces.push(face);
            }
        }
        Ok(shape)
    }
}

fn number(value: f64) -> String {
    format!("{value}")
}

/// The body's faces the references match.
fn faces_matching(body: &MockShape, references: &[FaceName]) -> Vec<FaceName> {
    body.faces
        .iter()
        .filter(|f| references.iter().any(|r| f.matches(r)))
        .cloned()
        .collect()
}

/// The edges of fillet or chamfer sets: the named ones and those of the faces.
fn set_edges<'a>(
    body: &MockShape,
    sets: impl Iterator<Item = (&'a [EdgeName], &'a [FaceName])>,
) -> Vec<EdgeName> {
    let mut edges = Vec::new();
    for (named, faces) in sets {
        edges.extend(named.iter().cloned());
        for (a, b) in &body.edges {
            if faces.iter().any(|f| a.matches(f) || b.matches(f)) {
                edges.push(EdgeName::new(a.clone(), b.clone()));
            }
        }
    }
    edges
}

impl Kernel for MockKernel {
    type Shape = MockShape;

    fn sheet_shape(
        &self,
        spec: &crate::sheet::SheetShapeSpec<'_>,
        _prefix: &str,
    ) -> Result<crate::sheet::SheetGeometry<MockShape>, KernelError> {
        self.call("sheet_shape")?;
        let mut solids = Vec::new();
        for patch in spec.patches {
            let crate::sheet::SheetEmbedding::Plane {
                front_frame: frame,
                front_to_back_sign,
            } = &patch.embedding
            else {
                return Err(KernelError::Unsupported("mock cylindrical sheet"));
            };
            let start = 0.0;
            let mut points = Vec::new();
            for segment in patch
                .regions
                .iter()
                .flat_map(|r| &r.loops)
                .flat_map(|l| &l.edges)
            {
                let key = match &segment.piece.curve {
                    crate::sheet::SheetCurveId::Sketch { segment, .. } => segment.clone(),
                    crate::sheet::SheetCurveId::Generated { .. } => "c1".parse().expect("segment"),
                };
                let region = crate::profile::ProfileRegion {
                    key: "r{c1}".parse().expect("region"),
                    loops: vec![crate::profile::ProfileLoop {
                        segments: vec![crate::profile::ProfileSegment {
                            key,
                            geometry: segment.curve.geometry.clone(),
                        }],
                    }],
                };
                let (min, max) = region.bounds();
                for x in [min[0], max[0]] {
                    for y in [min[1], max[1]] {
                        for z in [start, start + spec.thickness] {
                            let p = frame.point([x, y]);
                            points.push(std::array::from_fn(|i| {
                                p[i] + z * front_to_back_sign.sign() * frame.normal()[i]
                            }));
                        }
                    }
                }
            }
            solids.push(crate::sheet::SheetSolid {
                membership: (0..patch.regions.len())
                    .map(|region| crate::sheet::RegionMembership {
                        domain: patch.domain,
                        region,
                    })
                    .collect(),
                shape: MockShape {
                    history: format!(
                        "sheet({:?}:{:?}:{start}..{})",
                        patch.domain,
                        frame,
                        start + spec.thickness
                    ),
                    faces: ["sheet_front", "sheet_back"]
                        .iter()
                        .map(|role| {
                            format!("{}:{role}({})", patch.domain.feature(), patch.domain.slot())
                                .parse()
                                .expect("sheet face name")
                        })
                        .collect(),
                    edges: Vec::new(),
                    parts: Vec::new(),
                    bounds: Some(BoundingBox {
                        min: std::array::from_fn(|i| {
                            points
                                .iter()
                                .map(|p: &[f64; 3]| p[i])
                                .fold(f64::INFINITY, f64::min)
                        }),
                        max: std::array::from_fn(|i| {
                            points
                                .iter()
                                .map(|p| p[i])
                                .fold(f64::NEG_INFINITY, f64::max)
                        }),
                    }),
                },
            });
        }
        Ok(crate::sheet::SheetGeometry {
            solids,
            diagnostics: Vec::new(),
        })
    }

    fn extrude(&self, spec: &ExtrudeSpec<'_>) -> Result<MockShape, KernelError> {
        self.call("extrude")?;
        let feature = spec.feature;
        let mut parts = Vec::new();
        for region in spec.regions {
            let start = FaceName::start(feature, region.key.clone());
            let end = FaceName::end(feature, region.key.clone());
            let mut faces = vec![start.clone(), end.clone()];
            let mut edges = Vec::new();
            for profile_loop in &region.loops {
                let sides: Vec<FaceName> = profile_loop
                    .segments
                    .iter()
                    .map(|s| FaceName::side(feature, s.key.clone()))
                    .collect();
                for (i, side) in sides.iter().enumerate() {
                    // Neighbours around the loop; a loop of one closed curve has a seam.
                    edges.push((side.clone(), sides[(i + 1) % sides.len()].clone()));
                    edges.push((side.clone(), start.clone()));
                    edges.push((side.clone(), end.clone()));
                }
                faces.extend(sides);
            }
            let (min, max) = region.bounds();
            // Profiles are in sketch coordinates; both their frame and
            // the sweep direction are in the body's component space.
            let mut corners: Vec<[f64; 3]> = Vec::with_capacity(8);
            for u in [min[0], max[0]] {
                for v in [min[1], max[1]] {
                    let point = spec.frame.point([u, v]);
                    for offset in [spec.start, spec.end] {
                        corners.push(std::array::from_fn(|i| {
                            point[i] + offset * spec.direction[i]
                        }));
                    }
                }
            }
            parts.push(MockShape {
                history: format!(
                    "prism({feature}:{}..{})",
                    number(spec.start),
                    number(spec.end)
                ),
                faces,
                edges,
                bounds: Some(BoundingBox {
                    min: std::array::from_fn(|i| {
                        corners.iter().map(|c| c[i]).fold(f64::INFINITY, f64::min)
                    }),
                    max: std::array::from_fn(|i| {
                        corners
                            .iter()
                            .map(|c| c[i])
                            .fold(f64::NEG_INFINITY, f64::max)
                    }),
                }),
                parts: Vec::new(),
            });
        }
        if parts.len() == 1 {
            return Ok(parts.remove(0));
        }
        let all: Vec<&MockShape> = parts.iter().collect();
        let mut shape = MockShape::merged(
            format!(
                "prism({feature}:{}..{})",
                number(spec.start),
                number(spec.end)
            ),
            &all,
            &[],
        );
        shape.parts = parts;
        Ok(shape)
    }

    fn solids(&self, shape: &MockShape) -> Result<Vec<MockShape>, KernelError> {
        Ok(if shape.parts.is_empty() {
            vec![shape.clone()]
        } else {
            shape.parts.clone()
        })
    }

    fn boolean(
        &self,
        op: BooleanOp,
        targets: &[&MockShape],
        tool: &MockShape,
    ) -> Result<BooleanOutput<MockShape>, KernelError> {
        let name = match op {
            BooleanOp::Join => "join",
            BooleanOp::Cut => "cut",
            BooleanOp::Intersect => "intersect",
        };
        self.call(name)?;
        if targets
            .iter()
            .any(|shape| self.boolean_fail_targets.borrow().contains(&shape.history))
        {
            return Err(KernelError::failed("target Boolean failed"));
        }
        if self.boolean_fail_tools.borrow().contains(&tool.history) {
            return Err(KernelError::failed("tool Boolean failed"));
        }
        let touched = self
            .touch
            .borrow()
            .clone()
            .unwrap_or_else(|| vec![true; targets.len()]);
        let lost = self.lose.borrow().clone();
        let mut pieces = Vec::new();
        if op == BooleanOp::Join {
            let sources: Vec<usize> = (0..targets.len()).filter(|i| touched[*i]).collect();
            if !sources.is_empty() {
                let mut inputs: Vec<&MockShape> = sources.iter().map(|i| targets[*i]).collect();
                inputs.push(tool);
                let history = format!(
                    "join({})",
                    inputs
                        .iter()
                        .map(|s| s.history.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                );
                pieces.push(BooleanPiece {
                    shape: MockShape::merged(history, &inputs, &lost),
                    sources,
                });
            }
        } else {
            let verb = if op == BooleanOp::Cut {
                "cut"
            } else {
                "common"
            };
            let boxes = self.piece_boxes.borrow().clone();
            for (i, target) in targets.iter().enumerate() {
                if !touched[i] {
                    continue;
                }
                if let Some(boxes) = &boxes {
                    for (k, bounds) in boxes.iter().enumerate() {
                        let history = format!("{verb}({},{})#{k}", target.history, tool.history);
                        let mut shape = MockShape::merged(history, &[target, tool], &lost);
                        shape.bounds = Some(*bounds);
                        pieces.push(BooleanPiece {
                            shape,
                            sources: vec![i],
                        });
                    }
                    continue;
                }
                let count = self.split.get().unwrap_or(1);
                let half_space = if op == BooleanOp::Cut && self.half_space_cuts.get() {
                    self.box_planes.borrow().get(&tool.history).copied()
                } else {
                    None
                };
                for k in 0..count {
                    let mut history = format!("{verb}({},{})", target.history, tool.history);
                    if count > 1 {
                        history.push_str(&format!("#{k}"));
                    }
                    let mut shape = MockShape::merged(history, &[target, tool], &lost);
                    if let Some((point, normal)) = half_space {
                        shape.bounds = target.bounds.and_then(|b| clipped(&b, point, normal));
                    }
                    if op == BooleanOp::Cut && self.cuts_keep_bounds.get() {
                        shape.bounds = target.bounds;
                    }
                    if op == BooleanOp::Cut {
                        self.split_named_faces(&mut shape, target, tool);
                    }
                    pieces.push(BooleanPiece {
                        shape,
                        sources: vec![i],
                    });
                }
            }
        }
        Ok(BooleanOutput { pieces, touched })
    }

    fn fillet(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        sets: &[FilletSet<'_>],
        rolling_ball_corners: bool,
    ) -> Result<MockShape, KernelError> {
        let sizes: Vec<String> = sets
            .iter()
            .map(|set| {
                let size = match set.size {
                    FilletSize::Constant { radius } => number(radius),
                    size => format!("{size:?}"),
                };
                if set.curvature {
                    format!("{size},G2 weight {}", number(set.weight))
                } else {
                    size
                }
            })
            .collect();
        let corners = if rolling_ball_corners { "" } else { ",setback" };
        let edges = set_edges(body, sets.iter().map(|s| (s.edges, s.faces)));
        self.dress(
            "fillet",
            FaceName::fillet,
            feature,
            body,
            &edges,
            format!("{}{corners}", sizes.join(";")),
        )
    }

    fn chamfer(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        sets: &[Chamfer<'_>],
        corner: ChamferCorner,
    ) -> Result<MockShape, KernelError> {
        let sizes: Vec<String> = sets
            .iter()
            .map(|set| {
                format!(
                    "{:?}{}{}",
                    set.size,
                    set.reference.map_or(String::new(), |f| format!(",on {f}")),
                    if set.flip { ",flip" } else { "" }
                )
            })
            .collect();
        let edges = set_edges(body, sets.iter().map(|s| (s.edges, s.faces)));
        let corner = match corner {
            ChamferCorner::Chamfer => String::new(),
            other => format!(",{other:?} corners"),
        };
        self.dress(
            "chamfer",
            FaceName::chamfer,
            feature,
            body,
            &edges,
            format!("{}{corner}", sizes.join(";")),
        )
    }

    // Face operations (F2): the history records the operation; new faces
    // get the names the OCCT kernel gives them.

    fn shell(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        spec: &ShellSpec<'_>,
    ) -> Result<MockShape, KernelError> {
        self.call("shell")?;
        let mut shape = MockShape::merged(
            format!(
                "shell({},{},{})",
                body.history,
                number(spec.inside),
                number(spec.outside)
            ),
            &[body],
            &faces_matching(body, spec.faces),
        );
        for face in &body.faces {
            if !spec.faces.iter().any(|r| face.matches(r)) {
                shape.faces.push(FaceName::new(
                    feature,
                    "offset",
                    Some(RoleKey::Face(Box::new(face.clone()))),
                ));
            }
        }
        Ok(shape)
    }

    fn draft(
        &self,
        _feature: FeatureUid,
        body: &MockShape,
        spec: &DraftSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("draft")?;
        Ok(MockShape {
            history: format!("draft({},{})", body.history, number(spec.angle)),
            ..body.clone()
        })
    }

    fn offset_faces(
        &self,
        _feature: FeatureUid,
        body: &MockShape,
        _faces: &[FaceName],
        distance: f64,
    ) -> Result<MockShape, KernelError> {
        self.call("offset_faces")?;
        Ok(MockShape {
            history: format!("offset({},{})", body.history, number(distance)),
            ..body.clone()
        })
    }

    fn delete_faces(
        &self,
        _feature: FeatureUid,
        body: &MockShape,
        faces: &[FaceName],
    ) -> Result<MockShape, KernelError> {
        self.call("delete_faces")?;
        Ok(MockShape::merged(
            format!("delete({})", body.history),
            &[body],
            &faces_matching(body, faces),
        ))
    }

    fn split_body(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        _tool: &ToolInput<'_, MockShape>,
        _extend: bool,
    ) -> Result<Vec<MockShape>, KernelError> {
        self.call("split_body")?;
        let count = self.split.get().unwrap_or(2);
        Ok((0..count)
            .map(|k| {
                let mut piece = MockShape {
                    history: format!("split({})#{k}", body.history),
                    ..body.clone()
                };
                piece.faces.push(FaceName::new(feature, "split", None));
                piece
            })
            .collect())
    }

    fn split_faces(
        &self,
        _feature: FeatureUid,
        body: &MockShape,
        faces: &[FaceName],
        _tool: &ToolInput<'_, MockShape>,
        _extend: bool,
    ) -> Result<MockShape, KernelError> {
        self.call("split_faces")?;
        let mut shape = MockShape {
            history: format!("split_faces({})", body.history),
            ..body.clone()
        };
        for face in faces_matching(body, faces) {
            shape.faces.retain(|f| *f != face);
            shape.faces.push(face.clone().piece(0));
            shape.faces.push(face.piece(1));
        }
        Ok(shape)
    }

    fn count_edges(&self, shape: &MockShape, edge: &EdgeName) -> usize {
        let count = shape.matching_edges(edge).len();
        match edge.index {
            Some(k) => usize::from(k < count as u32),
            None => count,
        }
    }

    fn count_faces(&self, shape: &MockShape, face: &FaceName) -> Result<usize, KernelError> {
        Ok(usize::from(shape.has_face(face)))
    }

    fn faces(&self, shape: &MockShape) -> Result<Vec<crate::kernel::FaceInfo>, KernelError> {
        Ok(shape
            .faces
            .iter()
            .map(|face| crate::kernel::FaceInfo {
                names: vec![face.to_string()],
                surface: "plane".to_owned(),
                area: 0.0,
            })
            .collect())
    }

    /// The face pairs as named edges (mitcad#256), numbered where the same
    /// faces meet more than once.
    fn edges(&self, shape: &MockShape) -> Result<Vec<crate::kernel::EdgeInfo>, KernelError> {
        let names: Vec<EdgeName> = shape
            .edges
            .iter()
            .map(|(a, b)| EdgeName::new(a.clone(), b.clone()))
            .collect();
        Ok(names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let repeats = names.iter().filter(|n| *n == name).count();
                let before = names[..i].iter().filter(|n| *n == name).count() as u32;
                let named = if repeats > 1 {
                    name.clone().with_index(before)
                } else {
                    name.clone()
                };
                crate::kernel::EdgeInfo {
                    name: Some(named.to_string()),
                    curve: "line".to_owned(),
                    length: 0.0,
                }
            })
            .collect())
    }

    fn bounding_box(&self, shape: &MockShape) -> Result<Option<BoundingBox>, KernelError> {
        Ok(shape.bounds)
    }

    /// The bounding box's volume and centre, when [`MockKernel::measures`].
    fn mass_properties(&self, shape: &MockShape) -> Result<MassProperties, KernelError> {
        if !self.measures.get() {
            return Err(KernelError::Unsupported("mass properties"));
        }
        let b = shape
            .bounds
            .ok_or(KernelError::Unsupported("mass properties without bounds"))?;
        let size: [f64; 3] = std::array::from_fn(|i| b.max[i] - b.min[i]);
        Ok(MassProperties {
            volume: size[0] * size[1] * size[2],
            area: 2.0 * (size[0] * size[1] + size[1] * size[2] + size[2] * size[0]),
            center: std::array::from_fn(|i| (b.min[i] + b.max[i]) / 2.0),
        })
    }

    fn curves_of(
        &self,
        shape: &MockShape,
        name: &crate::topo::TopoName,
    ) -> Result<Vec<crate::kernel::Curve3>, KernelError> {
        self.call("curves_of")?;
        if let Some(curve) = self.named_curves.borrow().get(&name.to_string()) {
            return Ok(vec![curve.clone()]);
        }
        Ok(if shape.faces.is_empty() {
            Vec::new()
        } else {
            self.curves.borrow().clone()
        })
    }

    // FreeCAD import: faces and edges in the shape's order, their curves as
    // `curves_of` gives them; vertices only as `vertices` lists them
    // (mitcad#256).
    fn indexed_element(
        &self,
        shape: &MockShape,
        kind: crate::kernel::ElementKind,
        index: usize,
    ) -> Result<Option<crate::kernel::IndexedElement>, KernelError> {
        use crate::kernel::ElementKind;
        let name = match kind {
            ElementKind::Face => shape
                .faces
                .get(index)
                .map(|f| crate::topo::TopoName::Face(f.clone())),
            ElementKind::Edge => shape
                .edges
                .get(index)
                .map(|(a, b)| crate::topo::TopoName::Edge(EdgeName::new(a.clone(), b.clone()))),
            ElementKind::Vertex => {
                if shape.faces.is_empty() {
                    return Ok(None);
                }
                return Ok(self.vertices.borrow().get(index).map(|(name, at)| {
                    crate::kernel::IndexedElement {
                        name: Some(name.clone()),
                        curves: vec![crate::kernel::Curve3::Point(*at)],
                    }
                }));
            }
        };
        let Some(name) = name else {
            return Ok(None);
        };
        Ok(Some(crate::kernel::IndexedElement {
            curves: self.curves_of(shape, &name)?,
            name: Some(name.to_string()),
        }))
    }

    // Data exchange: B-rep data is "<faces> <history>".

    fn import_brep(
        &self,
        feature: FeatureUid,
        data: &[u8],
        first_face: u32,
    ) -> Result<(MockShape, u32), KernelError> {
        self.call("import")?;
        read_mock_brep(feature, data, first_face)
    }

    fn brep_data(&self, shape: &MockShape) -> Result<Vec<u8>, KernelError> {
        Ok(format!("{} {}", shape.faces.len(), shape.history).into_bytes())
    }

    fn compound(&self, shapes: &[MockShape]) -> Result<MockShape, KernelError> {
        let all: Vec<&MockShape> = shapes.iter().collect();
        let history: Vec<&str> = shapes.iter().map(|s| s.history.as_str()).collect();
        let mut shape = MockShape::merged(format!("compound({})", history.join(",")), &all, &[]);
        shape.parts = shapes.to_vec();
        Ok(shape)
    }

    fn body_kind(&self, shape: &MockShape) -> Result<BodyKind, KernelError> {
        Ok(if shape.faces.is_empty() {
            BodyKind::Empty
        } else if shape.history.starts_with("mesh") || shape.history.contains(",mesh") {
            BodyKind::Mesh
        } else if shape.history.contains(",sheet") {
            // An imported surface body ("import(F1:0..4,sheet)").
            BodyKind::Sheet
        } else {
            BodyKind::Solid
        })
    }

    fn read_file(
        &self,
        path: &str,
        options: &ReadOptions,
    ) -> Result<Vec<ImportBody<MockShape>>, KernelError> {
        self.read_units.borrow_mut().push(options.unit_mm);
        self.files
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| KernelError::failed(format!("cannot read {path}")))
    }

    fn write_file(
        &self,
        path: &str,
        bodies: &[ExportBody<'_, MockShape>],
        options: &ExportOptions,
    ) -> Result<(), KernelError> {
        self.call("write")?;
        self.write_units.borrow_mut().push(options.unit);
        self.written_placements
            .borrow_mut()
            .push(bodies.iter().map(|b| b.placements.to_vec()).collect());
        self.written.borrow_mut().push((
            path.to_owned(),
            options.format,
            bodies
                .iter()
                .map(|b| (b.name.to_owned(), b.color))
                .collect(),
        ));
        Ok(())
    }

    // Transforms, patterns, mirrors, combine and primitives (F4).

    fn transform_shape(
        &self,
        shape: &MockShape,
        transform: &Transform,
        instance: Option<Instance>,
    ) -> Result<MockShape, KernelError> {
        self.call("transform")?;
        Ok(transformed(shape, transform, instance))
    }

    fn unite(&self, shapes: &[&MockShape]) -> Result<MockShape, KernelError> {
        self.call("unite")?;
        let history = format!(
            "unite({})",
            shapes
                .iter()
                .map(|s| s.history.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut united = MockShape::merged(history, shapes, &[]);
        // The shapes stay separate solids, as if they did not touch.
        if shapes.len() > 1 {
            united.parts = shapes.iter().map(|s| (*s).clone()).collect();
        }
        Ok(united)
    }

    fn face_tool(
        &self,
        body: &MockShape,
        faces: &[FaceName],
    ) -> Result<(MockShape, BooleanOp), KernelError> {
        self.call("face_tool")?;
        let mut tool = MockShape {
            history: format!("faces({})", body.history),
            faces: Vec::new(),
            edges: Vec::new(),
            bounds: body.bounds,
            parts: Vec::new(),
        };
        for face in faces {
            let found: Vec<FaceName> = body
                .faces
                .iter()
                .filter(|f| f.matches(face))
                .cloned()
                .collect();
            if found.is_empty() {
                return Err(KernelError::failed(format!("the body has no face {face}")));
            }
            tool.faces.extend(found);
        }
        let op = if self.voids.get() {
            BooleanOp::Cut
        } else {
            BooleanOp::Join
        };
        Ok((tool, op))
    }

    /// The surfaces set in `surfaces`; else ends and starts of prisms are
    /// planes, as `face_plane` has them, and other faces unsupported.
    fn face_geometry(
        &self,
        shape: &MockShape,
        face: &FaceName,
    ) -> Result<SurfaceGeometry, KernelError> {
        if let Some(surface) = self.surfaces.borrow().get(&face.to_string()) {
            return Ok(surface.clone());
        }
        if !matches!(plain_role(face), "end" | "start" | "end_a" | "end_b") {
            return Err(KernelError::Unsupported("face geometry"));
        }
        let plane = self.face_plane(shape, face)?;
        Ok(SurfaceGeometry::Plane {
            origin: plane.origin,
            normal: plane.normal,
        })
    }

    fn primitive(&self, spec: &PrimitiveSpec) -> Result<MockShape, KernelError> {
        self.call("primitive")?;
        let feature = spec.feature;
        let (kind, size, roles): (&str, [f64; 3], &[&str]) = match spec.shape {
            PrimitiveShape::Box {
                length,
                width,
                height,
            } => (
                "box",
                [length, width, height],
                &["bottom", "top", "side0", "side1", "side2", "side3"],
            ),
            PrimitiveShape::Cylinder { radius, height } => (
                "cylinder",
                [radius, radius, height],
                &["bottom", "top", "side0"],
            ),
            PrimitiveShape::Sphere { radius } => ("sphere", [radius; 3], &["side0"]),
            PrimitiveShape::Torus {
                major_radius,
                minor_radius,
            } => (
                "torus",
                [major_radius, minor_radius, minor_radius],
                &["side0"],
            ),
        };
        let faces: Vec<FaceName> = roles
            .iter()
            .map(|role| FaceName::new(feature, role, None))
            .collect();
        let edges = faces
            .iter()
            .enumerate()
            .flat_map(|(i, a)| faces[i + 1..].iter().map(move |b| (a.clone(), b.clone())))
            .collect();
        let o = spec.frame.origin;
        let history = format!(
            "{kind}({feature}:{}x{}x{}@{},{},{})",
            number(size[0]),
            number(size[1]),
            number(size[2]),
            number(o[0]),
            number(o[1]),
            number(o[2])
        );
        if kind == "box" && self.half_space_cuts.get() {
            let (x, y) = (spec.frame.x_axis, spec.frame.y_axis);
            let normal = [
                x[1] * y[2] - x[2] * y[1],
                x[2] * y[0] - x[0] * y[2],
                x[0] * y[1] - x[1] * y[0],
            ];
            self.box_planes
                .borrow_mut()
                .insert(history.clone(), (o, normal));
        }
        Ok(MockShape {
            history,
            faces,
            edges,
            bounds: Some(BoundingBox {
                min: o,
                max: std::array::from_fn(|i| o[i] + size[i]),
            }),
            parts: Vec::new(),
        })
    }

    // Profile features (F1).

    fn extrude_feature(
        &self,
        spec: &ExtrudeFeatureSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.specs.borrow_mut().push(describe_extrude(spec));
        if let Ok(shape) = simple_extrude(self, spec) {
            return Ok(shape);
        }
        // A prism along the first side's distance (or 10 up to a target).
        let length = match spec.side1.end {
            SideEnd::Distance(d) => d,
            SideEnd::Target(_) => 10.0,
        };
        let extrusion = ExtrudeSpec {
            feature: spec.feature,
            frame: spec.frame,
            regions: spec.regions,
            direction: spec.direction,
            start: 0.0,
            end: length,
        };
        self.extrude(&extrusion)
    }

    fn revolve(&self, spec: &RevolveSpec<'_, MockShape>) -> Result<MockShape, KernelError> {
        self.call("revolve")?;
        self.specs.borrow_mut().push(format!(
            "revolve {} axis {:?} {:?} angles {} {:?} target {}",
            spec.feature,
            spec.axis.origin,
            spec.axis.direction,
            number(spec.angle1),
            spec.angle2,
            spec.target.is_some()
        ));
        let full = spec.target.is_none()
            && spec.angle1 + spec.angle2.unwrap_or(0.0) >= std::f64::consts::TAU - 1e-9;
        let history = format!("revolve({}:{})", spec.feature, number(spec.angle1));
        // Each region turns into a solid of its own, as an extrusion's do.
        let mut parts = Vec::new();
        for region in spec.regions {
            let mut faces = Vec::new();
            let mut edges = Vec::new();
            let start = FaceName::start(spec.feature, region.key.clone());
            let end = FaceName::end(spec.feature, region.key.clone());
            if !full {
                faces.extend([start.clone(), end.clone()]);
            }
            for segment in region.loops.iter().flat_map(|l| &l.segments) {
                let side = FaceName::side(spec.feature, segment.key.clone());
                if !full {
                    edges.push((side.clone(), start.clone()));
                    edges.push((side.clone(), end.clone()));
                }
                faces.push(side);
            }
            parts.push(MockShape {
                history: history.clone(),
                faces,
                edges,
                bounds: None,
                parts: Vec::new(),
            });
        }
        if parts.len() == 1 {
            return Ok(parts.remove(0));
        }
        let all: Vec<&MockShape> = parts.iter().collect();
        let mut shape = MockShape::merged(history, &all, &[]);
        shape.parts = parts;
        Ok(shape)
    }

    fn hole_tool(&self, spec: &HoleSpec<'_, MockShape>) -> Result<MockShape, KernelError> {
        self.call("hole")?;
        let end = match &spec.end {
            HoleEnd::Distance(d) => format!("depth {}", number(*d)),
            HoleEnd::ThroughAll => format!("through {} bodies", spec.bodies.len()),
            HoleEnd::Target(_) => "target".to_owned(),
        };
        let mut text = format!(
            "hole {} at {:?} diameter {:.6} {:?} tip {:?} {end}",
            spec.feature,
            spec.positions
                .iter()
                .map(|p| (p.origin, p.direction))
                .collect::<Vec<_>>(),
            spec.diameter,
            spec.shape,
            spec.tip_angle.map(number)
        );
        if spec.taper != 0.0 {
            text += &format!(" taper {}", number(spec.taper));
        }
        self.specs.borrow_mut().push(text);
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        for i in 0..spec.positions.len() {
            let face = |part: &str| FaceName::new(spec.feature, &format!("hole{i}.{part}"), None);
            if spec.taper == 0.0 {
                self.hole_radii
                    .borrow_mut()
                    .insert(face("wall").to_string(), spec.diameter / 2.0);
            }
            faces.extend([face("wall"), face("tip")]);
            edges.push((face("wall"), face("tip")));
        }
        Ok(MockShape {
            history: format!("hole({}:{})", spec.feature, spec.positions.len()),
            faces,
            edges,
            bounds: None,
            parts: Vec::new(),
        })
    }

    fn modeled_thread(
        &self,
        body: &MockShape,
        spec: &ThreadSpec<'_>,
    ) -> Result<MockShape, KernelError> {
        self.call("thread")?;
        // Extra clearance only when there is some (other specs stay as they were).
        let clearance = if spec.radial_clearance != 0.0 || spec.flank_clearance != 0.0 {
            format!(
                " clearance {:.4}/{:.4}",
                spec.radial_clearance, spec.flank_clearance
            )
        } else {
            String::new()
        };
        // End chamfers and cosmetic threads only when there are some.
        let mut clearance = clearance;
        if !spec.groove {
            clearance += " cosmetic";
        }
        if let Some(chamfer) = &spec.chamfer {
            clearance += &format!(
                " chamfer {:.4} deg {:.4} ends {:?}",
                chamfer.angle.to_degrees(),
                chamfer.diameter,
                chamfer.ends
            );
        }
        self.specs.borrow_mut().push(format!(
            "thread {} pitch {} diameters {:.4}/{:.4}/{:.4} angle {:.6} right {} part {:?}{clearance}",
            spec.faces
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            number(spec.pitch),
            spec.major,
            spec.minor,
            spec.pitch_diameter,
            spec.angle,
            spec.right_handed,
            spec.part
        ));
        let mut shape = MockShape {
            history: format!("thread({},{})", body.history, number(spec.pitch)),
            ..body.clone()
        };
        for (i, face) in spec.faces.iter().enumerate() {
            let named = |role: &str| {
                FaceName::new(
                    spec.feature,
                    role,
                    Some(RoleKey::Face(Box::new(face.clone()))),
                )
            };
            if spec.groove {
                shape.faces.push(named("thread"));
                // As the OCCT facade: a profile over the whole face whose
                // crest is off the face (a class's diameter, a radial
                // clearance) leaves no piece of it (mitcad#433).
                if spec.part.is_none()
                    && let Ok(cylinder) = self.face_cylinder(body, face)
                {
                    let crest = if cylinder.internal {
                        spec.minor / 2.0 + spec.radial_clearance
                    } else {
                        spec.major / 2.0 - spec.radial_clearance
                    };
                    if (crest - cylinder.radius).abs() > 1e-7 {
                        shape.faces.retain(|f| f != face);
                    }
                }
            }
            let ends = spec.chamfer.as_ref().and_then(|c| c.ends.get(i).copied());
            let (low, high) = ends.unwrap_or((false, false));
            if low {
                shape.faces.push(named("thread_chamfer_low"));
            }
            if high {
                shape.faces.push(named("thread_chamfer_high"));
            }
        }
        Ok(shape)
    }

    /// End caps lie on the top and bottom of the bounds.
    fn face_plane(&self, shape: &MockShape, face: &FaceName) -> Result<Plane, KernelError> {
        if !shape.has_face(face) {
            return Err(KernelError::failed(format!("the body has no face {face}")));
        }
        let bounds = shape.bounds.ok_or(KernelError::failed("no bounds"))?;
        if self.box_faces.get()
            && let Some((axis, high)) = box_side(plain_role(face))
        {
            let mut origin: [f64; 3] =
                std::array::from_fn(|i| (bounds.min[i] + bounds.max[i]) / 2.0);
            origin[axis] = if high {
                bounds.max[axis]
            } else {
                bounds.min[axis]
            };
            let mut normal = [0.0; 3];
            normal[axis] = if high { 1.0 } else { -1.0 };
            return Ok(Plane { origin, normal });
        }
        let middle = |z: f64| {
            [
                (bounds.min[0] + bounds.max[0]) / 2.0,
                (bounds.min[1] + bounds.max[1]) / 2.0,
                z,
            ]
        };
        match plain_role(face) {
            "end" | "end_b" => Ok(Plane {
                origin: middle(bounds.max[2]),
                normal: [0.0, 0.0, 1.0],
            }),
            "start" | "end_a" => Ok(Plane {
                origin: middle(bounds.min[2]),
                normal: [0.0, 0.0, -1.0],
            }),
            _ => Err(KernelError::failed(format!("face {face} is not planar"))),
        }
    }

    /// With `box_faces` (mitcad#524): each face's slab of the bounds (a
    /// box's face), else the bounds.
    fn face_boxes(&self, shape: &MockShape) -> Result<Vec<[[f64; 3]; 2]>, KernelError> {
        if !self.box_faces.get() {
            let points = self.piece_points.borrow();
            if points.is_empty() {
                return Err(KernelError::Unsupported("face boxes"));
            }
            let bounds = shape.bounds.ok_or(KernelError::failed("no bounds"))?;
            return Ok(shape
                .faces
                .iter()
                .map(|face| {
                    let name = face.to_string();
                    points
                        .iter()
                        .find(|(b, n, _)| *b == bounds && *n == name)
                        .map_or([bounds.min, bounds.max], |(_, _, p)| [*p, *p])
                })
                .collect());
        }
        let bounds = shape.bounds.ok_or(KernelError::failed("no bounds"))?;
        Ok(shape
            .faces
            .iter()
            .map(|face| {
                let (mut lo, mut hi) = (bounds.min, bounds.max);
                if let Some((axis, high)) = box_side(plain_role(face)) {
                    let at = if high {
                        bounds.max[axis]
                    } else {
                        bounds.min[axis]
                    };
                    lo[axis] = at;
                    hi[axis] = at;
                }
                [lo, hi]
            })
            .collect())
    }

    /// With `box_faces` (mitcad#524): where the segment enters and leaves
    /// the bounds.
    fn segment_crossings(
        &self,
        shape: &MockShape,
        from: [f64; 3],
        to: [f64; 3],
    ) -> Result<Vec<(f64, bool)>, KernelError> {
        if !self.box_faces.get() {
            return Err(KernelError::Unsupported("segment crossings"));
        }
        let Some(b) = shape.bounds else {
            return Ok(Vec::new());
        };
        let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
        for i in 0..3 {
            let d = to[i] - from[i];
            if d.abs() < 1e-12 {
                if from[i] < b.min[i] || from[i] > b.max[i] {
                    return Ok(Vec::new());
                }
                continue;
            }
            let (a, c) = ((b.min[i] - from[i]) / d, (b.max[i] - from[i]) / d);
            t0 = t0.max(a.min(c));
            t1 = t1.min(a.max(c));
        }
        if t0 >= t1 || t1 <= 0.0 || t0 >= 1.0 {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        if t0 > 0.0 {
            out.push((t0, true));
        }
        if t1 < 1.0 {
            out.push((t1, false));
        }
        Ok(out)
    }

    /// Side faces of closed curves and hole walls are cylinders along z,
    /// as wide as the bounds; hole walls are internal.
    fn face_cylinder(&self, shape: &MockShape, face: &FaceName) -> Result<Cylinder, KernelError> {
        if !shape.has_face(face) {
            return Err(KernelError::failed(format!("the body has no face {face}")));
        }
        let internal = face.role.ends_with(".wall");
        let closed = matches!(&face.key, Some(RoleKey::Segment(s)) if s.ends.is_none());
        if !internal && !(face.role == "side" && closed) {
            return Err(KernelError::failed(format!(
                "face {face} is not cylindrical"
            )));
        }
        let bounds = shape.bounds.unwrap_or(BoundingBox {
            min: [0.0; 3],
            max: [10.0; 3],
        });
        Ok(Cylinder {
            axis: Axis {
                origin: [
                    (bounds.min[0] + bounds.max[0]) / 2.0,
                    (bounds.min[1] + bounds.max[1]) / 2.0,
                    bounds.min[2],
                ],
                direction: [0.0, 0.0, 1.0],
            },
            radius: self
                .hole_radii
                .borrow()
                .get(&face.to_string())
                .copied()
                .unwrap_or((bounds.max[0] - bounds.min[0]) / 2.0),
            length: bounds.max[2] - bounds.min[2],
            internal,
        })
    }

    // Sweeps, lofts, pipes, coils, ribs and webs (F3): shapes with the
    // OCCT facade's face names, the spec described in `specs`.

    fn sweep(&self, spec: &SweepSpec<'_>) -> Result<MockShape, KernelError> {
        self.call("sweep")?;
        let names: Vec<&str> = spec.path.iter().map(|p| p.name.as_str()).collect();
        self.specs.borrow_mut().push(format!(
            "sweep {} path {} extents {} {} {:?} twist {} taper {} rail {}",
            spec.feature,
            names.join(","),
            number(spec.extent1),
            number(spec.extent2),
            spec.orientation,
            number(spec.twist),
            number(spec.taper),
            spec.guide.map_or_else(
                || "none".to_owned(),
                |g| format!(
                    "{} {:?}",
                    g.rail
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    g.scaling
                )
            )
        ));
        // A fixed binormal (mitcad#397), when one is given.
        if let Some(b) = spec.binormal
            && let Some(last) = self.specs.borrow_mut().last_mut()
        {
            last.push_str(&format!(
                " binormal {} {} {}",
                number(b[0]),
                number(b[1]),
                number(b[2])
            ));
        }
        // A second rail and an end scale (mitcad#464), when given.
        if let Some(last) = self.specs.borrow_mut().last_mut() {
            if let Some(rail2) = spec.guide.and_then(|g| g.rail2) {
                let names: Vec<&str> = rail2.iter().map(|p| p.name.as_str()).collect();
                last.push_str(&format!(" rail2 {}", names.join(",")));
            }
            if spec.end_scale != 1.0 {
                last.push_str(&format!(" end scale {}", number(spec.end_scale)));
            }
            // The plane rule of the rails' points (mitcad#469).
            if spec
                .guide
                .is_some_and(|g| g.point == crate::sweeps::RailPoint::Plane)
            {
                last.push_str(" rail point plane");
            }
            // A Frenet or auxiliary frame (mitcad#539).
            match spec.path_frame {
                Some(crate::sweeps::PathFrame::Frenet) => last.push_str(" frame frenet"),
                Some(crate::sweeps::PathFrame::Auxiliary { curve, curvilinear }) => {
                    let names: Vec<&str> = curve.iter().map(|p| p.name.as_str()).collect();
                    last.push_str(&format!(
                        " frame auxiliary {}{}",
                        names.join(","),
                        if curvilinear { " curvilinear" } else { "" }
                    ));
                }
                Some(crate::sweeps::PathFrame::ConstantBinormal(b)) => {
                    last.push_str(&format!(
                        " frame constant binormal {} {} {}",
                        number(b[0]),
                        number(b[1]),
                        number(b[2])
                    ));
                }
                None => {}
            }
        }
        let around = crate::features::path::is_closed(spec.path)
            && spec.extent1 + spec.extent2 >= 1.0 - 1e-9;
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        for region in spec.regions {
            let caps = [
                FaceName::start(spec.feature, region.key.clone()),
                FaceName::end(spec.feature, region.key.clone()),
            ];
            if !around {
                faces.extend(caps.clone());
            }
            for segment in region.loops.iter().flat_map(|l| &l.segments) {
                let side = FaceName::side(spec.feature, segment.key.clone());
                if !around {
                    for cap in &caps {
                        edges.push((side.clone(), cap.clone()));
                    }
                }
                faces.push(side);
            }
        }
        Ok(MockShape {
            history: format!("sweep({}:{})", spec.feature, names.join(",")),
            faces,
            edges,
            bounds: None,
            parts: Vec::new(),
        })
    }

    // Sweeps along guide surfaces (mitcad#157): the sweep's spec line ends
    // with the faces.

    fn surface_sweep(
        &self,
        spec: &SweepSpec<'_>,
        surfaces: &[crate::sweeps::GuideSurface<'_, MockShape>],
    ) -> Result<MockShape, KernelError> {
        for surface in surfaces {
            if !surface.body.faces.contains(surface.face) {
                return Err(KernelError::Failed(format!(
                    "guide surface {} does not exist",
                    surface.face
                )));
            }
        }
        let shape = self.sweep(spec)?;
        let faces: Vec<String> = surfaces.iter().map(|s| s.face.to_string()).collect();
        if let Some(line) = self.specs.borrow_mut().last_mut() {
            line.push_str(&format!(" surfaces {}", faces.join(",")));
        }
        Ok(shape)
    }

    fn loft(&self, spec: &LoftSpec<'_, MockShape>) -> Result<MockShape, KernelError> {
        self.call("loft")?;
        let describe = |section: &LoftSection<'_, MockShape>| match section {
            LoftSection::Region { region, .. } => format!("region {}", region.key),
            LoftSection::Face { face, .. } => format!("face {face}"),
            LoftSection::Point(p) => format!("point {p:?}"),
        };
        let names = |path: &[PathCurve]| {
            path.iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut text = format!(
            "loft {} sections {} ruled {} closed {} centerline {}",
            spec.feature,
            spec.sections
                .iter()
                .map(describe)
                .collect::<Vec<_>>()
                .join("; "),
            spec.ruled,
            spec.closed,
            spec.centerline.map_or_else(|| "none".to_owned(), names)
        );
        if !spec.solid {
            text += " surface";
        }
        // Rails and end conditions only when there are any.
        if !spec.rails.is_empty() {
            let rails: Vec<String> = spec.rails.iter().map(|r| names(r)).collect();
            text += &format!(" rails {}", rails.join("; "));
        }
        for (which, end) in [("start", spec.start), ("end", spec.end)] {
            if !matches!(end.kind, LoftEndKind::Free | LoftEndKind::PointSharp) {
                text += &format!(
                    " {which} {:?} angle {} weight {}",
                    end.kind, end.angle, end.weight
                );
            }
        }
        // A frame along the centre line (mitcad#543).
        match spec.centerline_frame {
            Some(crate::sweeps::PathFrame::Frenet) => text += " frame frenet",
            Some(crate::sweeps::PathFrame::Auxiliary { curve, curvilinear }) => {
                text += &format!(
                    " frame auxiliary {}{}",
                    names(curve),
                    if curvilinear { " curvilinear" } else { "" }
                );
            }
            Some(crate::sweeps::PathFrame::ConstantBinormal(b)) => {
                text += &format!(
                    " frame constant binormal {} {} {}",
                    number(b[0]),
                    number(b[1]),
                    number(b[2])
                );
            }
            None => {}
        }
        // The parallel orientation along the centre line (mitcad#547).
        if spec.centerline_parallel {
            text += " parallel";
        }
        self.specs.borrow_mut().push(text);
        let key = |section: &LoftSection<'_, MockShape>| match section {
            LoftSection::Region { region, .. } => Some(RoleKey::Region(region.key.clone())),
            LoftSection::Face { face, .. } => Some(RoleKey::Face(Box::new((*face).clone()))),
            LoftSection::Point(_) => None,
        };
        let mut faces = Vec::new();
        for section in &spec.sections {
            match section {
                LoftSection::Region { region, .. } => {
                    for segment in &region.loops[0].segments {
                        faces.push(FaceName::side(spec.feature, segment.key.clone()));
                    }
                    break;
                }
                LoftSection::Face { face, .. } => {
                    faces.push(FaceName::new(
                        spec.feature,
                        "side",
                        Some(RoleKey::Face(Box::new((*face).clone()))),
                    ));
                    break;
                }
                LoftSection::Point(_) => {}
            }
        }
        if !spec.closed {
            for (role, section) in [
                ("start", spec.sections.first()),
                ("end", spec.sections.last()),
            ] {
                if let Some(key) = section.and_then(key) {
                    faces.push(FaceName::new(spec.feature, role, Some(key)));
                }
            }
        }
        Ok(MockShape {
            history: format!("loft({}:{})", spec.feature, spec.sections.len()),
            faces,
            edges: Vec::new(),
            bounds: None,
            parts: Vec::new(),
        })
    }

    fn pipe(&self, spec: &PipeSpec<'_>) -> Result<MockShape, KernelError> {
        self.call("pipe")?;
        let names: Vec<&str> = spec.path.iter().map(|p| p.name.as_str()).collect();
        self.specs.borrow_mut().push(format!(
            "pipe {} path {} extents {} {} {:?} size {} thickness {:?}",
            spec.feature,
            names.join(","),
            number(spec.extent1),
            number(spec.extent2),
            spec.section,
            number(spec.size),
            spec.thickness
        ));
        let sides = match spec.section {
            PipeSection::Circular => 1,
            PipeSection::Square => 4,
            PipeSection::Triangular => 3,
        };
        let mut faces: Vec<FaceName> = (0..sides)
            .map(|i| FaceName::new(spec.feature, &format!("side{i}"), None))
            .collect();
        if spec.thickness.is_some() {
            faces.extend(
                (0..sides).map(|i| FaceName::new(spec.feature, &format!("inner{i}"), None)),
            );
        }
        faces.extend(["start", "end"].map(|role| FaceName::new(spec.feature, role, None)));
        Ok(MockShape {
            history: format!("pipe({}:{})", spec.feature, names.join(",")),
            faces,
            edges: Vec::new(),
            bounds: None,
            parts: Vec::new(),
        })
    }

    fn coil(&self, spec: &CoilSpec) -> Result<MockShape, KernelError> {
        self.call("coil")?;
        self.specs.borrow_mut().push(format!(
            "coil {} at {:?} axis {:?} diameter {} revolutions {} pitch {} angle {} spiral {} \
             clockwise {} {:?} {:?} size {}",
            spec.feature,
            spec.frame.origin,
            spec.frame.normal(),
            number(spec.diameter),
            number(spec.revolutions),
            number(spec.pitch),
            number(spec.angle),
            spec.spiral,
            spec.clockwise,
            spec.section,
            spec.position,
            number(spec.size)
        ));
        let mut faces = vec![
            FaceName::new(spec.feature, "start", None),
            FaceName::new(spec.feature, "end", None),
        ];
        faces.push(FaceName::new(spec.feature, "side0", None));
        Ok(MockShape {
            history: format!("coil({})", spec.feature),
            faces,
            edges: Vec::new(),
            bounds: None,
            parts: Vec::new(),
        })
    }

    fn rib(&self, spec: &RibSpec<'_, MockShape>) -> Result<MockShape, KernelError> {
        self.call("rib")?;
        let chains: Vec<String> = spec
            .chains
            .iter()
            .map(|c| {
                c.iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect();
        self.specs.borrow_mut().push(format!(
            "{} {} chains {} thickness {} {:?} depth {:?} flip {} bodies {}",
            if spec.web { "web" } else { "rib" },
            spec.feature,
            chains.join("; "),
            number(spec.thickness),
            spec.location,
            spec.depth,
            spec.flip,
            spec.bodies.len()
        ));
        let mut faces = Vec::new();
        for curve in spec.chains.iter().flatten() {
            let key = RoleKey::Segment(SegmentKey::closed(
                crate::ids::EntityUid::parse_curve(&curve.name).unwrap_or(crate::ids::EntityUid(0)),
            ));
            let role = if spec.web { "wall1" } else { "side" };
            faces.push(FaceName::new(spec.feature, role, Some(key)));
        }
        Ok(MockShape {
            history: format!("{}({})", if spec.web { "web" } else { "rib" }, spec.feature),
            faces,
            edges: Vec::new(),
            bounds: None,
            parts: Vec::new(),
        })
    }

    /// The points set in `vertex_points` (mitcad#55).
    fn vertex_point(
        &self,
        _shape: &MockShape,
        vertex: &crate::topo::VertexName,
    ) -> Result<crate::datum::Vec3, KernelError> {
        self.vertex_points
            .borrow()
            .get(&vertex.to_string())
            .copied()
            .ok_or_else(|| KernelError::failed(format!("the body has no vertex {vertex}")))
    }

    /// Every edge is a line along x from the bounds' low corner, unless
    /// `edge_curves` says otherwise.
    fn edge_geometry(
        &self,
        shape: &MockShape,
        edge: &EdgeName,
    ) -> Result<CurveGeometry, KernelError> {
        if shape.matching_edges(edge).is_empty() {
            return Err(KernelError::failed(format!("the body has no edge {edge}")));
        }
        if let Some(curve) = self.edge_curves.borrow().get(&edge.to_string()) {
            return Ok(curve.clone());
        }
        let start = shape.bounds.map_or([0.0; 3], |b| b.min);
        Ok(CurveGeometry::Line {
            start,
            end: [start[0] + 1.0, start[1], start[2]],
        })
    }

    /// Box letters of "Mock Sans" (any other family falls back to it): a
    /// 0.5 x 0.7 em block; `O`-like letters have one counter, `B` and `8`
    /// two, `C` and `S` a curved top (a quadratic piece); 0.7 em advance.
    fn font_glyphs(&self, request: &FontRequest<'_>) -> Result<FontGlyphs, KernelError> {
        self.call("font_glyphs")?;
        let rect = |x0: f64, y0: f64, x1: f64, y1: f64, ccw: bool| {
            let mut corners = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
            if !ccw {
                corners.reverse();
            }
            (0..4)
                .map(|i| vec![corners[i], corners[(i + 1) % 4]])
                .collect::<Vec<_>>()
        };
        let glyphs = request
            .text
            .chars()
            .map(|c| {
                let outer = rect(0.1, 0.0, 0.6, 0.7, true);
                let contours = match c {
                    ' ' | '\n' => Vec::new(),
                    'O' | 'o' | '0' | 'A' | 'a' | 'D' | 'e' | 'P' | 'R' | 'Q' | 'd' | 'b' => {
                        vec![outer, rect(0.25, 0.2, 0.45, 0.5, false)]
                    }
                    'B' | '8' => vec![
                        outer,
                        rect(0.25, 0.1, 0.45, 0.3, false),
                        rect(0.25, 0.4, 0.45, 0.6, false),
                    ],
                    'C' | 'S' => vec![vec![
                        vec![[0.1, 0.0], [0.6, 0.0]],
                        vec![[0.6, 0.0], [0.6, 0.7]],
                        vec![[0.6, 0.7], [0.35, 0.95], [0.1, 0.7]],
                        vec![[0.1, 0.7], [0.1, 0.0]],
                    ]],
                    _ => vec![outer],
                };
                Glyph {
                    advance: match c {
                        '\n' => 0.0,
                        ' ' => 0.35,
                        _ => 0.7,
                    },
                    contours,
                }
            })
            .collect();
        Ok(FontGlyphs {
            family: "Mock Sans".to_owned(),
            fallback: !(request.family.is_empty() || request.family == "Mock Sans"),
            ascender: 0.75,
            descender: -0.25,
            cap_height: 0.0,
            line_spacing: 1.2,
            glyphs,
        })
    }

    /// The test fonts of `data/fonts` as installed (mitcad#473): Liberation
    /// Sans and Noto Sans Arabic.
    fn font_source(
        &self,
        request: &FontRequest<'_>,
    ) -> Result<crate::kernel::FontSource, KernelError> {
        let file = match request.family {
            "Liberation Sans" => "LiberationSans-Regular.ttf",
            "Noto Sans Arabic" => "NotoSansArabic-Regular.ttf",
            _ => return Err(KernelError::failed("not installed")),
        };
        Ok(crate::kernel::FontSource {
            family: request.family.to_owned(),
            path: format!("{}/data/fonts/{file}", env!("CARGO_MANIFEST_DIR")),
            index: 0,
            synthetic_italic: request.italic,
        })
    }

    // Model warnings (P9).

    fn notes(&self, shape: &MockShape) -> Vec<String> {
        self.notes
            .borrow()
            .iter()
            .filter(|(op, _)| shape.history.starts_with(&format!("{op}(")))
            .map(|(_, note)| note.clone())
            .collect()
    }

    // The result store (P7d): a shape as JSON.

    fn shape_bytes(&self, shape: &MockShape) -> Result<Vec<u8>, KernelError> {
        self.call("shape_bytes")?;
        Ok(serde_json::to_vec(&StoredMock::new(shape)).expect("mock shapes serialize"))
    }

    fn shape_from_bytes(&self, bytes: &[u8]) -> Result<MockShape, KernelError> {
        self.call("shape_from_bytes")?;
        serde_json::from_slice::<StoredMock>(bytes)
            .map(StoredMock::shape)
            .map_err(|e| KernelError::failed(format!("not a stored mock shape: {e}")))
    }

    /// A kilobyte a face, so that tests can reckon with the memory budget.
    fn shape_memory(&self, shape: &MockShape) -> u64 {
        1000 * shape.faces.len() as u64
    }

    // Cancellation inside an operation (P7e).

    fn interruptible<R>(&self, monitor: &Arc<RecomputeMonitor>, f: impl FnOnce() -> R) -> R {
        *self.calls.borrow_mut().entry("interruptible").or_default() += 1;
        let outer = self.interrupt.replace(Some(monitor.clone()));
        let result = f();
        self.interrupt.replace(outer);
        result
    }

    // Triangle meshes for 3D printing (3MF export, mitcad#13).

    /// The shape's bounding box as a closed mesh of 12 triangles facing
    /// outwards; the tolerances go to `mesh_tolerances`.
    fn triangle_mesh(
        &self,
        shape: &MockShape,
        tolerance: &crate::exchange::MeshTolerance,
    ) -> Result<crate::exchange::TriangleMesh, KernelError> {
        self.call("triangle_mesh")?;
        self.mesh_tolerances.borrow_mut().push(*tolerance);
        let Some(bounds) = shape.bounds else {
            return Ok(crate::exchange::TriangleMesh::default());
        };
        let corner = |i: usize| {
            std::array::from_fn(|axis| {
                if (i >> axis) & 1 == 1 {
                    bounds.max[axis]
                } else {
                    bounds.min[axis]
                }
            })
        };
        let quads = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        Ok(crate::exchange::TriangleMesh {
            vertices: (0..8).map(corner).collect(),
            triangles: quads
                .iter()
                .flat_map(|q: &[u32; 4]| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
                .collect(),
        })
    }

    /// The shape's bounding box, each face of [`MockShape::faces`] (six
    /// without any) a side of the box (the sides in turn: -Z, +Z, -Y, +Y,
    /// -X, +X) of two triangles with its own four vertices and the side's
    /// normal; the tolerances go to `mesh_tolerances`.
    fn face_meshes(
        &self,
        shape: &MockShape,
        tolerance: &crate::exchange::MeshTolerance,
    ) -> Result<crate::exchange::FaceMeshes, KernelError> {
        self.call("face_meshes")?;
        self.mesh_tolerances.borrow_mut().push(*tolerance);
        let Some(bounds) = shape.bounds else {
            return Ok(crate::exchange::FaceMeshes::default());
        };
        let corner = |i: u32| -> [f64; 3] {
            std::array::from_fn(|axis| {
                if (i >> axis) & 1 == 1 {
                    bounds.max[axis]
                } else {
                    bounds.min[axis]
                }
            })
        };
        let sides: [([u32; 4], [f64; 3]); 6] = [
            ([0, 2, 3, 1], [0.0, 0.0, -1.0]),
            ([4, 5, 7, 6], [0.0, 0.0, 1.0]),
            ([0, 1, 5, 4], [0.0, -1.0, 0.0]),
            ([2, 6, 7, 3], [0.0, 1.0, 0.0]),
            ([0, 4, 6, 2], [-1.0, 0.0, 0.0]),
            ([1, 3, 7, 5], [1.0, 0.0, 0.0]),
        ];
        let mut mesh = crate::exchange::FaceMeshes::default();
        let count = if shape.faces.is_empty() {
            6
        } else {
            shape.faces.len()
        };
        for face in 0..count {
            let (quad, normal) = sides[face % 6];
            let first = mesh.vertices.len() as u32;
            for i in quad {
                mesh.vertices.push(corner(i));
                mesh.normals.push(normal);
            }
            mesh.triangles
                .extend([[first, first + 1, first + 2], [first, first + 2, first + 3]]);
            mesh.face_triangles.push(2);
        }
        Ok(mesh)
    }

    // FreeCAD helices (mitcad#4).

    fn helix(
        &self,
        spec: &crate::sweeps::HelixSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("helix")?;
        let growth = if spec.growth == 0.0 {
            String::new()
        } else {
            let freecad = if spec.freecad { " freecad" } else { "" };
            format!(
                " growth {} flip {}{freecad}",
                number(spec.growth),
                spec.flip
            )
        };
        let ends = if spec.ends.iter().all(|a| *a == 0.0) {
            String::new()
        } else {
            let angles: Vec<String> = spec.ends.iter().map(|a| number(*a)).collect();
            format!(" ends {}", angles.join(" "))
        };
        let shift = if spec.piece_shift == 0.0 {
            String::new()
        } else {
            format!(" shift {}", number(spec.piece_shift))
        };
        self.specs.borrow_mut().push(format!(
            "helix {} axis {:?} {:?} pitch {} revolutions {} left {}{growth}{ends}{shift}",
            spec.feature,
            spec.origin,
            spec.direction,
            number(spec.pitch),
            number(spec.revolutions),
            spec.left_handed
        ));
        let mut faces = Vec::new();
        for region in spec.regions {
            faces.push(FaceName::start(spec.feature, region.key.clone()));
            faces.push(FaceName::end(spec.feature, region.key.clone()));
            for segment in region.loops.iter().flat_map(|l| &l.segments) {
                faces.push(FaceName::side(spec.feature, segment.key.clone()));
            }
        }
        // Pieces' seams shifted (mitcad#235): in the history too.
        let history = if spec.piece_shift == 0.0 {
            format!("helix({})", spec.feature)
        } else {
            format!("helix({} shift {})", spec.feature, number(spec.piece_shift))
        };
        Ok(MockShape {
            history,
            faces,
            edges: Vec::new(),
            bounds: None,
            parts: Vec::new(),
        })
    }

    // Definitions evaluated in parallel (the .f3d import, mitcad#95): a
    // kernel with the same settings, its own calls and requests.
    fn fork(&self) -> Option<Self> {
        Some(MockKernel {
            measures: self.measures.clone(),
            fail: self.fail.clone(),
            boolean_fail_targets: self.boolean_fail_targets.clone(),
            boolean_fail_tools: self.boolean_fail_tools.clone(),
            out_of_memory: self.out_of_memory.clone(),
            touch: self.touch.clone(),
            split: self.split.clone(),
            lose: self.lose.clone(),
            voids: self.voids.clone(),
            surfaces: self.surfaces.clone(),
            files: self.files.clone(),
            notes: self.notes.clone(),
            curves: self.curves.clone(),
            named_curves: self.named_curves.clone(),
            edge_curves: self.edge_curves.clone(),
            vertex_points: self.vertex_points.clone(),
            middles: self.middles.clone(),
            vertices: self.vertices.clone(),
            hole_radii: self.hole_radii.clone(),
            ..MockKernel::default()
        })
    }

    // Fillets repeated on a pattern's copies (mitcad#105).
    fn edge_middles(
        &self,
        shape: &MockShape,
    ) -> Result<Vec<crate::kernel::EdgeMiddle>, KernelError> {
        let middles = self.middles.borrow();
        Ok(shape
            .edges
            .iter()
            .filter_map(|(a, b)| {
                let name = EdgeName::new(a.clone(), b.clone()).to_string();
                middles.get(&name).map(|point| crate::kernel::EdgeMiddle {
                    name,
                    point: *point,
                    tangent: [0.0, 1.0, 0.0],
                    length: 1.0,
                })
            })
            .collect())
    }

    // The faces a run of features made, copied by a pattern.
    //
    // The faces `after` has and `before` lacks are one added solid (with
    // `after`'s bounds); the faces `before` has and `after` lacks one
    // removed solid. The mock never has "material", only faces.
    fn feature_changes(
        &self,
        before: &MockShape,
        after: &MockShape,
    ) -> Result<crate::kernel::FeatureChanges<MockShape>, KernelError> {
        self.call("feature_changes")?;
        let piece = |label: &str, from: &MockShape, lacking: &MockShape| {
            let faces: Vec<FaceName> = from
                .faces
                .iter()
                .filter(|f| !lacking.faces.contains(f))
                .cloned()
                .collect();
            (!faces.is_empty()).then(|| MockShape {
                history: format!("{label}({},{})", from.history, lacking.history),
                edges: from
                    .edges
                    .iter()
                    .filter(|(a, b)| faces.contains(a) && faces.contains(b))
                    .cloned()
                    .collect(),
                faces,
                bounds: from.bounds,
                parts: Vec::new(),
            })
        };
        Ok(crate::kernel::FeatureChanges {
            added: piece("added", after, before)
                .into_iter()
                .map(|solid| crate::kernel::AddedMaterial {
                    seam: Some(MockShape {
                        history: format!("seam({})", solid.history),
                        faces: Vec::new(),
                        edges: Vec::new(),
                        bounds: solid.bounds,
                        parts: Vec::new(),
                    }),
                    solid,
                })
                .collect(),
            removed: piece("removed", before, after).into_iter().collect(),
        })
    }

    /// Copies stand on the body, except those whose index is in
    /// `unsupported` (counted over the copies of one call).
    fn supported_copies(
        &self,
        _body: &MockShape,
        copies: &[(&MockShape, Option<&MockShape>)],
    ) -> Result<Vec<Option<MockShape>>, KernelError> {
        self.call("supported_copies")?;
        let unsupported = self.unsupported.borrow();
        Ok(copies
            .iter()
            .enumerate()
            .map(|(i, (solid, _))| (!unsupported.contains(&i)).then(|| (*solid).clone()))
            .collect())
    }

    /// The shape [`Kernel::import_brep`] reads from the shape's data,
    /// counted as "adopt" (fails when `fail` has "adopt").
    fn adopt_shape(
        &self,
        feature: FeatureUid,
        shape: &MockShape,
        first_face: u32,
    ) -> Result<(MockShape, u32), KernelError> {
        self.call("adopt")?;
        read_mock_brep(feature, &self.brep_data(shape)?, first_face)
    }

    // Bodies from their boundaries and from triangles (mitcad#273): a body
    // of a face per boundary face in the box of the vertices (a sheet when
    // a shell has fewer than four faces, which closes none).
    fn boundary_body(
        &self,
        body: &crate::boundary::BoundaryBody,
    ) -> Result<(MockShape, crate::boundary::BoundaryReport), KernelError> {
        self.call("boundary_body")?;
        body.check().map_err(KernelError::Failed)?;
        if body.faces.is_empty() {
            return Err(KernelError::failed("a boundary without faces"));
        }
        let solid = body.shells.iter().all(|s| s.len() >= 4);
        let mut shape = MockShape::imported(
            if solid { "boundary" } else { "boundary,sheet" },
            body.faces.len(),
        );
        shape.bounds = points_box(&body.vertices);
        let report = crate::boundary::BoundaryReport {
            solid,
            valid: true,
            faces: body.faces.len(),
            ..Default::default()
        };
        Ok((shape, report))
    }

    fn triangle_body(
        &self,
        points: &[[f64; 3]],
        triangles: &[[u32; 3]],
    ) -> Result<MockShape, KernelError> {
        self.call("triangle_body")?;
        if triangles.is_empty()
            || triangles
                .iter()
                .flatten()
                .any(|i| *i as usize >= points.len())
        {
            return Err(KernelError::failed(
                "no triangles, or an index out of range",
            ));
        }
        let mut shape = MockShape::imported("mesh", 1);
        shape.bounds = points_box(points);
        Ok(shape)
    }

    /// By bounding boxes, counted as "tool_contacts" per body asked about:
    /// boxes apart are apart, boxes that overlap in a volume with the
    /// tool's centre inside the body's are inside, others touch.
    fn tool_contacts(
        &self,
        tool: &MockShape,
        bodies: &[&MockShape],
    ) -> Result<Vec<crate::kernel::ToolContact>, KernelError> {
        use crate::kernel::ToolContact;
        let tool_box = tool
            .bounds
            .ok_or_else(|| KernelError::failed("a tool without bounds"))?;
        let mut contacts = Vec::new();
        for body in bodies {
            self.call("tool_contacts")?;
            let Some(b) = body.bounds else {
                contacts.push(ToolContact::Apart);
                continue;
            };
            let overlap: [f64; 3] = std::array::from_fn(|i| {
                tool_box.max[i].min(b.max[i]) - tool_box.min[i].max(b.min[i])
            });
            let centre: [f64; 3] =
                std::array::from_fn(|i| (tool_box.min[i] + tool_box.max[i]) / 2.0);
            contacts.push(if overlap.iter().any(|o| *o < -1e-9) {
                ToolContact::Apart
            } else if overlap.iter().all(|o| *o > 1e-9)
                && (0..3).all(|i| b.min[i] < centre[i] && centre[i] < b.max[i])
            {
                ToolContact::Inside
            } else {
                ToolContact::Touches
            });
        }
        Ok(contacts)
    }

    /// [`MockKernel::boolean`]'s join, counted as "join_within" too; the
    /// tolerance goes to `specs` ("join within <mm>").
    fn join_within(
        &self,
        targets: &[&MockShape],
        tool: &MockShape,
        tolerance: f64,
    ) -> Result<BooleanOutput<MockShape>, KernelError> {
        self.specs
            .borrow_mut()
            .push(format!("join within {tolerance}"));
        self.call("join_within")?;
        self.boolean(BooleanOp::Join, targets, tool)
    }

    /// [`MockKernel::boolean`]'s cut, counted as "cut_within" too; the
    /// tolerance goes to `specs` ("cut within <mm>").
    fn cut_within(
        &self,
        targets: &[&MockShape],
        tool: &MockShape,
        tolerance: f64,
    ) -> Result<BooleanOutput<MockShape>, KernelError> {
        self.specs
            .borrow_mut()
            .push(format!("cut within {tolerance}"));
        self.call("cut_within")?;
        self.boolean(BooleanOp::Cut, targets, tool)
    }

    /// Renames the faces (and the faces of the edges) as the OCCT facade
    /// does; the prefix `F7:part` wraps a name n as `F7:part(n)`.
    fn rename_faces(
        &self,
        shape: &MockShape,
        prefix: &str,
        renames: &[(FaceName, FaceName)],
    ) -> Result<MockShape, KernelError> {
        self.call("rename_faces")?;
        let wrap = if prefix.is_empty() {
            None
        } else {
            let (feature, role) = prefix
                .split_once(':')
                .ok_or_else(|| KernelError::failed(format!("invalid prefix {prefix}")))?;
            let feature: FeatureUid = feature
                .parse()
                .map_err(|_| KernelError::failed(format!("invalid prefix {prefix}")))?;
            Some((feature, role.to_owned()))
        };
        Ok(renamed(shape, &wrap, renames))
    }

    /// Where the plane cuts the shape's bounds (a box): a line per side of
    /// the polygon, each on the face asked for (on none for all faces).
    fn section_curves(
        &self,
        shape: &MockShape,
        face: Option<&FaceName>,
        plane: &crate::datum::DatumPlane,
    ) -> Result<Vec<crate::kernel::SectionCurve>, KernelError> {
        self.call("section_curves")?;
        let (Some(bounds), true) = (shape.bounds, face.is_none_or(|f| shape.has_face(f))) else {
            return Ok(Vec::new());
        };
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let normal = plane.normal();
        let corners: Vec<[f64; 3]> = bounds.corners().collect();
        let mut points: Vec<[f64; 3]> = Vec::new();
        // The box's edges join corners whose indices differ in one bit.
        for i in 0..8 {
            for axis in 0..3 {
                let j = i | (1 << axis);
                if j == i {
                    continue;
                }
                let (p, q) = (corners[i], corners[j]);
                let (dp, dq) = (
                    dot(sub(p, plane.origin), normal),
                    dot(sub(q, plane.origin), normal),
                );
                if (dp > 0.0 && dq > 0.0) || (dp < 0.0 && dq < 0.0) || dp == dq {
                    continue;
                }
                let t = dp / (dp - dq);
                let at: [f64; 3] = std::array::from_fn(|k| p[k] + t * (q[k] - p[k]));
                if !points
                    .iter()
                    .any(|o| sub(*o, at).iter().all(|d| d.abs() < 1e-9))
                {
                    points.push(at);
                }
            }
        }
        if points.len() < 3 {
            return Ok(Vec::new());
        }
        // Round the polygon's middle, in the plane's coordinates.
        let n = points.len() as f64;
        let middle: [f64; 3] =
            std::array::from_fn(|k| points.iter().map(|p| p[k]).sum::<f64>() / n);
        let angle = |p: &[f64; 3]| {
            let d = sub(*p, middle);
            dot(d, plane.y_axis).atan2(dot(d, plane.x_axis))
        };
        points.sort_by(|a, b| angle(a).total_cmp(&angle(b)));
        let faces: Vec<FaceName> = face.into_iter().cloned().collect();
        Ok((0..points.len())
            .map(|k| crate::kernel::SectionCurve {
                curve: crate::kernel::Curve3::Line {
                    start: points[k],
                    end: points[(k + 1) % points.len()],
                },
                faces: faces.clone(),
            })
            .collect())
    }

    // Unions within a tolerance (mitcad#397).

    /// [`MockKernel::unite`], counted as "unite_within" too; the tolerance
    /// goes to `specs` ("unite within <mm>").
    fn unite_within(
        &self,
        shapes: &[&MockShape],
        tolerance: f64,
    ) -> Result<MockShape, KernelError> {
        self.specs
            .borrow_mut()
            .push(format!("unite within {tolerance}"));
        self.call("unite_within")?;
        self.unite(shapes)
    }

    // Contact sets (mitcad#454).

    /// The common volume of the placed bounding boxes (the boxes around
    /// their moved corners), counted as "overlap_volumes" per pair: boxes
    /// that overlap by `tolerance` or less along an axis only touch.
    fn overlap_volumes(
        &self,
        pairs: &[crate::kernel::PlacedPair<'_, MockShape>],
        tolerance: f64,
    ) -> Result<Vec<f64>, KernelError> {
        let placed = |s: &MockShape, t: &Transform| -> Option<BoundingBox> {
            let b = s.bounds?;
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for corner in b.corners() {
                let p = t.apply_point(corner);
                for i in 0..3 {
                    min[i] = min[i].min(p[i]);
                    max[i] = max[i].max(p[i]);
                }
            }
            Some(BoundingBox { min, max })
        };
        let mut volumes = Vec::with_capacity(pairs.len());
        for pair in pairs {
            self.call("overlap_volumes")?;
            let (Some(a), Some(b)) = (
                placed(pair.a, &pair.a_placement),
                placed(pair.b, &pair.b_placement),
            ) else {
                volumes.push(0.0);
                continue;
            };
            let overlap: [f64; 3] =
                std::array::from_fn(|i| a.max[i].min(b.max[i]) - a.min[i].max(b.min[i]));
            volumes.push(if overlap.iter().all(|o| *o > tolerance) {
                overlap.iter().product()
            } else {
                0.0
            });
        }
        Ok(volumes)
    }

    // Emboss (mitcad#458).

    /// The profiles' prism one unit along the sketch normal, faces named
    /// as the wrapped tool's, with the history `wrap(<feature>:<radius>..
    /// <to_radius>)`; the spec described in `specs`.
    fn wrapped_emboss(
        &self,
        spec: &crate::kernel::EmbossWrapSpec<'_>,
    ) -> Result<MockShape, KernelError> {
        self.call("wrapped_emboss")?;
        let axis = |v: [f64; 3]| v.map(number).join(" ");
        self.specs.borrow_mut().push(format!(
            "wrap {} about {} along {} radius {} to {}",
            spec.feature,
            axis(spec.axis.origin),
            axis(spec.axis.direction),
            number(spec.radius),
            number(spec.to_radius)
        ));
        let mut shape = self.extrude(&ExtrudeSpec {
            feature: spec.feature,
            frame: spec.frame,
            regions: spec.regions,
            direction: spec.frame.normal(),
            start: 0.0,
            end: 1.0,
        })?;
        shape.history = format!(
            "wrap({}:{}..{})",
            spec.feature,
            number(spec.radius),
            number(spec.to_radius)
        );
        for part in &mut shape.parts {
            part.history = shape.history.clone();
        }
        Ok(shape)
    }

    // Thin extrusions along sketch curves (mitcad#463).

    /// The extrusion's line in `specs` with the chains (`; chains c1>c2
    /// open from [0, 0, 0] | c3 closed`), counted as "extrude_curves"; the
    /// faces the OCCT kernel names, the bounds of the chains' ends and the
    /// first side's distance along the direction.
    fn extrude_curves(
        &self,
        spec: &ExtrudeFeatureSpec<'_, MockShape>,
        chains: &[Vec<PathCurve>],
    ) -> Result<MockShape, KernelError> {
        self.call("extrude_curves")?;
        let closed = |chain: &[PathCurve]| crate::features::path::is_closed(chain);
        let described: Vec<String> = chains
            .iter()
            .map(|chain| {
                let names: Vec<&str> = chain.iter().map(|p| p.name.as_str()).collect();
                match crate::features::path::curve_ends(&chain[0].curve) {
                    Some((start, _)) if !closed(chain) => format!(
                        "{} open from [{}]",
                        names.join(">"),
                        start.map(|v| number(round(v))).join(", ")
                    ),
                    _ => format!("{} closed", names.join(">")),
                }
            })
            .collect();
        self.specs.borrow_mut().push(format!(
            "{}; chains {}",
            describe_extrude(spec),
            described.join(" | ")
        ));
        let key = |name: &str| {
            SegmentKey::closed(
                crate::ids::EntityUid::parse_curve(name).unwrap_or(crate::ids::EntityUid(0)),
            )
        };
        let mut faces = Vec::new();
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for chain in chains {
            for piece in chain {
                for role in ["outer", "inner"] {
                    faces.push(FaceName::new(
                        spec.feature,
                        role,
                        Some(RoleKey::Segment(key(&piece.name))),
                    ));
                }
                if let Some((a, b)) = crate::features::path::curve_ends(&piece.curve) {
                    for p in [a, b] {
                        for i in 0..3 {
                            min[i] = min[i].min(p[i]);
                            max[i] = max[i].max(p[i]);
                        }
                    }
                }
            }
            if !closed(chain) {
                let first = &chain[0].name;
                let last = &chain[chain.len() - 1].name;
                faces.push(FaceName::new(
                    spec.feature,
                    "tip0",
                    Some(RoleKey::Segment(key(first))),
                ));
                faces.push(FaceName::new(
                    spec.feature,
                    "tip1",
                    Some(RoleKey::Segment(key(last))),
                ));
            }
            let region =
                crate::topo::RegionKey::new(chain.iter().map(|p| key(&p.name))).expect("curves");
            faces.push(FaceName::start(spec.feature, region.clone()));
            faces.push(FaceName::end(spec.feature, region));
        }
        let length = match spec.side1.end {
            SideEnd::Distance(d) => d,
            SideEnd::Target(_) => 10.0,
        };
        let far: [f64; 3] = std::array::from_fn(|i| spec.direction[i] * length);
        let bounds = (min[0] <= max[0]).then(|| BoundingBox {
            min: std::array::from_fn(|i| min[i] + far[i].min(0.0)),
            max: std::array::from_fn(|i| max[i] + far[i].max(0.0)),
        });
        Ok(MockShape {
            history: format!("thin_curves({})", spec.feature),
            faces,
            edges: Vec::new(),
            bounds,
            parts: Vec::new(),
        })
    }

    // Boundary Fill (mitcad#457).

    /// Cells by the tools' bounding boxes, counted as "boundary_cells" per
    /// call: a tool holds a point inside its box (a point on a box's side
    /// fails, as one on a tool's boundary does); the cell's box is the box
    /// common to the tools that hold its point, its faces theirs and those
    /// of the other tools whose boxes reach into it, its history
    /// `cell(<the holders' histories>|<the others' that reach it>)`.
    fn boundary_cells(
        &self,
        tools: &[&MockShape],
        points: &[crate::datum::Vec3],
    ) -> Result<Vec<crate::kernel::BoundaryCell<MockShape>>, KernelError> {
        self.call("boundary_cells")?;
        let mut cells = Vec::with_capacity(points.len());
        for p in points {
            let at = format!("({}, {}, {})", number(p[0]), number(p[1]), number(p[2]));
            let mut holders = Vec::new();
            for (i, tool) in tools.iter().enumerate() {
                let Some(b) = tool.bounds else {
                    continue;
                };
                if (0..3).all(|k| p[k] > b.min[k] && p[k] < b.max[k]) {
                    holders.push(i);
                } else if (0..3).all(|k| p[k] >= b.min[k] && p[k] <= b.max[k]) {
                    return Err(KernelError::failed(format!(
                        "the point {at} lies on the boundary of tool {}",
                        i + 1
                    )));
                }
            }
            let Some(bounds) = holders
                .iter()
                .filter_map(|i| tools[*i].bounds)
                .reduce(|a, b| BoundingBox {
                    min: std::array::from_fn(|k| a.min[k].max(b.min[k])),
                    max: std::array::from_fn(|k| a.max[k].min(b.max[k])),
                })
            else {
                return Err(KernelError::failed(format!(
                    "the point {at} is inside no tool body: no enclosed cell there"
                )));
            };
            let reaching: Vec<usize> = (0..tools.len())
                .filter(|i| {
                    !holders.contains(i)
                        && tools[*i].bounds.is_some_and(|b| {
                            (0..3).all(|k| b.min[k] < bounds.max[k] && b.max[k] > bounds.min[k])
                        })
                })
                .collect();
            let histories = |indices: &[usize]| {
                indices
                    .iter()
                    .map(|i| tools[*i].history.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let history = format!(
                "cell({}|{})",
                histories(holders.as_slice()),
                histories(reaching.as_slice())
            );
            let inputs: Vec<&MockShape> =
                holders.iter().chain(&reaching).map(|i| tools[*i]).collect();
            let mut shape = MockShape::merged(history, &inputs, &[]);
            shape.bounds = Some(bounds);
            cells.push(crate::kernel::BoundaryCell { shape, holders });
        }
        Ok(cells)
    }

    // Patch, Stitch and Thicken (mitcad#456). Surface bodies are shapes
    // whose history holds ",sheet" (`body_kind`); a stitch closes into a
    // solid only when `stitch_closes` is set. Each call is described in
    // `specs`.

    /// A sheet of one face `<feature>:patch(<region>)` per region, as flat
    /// as its region.
    fn patch_regions(
        &self,
        feature: FeatureUid,
        frame: &crate::profile::SketchFrame,
        regions: &[crate::profile::ProfileRegion],
    ) -> Result<Vec<MockShape>, KernelError> {
        self.call("patch_regions")?;
        let mut patches = Vec::new();
        for region in regions {
            let (min, max) = region.bounds();
            let corners: Vec<[f64; 3]> = [
                [min[0], min[1]],
                [max[0], min[1]],
                [min[0], max[1]],
                [max[0], max[1]],
            ]
            .iter()
            .map(|p| frame.point(*p))
            .collect();
            self.specs
                .borrow_mut()
                .push(format!("patch_regions({feature}:{})", region.key));
            patches.push(MockShape {
                history: format!("patch({feature}:{},sheet)", region.key),
                faces: vec![FaceName::new(
                    feature,
                    "patch",
                    Some(RoleKey::Region(region.key.clone())),
                )],
                edges: Vec::new(),
                bounds: points_box(&corners),
                parts: Vec::new(),
            });
        }
        Ok(patches)
    }

    /// One closed opening per face of a surface body, in face order; none
    /// of a solid.
    fn open_boundaries(
        &self,
        shape: &MockShape,
    ) -> Result<Vec<crate::surfaces::OpenBoundary>, KernelError> {
        self.call("open_boundaries")?;
        if self.body_kind(shape)? != BodyKind::Sheet {
            return Ok(Vec::new());
        }
        Ok(shape
            .faces
            .iter()
            .map(|face| crate::surfaces::OpenBoundary {
                faces: vec![face.clone()],
                points: Vec::new(),
                closed: true,
            })
            .collect())
    }

    /// A sheet of one face `<feature>:patch` over the body's bounds; the
    /// edges must be the body's and the opening one of its openings.
    fn patch_loop(
        &self,
        spec: &crate::surfaces::PatchLoopSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("patch_loop")?;
        let boundary = match spec.boundary {
            crate::surfaces::PatchBoundary::Edges(edges) => {
                for edge in edges {
                    if spec.body.matching_edges(edge).is_empty() {
                        return Err(KernelError::failed(format!("the body has no edge {edge}")));
                    }
                }
                format!(
                    "edges {}",
                    edges
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
            crate::surfaces::PatchBoundary::Opening(index) => {
                if index >= self.open_boundaries(spec.body)?.len() {
                    return Err(KernelError::failed(format!(
                        "the body has no opening {index}"
                    )));
                }
                format!("opening {index}")
            }
        };
        let own: Vec<String> = spec
            .edge_continuity
            .iter()
            .map(|(edge, g)| format!(" {edge}={}", g.as_str()))
            .collect();
        self.specs.borrow_mut().push(format!(
            "patch_loop({}: {boundary}, {}{}, {} points)",
            spec.feature,
            spec.continuity.as_str(),
            own.concat(),
            spec.points.len()
        ));
        Ok(MockShape {
            history: format!("patch_loop({},sheet)", spec.feature),
            faces: vec![FaceName::new(spec.feature, "patch", None)],
            edges: Vec::new(),
            bounds: spec.body.bounds,
            parts: Vec::new(),
        })
    }

    /// A band named `band_top`, `band_bottom`, `band_side` and `band_edge`
    /// (no edge keys) within the body's bounds; the call is in `specs`
    /// (mitcad#459).
    fn rim_band(
        &self,
        spec: &crate::kernel::RimBandSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("rim_band")?;
        self.specs.borrow_mut().push(format!(
            "rim_band {} on {} {} width {} free {} from {} to {}",
            spec.feature,
            spec.face,
            if spec.outside { "outside" } else { "inside" },
            number(spec.width),
            number(spec.free),
            number(spec.start),
            number(spec.end)
        ));
        if !spec.body.has_face(spec.face) {
            return Err(KernelError::failed(format!(
                "the body has no face {}",
                spec.face
            )));
        }
        let face = |role: &str| FaceName::new(spec.feature, role, None);
        let [top, bottom, side, edge] =
            ["band_top", "band_bottom", "band_side", "band_edge"].map(face);
        Ok(MockShape {
            history: format!(
                "band({}:{}..{})",
                spec.feature,
                number(spec.start),
                number(spec.end)
            ),
            faces: vec![top.clone(), bottom.clone(), side.clone(), edge.clone()],
            edges: vec![
                (side.clone(), top.clone()),
                (side.clone(), bottom.clone()),
                (edge.clone(), top),
                (edge, bottom),
            ],
            bounds: spec.body.bounds,
            parts: Vec::new(),
        })
    }

    /// The shapes' faces and edges together; a surface body unless
    /// `stitch_closes`.
    fn stitch(&self, shapes: &[&MockShape], tolerance: f64) -> Result<MockShape, KernelError> {
        self.call("stitch")?;
        self.specs.borrow_mut().push(format!(
            "stitch({} bodies, {})",
            shapes.len(),
            number(tolerance)
        ));
        let history = if self.stitch_closes.get() {
            format!("stitch({} bodies)", shapes.len())
        } else {
            format!("stitch({} bodies,sheet)", shapes.len())
        };
        Ok(MockShape::merged(history, shapes, &[]))
    }

    /// A solid with `front(<face>)`, `back(<face>)` and `wall(<face>)` for
    /// each face, over the sheet's bounds.
    fn thicken(
        &self,
        feature: FeatureUid,
        shape: &MockShape,
        thickness: f64,
        symmetric: bool,
    ) -> Result<MockShape, KernelError> {
        self.call("thicken")?;
        let side = if symmetric { "symmetric" } else { "one side" };
        self.specs.borrow_mut().push(format!(
            "thicken({feature}: {} faces, {}, {side})",
            shape.faces.len(),
            number(thickness)
        ));
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        for face in &shape.faces {
            let of = |role: &str| {
                FaceName::new(feature, role, Some(RoleKey::Face(Box::new(face.clone()))))
            };
            let (front, back, wall) = (of("front"), of("back"), of("wall"));
            edges.push((front.clone(), wall.clone()));
            edges.push((back.clone(), wall.clone()));
            faces.extend([front, back, wall]);
        }
        Ok(MockShape {
            history: format!("thicken({feature},{},{side})", number(thickness)),
            faces,
            edges,
            bounds: shape.bounds,
            parts: Vec::new(),
        })
    }

    /// Drawings (mitcad#471): the twelve edges of each shape's box, all
    /// seen.
    fn project_view(
        &self,
        shapes: &[&MockShape],
        spec: &crate::drawing::ViewSpec,
    ) -> Result<crate::drawing::ViewProjection, KernelError> {
        use crate::drawing::{ViewCurve, ViewEdge, ViewEdgeKind, ViewProjection};
        self.call("project_view")?;
        let mut projection = ViewProjection::default();
        for shape in shapes {
            let Some(b) = shape.bounds else { continue };
            let corner = |i: usize| {
                [
                    if i & 1 == 0 { b.min[0] } else { b.max[0] },
                    if i & 2 == 0 { b.min[1] } else { b.max[1] },
                    if i & 4 == 0 { b.min[2] } else { b.max[2] },
                ]
            };
            for i in 0..8 {
                for bit in [1, 2, 4] {
                    if i & bit == 0 {
                        projection.edges.push(ViewEdge {
                            visible: true,
                            kind: ViewEdgeKind::Sharp,
                            curve: ViewCurve::Line(
                                spec.project(corner(i)),
                                spec.project(corner(i | bit)),
                            ),
                        });
                    }
                }
            }
        }
        Ok(projection)
    }

    fn round_holes(
        &self,
        _shape: &MockShape,
    ) -> Result<Vec<crate::drawing::RoundHole>, KernelError> {
        Ok(Vec::new())
    }
    /// A face per curve and the caps, over the loops' bounds along the
    /// steps (mitcad#472).
    fn gear_sweep(
        &self,
        spec: &crate::kernel::GearSweepSpec<'_>,
    ) -> Result<MockShape, KernelError> {
        self.call("gear_sweep")?;
        let length: f64 = spec.steps.iter().map(|s| s.length).sum();
        let twist: f64 = spec.steps.iter().map(|s| s.twist).sum();
        self.specs.borrow_mut().push(format!(
            "gear_sweep {} at {:?} curves {} from {} length {} twist {}",
            spec.feature,
            spec.frame.origin,
            spec.loops.iter().map(Vec::len).sum::<usize>(),
            number(spec.start),
            number(length),
            number(twist)
        ));
        let mut points = Vec::new();
        for segment in spec.loops.iter().flatten() {
            for p in crate::features::gear_profile::sample(&segment.geometry, 4) {
                for h in [spec.start, spec.start + length] {
                    let q = spec.frame.point(p);
                    let n = spec.frame.normal();
                    points.push(std::array::from_fn::<f64, 3, _>(|i| q[i] + n[i] * h));
                }
            }
        }
        mock_gear(
            spec.feature,
            spec.loops,
            &[spec.start_name, spec.end_name],
            points,
        )
    }

    fn gear_cone(&self, spec: &crate::kernel::GearConeSpec<'_>) -> Result<MockShape, KernelError> {
        self.call("gear_cone")?;
        self.specs.borrow_mut().push(format!(
            "gear_cone {} at {:?} apex {} scale {}",
            spec.feature,
            spec.frame.origin,
            number(spec.apex),
            number(spec.scale)
        ));
        let mut points = Vec::new();
        for segment in spec.loops.iter().flatten() {
            points.extend(
                crate::features::gear_profile::sample(&segment.geometry, 4)
                    .into_iter()
                    .map(|p| spec.frame.point(p)),
            );
        }
        mock_gear(
            spec.feature,
            spec.loops,
            &["back", "front", "back_cone"],
            points,
        )
    }

    // Surface modelling, surface sweeps (mitcad#455): sheets are shapes
    // whose history holds ",sheet"; each call is described in `specs`.

    /// The shape without the feature's faces in the roles, parts too.
    fn open_faces(
        &self,
        shape: &MockShape,
        feature: FeatureUid,
        roles: &[&str],
    ) -> Result<MockShape, KernelError> {
        self.call("open_faces")?;
        self.specs.borrow_mut().push(format!(
            "open_faces({feature}: {} of {} faces)",
            roles.join(","),
            shape.faces.len()
        ));
        Ok(open_mock(shape, feature, roles))
    }

    fn extrude_sheet(
        &self,
        spec: &ExtrudeFeatureSpec<'_, MockShape>,
        chains: &[Vec<PathCurve>],
    ) -> Result<MockShape, KernelError> {
        self.call("extrude_sheet")?;
        let distance = |side: &ExtrudeSide<MockShape>| match side.end {
            SideEnd::Distance(d) => Ok(d),
            SideEnd::Target(_) => Err(KernelError::Unsupported(
                "a surface extrusion of curves to an object",
            )),
        };
        let one = distance(&spec.side1)?;
        let two = spec
            .side2
            .as_ref()
            .map(distance)
            .transpose()?
            .unwrap_or(0.0);
        self.specs.borrow_mut().push(format!(
            "extrude_sheet({}: {} along [{}] {} back {})",
            spec.feature,
            describe_chains(chains),
            spec.direction.map(|v| number(round(v))).join(", "),
            number(one),
            number(two)
        ));
        Ok(sheet_of_chains(
            format!("extrude_sheet({},sheet)", spec.feature),
            spec.feature,
            chains,
        ))
    }

    fn revolve_sheet(
        &self,
        spec: &RevolveSpec<'_, MockShape>,
        chains: &[Vec<PathCurve>],
    ) -> Result<MockShape, KernelError> {
        self.call("revolve_sheet")?;
        self.specs.borrow_mut().push(format!(
            "revolve_sheet({}: {} angles {} {:?})",
            spec.feature,
            describe_chains(chains),
            number(spec.angle1),
            spec.angle2.map(number)
        ));
        Ok(sheet_of_chains(
            format!("revolve_sheet({},sheet)", spec.feature),
            spec.feature,
            chains,
        ))
    }

    fn sweep_sheet(
        &self,
        spec: &SweepSpec<'_>,
        chains: &[Vec<PathCurve>],
    ) -> Result<MockShape, KernelError> {
        self.call("sweep_sheet")?;
        let path: Vec<&str> = spec.path.iter().map(|p| p.name.as_str()).collect();
        self.specs.borrow_mut().push(format!(
            "sweep_sheet({}: {} path {} extents {} {} {:?} end scale {})",
            spec.feature,
            describe_chains(chains),
            path.join(","),
            number(spec.extent1),
            number(spec.extent2),
            spec.orientation,
            number(spec.end_scale)
        ));
        Ok(sheet_of_chains(
            format!("sweep_sheet({},sheet)", spec.feature),
            spec.feature,
            chains,
        ))
    }

    fn loft_sheet(
        &self,
        feature: FeatureUid,
        sections: &[Vec<PathCurve>],
        ruled: bool,
        closed: bool,
    ) -> Result<MockShape, KernelError> {
        self.call("loft_sheet")?;
        if sections.len() < 2 {
            return Err(KernelError::failed("a loft needs two or more sections"));
        }
        self.specs.borrow_mut().push(format!(
            "loft_sheet({feature}: {} ruled {ruled} closed {closed})",
            describe_chains(sections)
        ));
        Ok(sheet_of_chains(
            format!("loft_sheet({feature},sheet)"),
            feature,
            &sections[..1],
        ))
    }

    // Surface modelling: Offset Surface, Ruled Surface and Extend
    // (mitcad#455).

    /// A sheet of `<feature>:offset(<face>)` for each face (all when none
    /// is named; a named face must be the body's).
    fn offset_surface(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        faces: &[FaceName],
        distance: f64,
        chain: bool,
    ) -> Result<MockShape, KernelError> {
        self.call("offset_surface")?;
        for face in faces {
            if !body.has_face(face) {
                return Err(KernelError::failed(format!("the body has no face {face}")));
            }
        }
        let offset: Vec<&FaceName> = if faces.is_empty() {
            body.faces.iter().collect()
        } else {
            body.faces
                .iter()
                .filter(|f| faces.iter().any(|r| f.matches(r)))
                .collect()
        };
        self.specs.borrow_mut().push(format!(
            "offset_surface({feature}: {} faces, {}, chain {chain})",
            offset.len(),
            number(distance)
        ));
        Ok(MockShape {
            history: format!("offset_surface({feature},sheet)"),
            faces: offset
                .into_iter()
                .map(|f| FaceName::new(feature, "offset", Some(RoleKey::Face(Box::new(f.clone())))))
                .collect(),
            edges: Vec::new(),
            bounds: body.bounds,
            parts: Vec::new(),
        })
    }

    /// A sheet of `<feature>:ruled(<edge>)` for each edge (the body's).
    fn ruled_surface(
        &self,
        spec: &crate::surface_modelling::RuledSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("ruled_surface")?;
        for edge in spec.edges {
            if spec.body.matching_edges(edge).is_empty() {
                return Err(KernelError::failed(format!("the body has no edge {edge}")));
            }
        }
        self.specs.borrow_mut().push(format!(
            "ruled_surface({}: {} edges, {} openings, {}, {:?} angle {} direction {:?} flip {} {:?})",
            spec.feature,
            spec.edges.len(),
            spec.openings.len(),
            number(spec.distance),
            spec.mode,
            number(spec.angle),
            spec.direction,
            spec.flip,
            spec.corners
        ));
        Ok(MockShape {
            history: format!("ruled_surface({},sheet)", spec.feature),
            faces: spec
                .edges
                .iter()
                .map(|e| {
                    FaceName::new(
                        spec.feature,
                        "ruled",
                        Some(RoleKey::Edge(Box::new(e.clone()))),
                    )
                })
                // A free edge's strip after its face (an opening is a
                // face's here, `open_boundaries`).
                .chain(spec.openings.iter().filter_map(|o| {
                    spec.body.faces.get(o.index).map(|f| {
                        FaceName::new(
                            spec.feature,
                            "ruled",
                            Some(RoleKey::Face(Box::new(f.clone()))),
                        )
                    })
                }))
                .collect(),
            edges: Vec::new(),
            bounds: spec.body.bounds,
            parts: Vec::new(),
        })
    }

    /// The body with its faces and edges, its history marked extended.
    fn extend_surface(
        &self,
        body: &MockShape,
        edges: &[EdgeName],
        openings: &[crate::surface_modelling::OpeningEdges<'_>],
        distance: f64,
        mode: crate::surface_modelling::ExtendMode,
    ) -> Result<MockShape, KernelError> {
        self.call("extend_surface")?;
        for edge in edges {
            if body.matching_edges(edge).is_empty() {
                return Err(KernelError::failed(format!("the body has no edge {edge}")));
            }
        }
        self.specs.borrow_mut().push(format!(
            "extend_surface({} edges, {} openings, {}, {})",
            edges.len(),
            openings.len(),
            number(distance),
            mode.as_str()
        ));
        let mut extended = body.clone();
        extended.history = format!("extend({},sheet)", body.history);
        Ok(extended)
    }

    // Surface modelling: Trim, Untrim, Unstitch and Reverse Normal
    // (mitcad#455).

    /// The body with its faces, its history marked trimmed; the tool and
    /// the keep points described.
    fn trim_surface(
        &self,
        spec: &crate::surface_modelling::TrimSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("trim_surface")?;
        let tool = match &spec.tool {
            crate::surface_modelling::TrimTool::Tool(ToolInput::Plane { .. }) => "plane".to_owned(),
            crate::surface_modelling::TrimTool::Tool(ToolInput::Face { face, .. }) => {
                format!("face {face}")
            }
            crate::surface_modelling::TrimTool::Tool(ToolInput::Body { .. }) => "body".to_owned(),
            crate::surface_modelling::TrimTool::Tool(ToolInput::Curves { curves, .. }) => {
                format!("regions of {} curves", curves.len())
            }
            crate::surface_modelling::TrimTool::Curves { chains, .. } => {
                format!("curves {}", describe_chains(chains))
            }
        };
        self.specs.borrow_mut().push(format!(
            "trim_surface({tool}, {} keep points, extend {})",
            spec.keep.len(),
            spec.extend
        ));
        let mut trimmed = spec.body.clone();
        trimmed.history = format!("trim({},sheet)", spec.body.history);
        Ok(trimmed)
    }

    /// The body with the face (in place), or a sheet of
    /// `<feature>:untrim(<face>)`.
    fn untrim_surface(
        &self,
        feature: FeatureUid,
        body: &MockShape,
        face: &FaceName,
        holes: bool,
        outer: bool,
        in_place: bool,
    ) -> Result<MockShape, KernelError> {
        self.call("untrim_surface")?;
        if !body.has_face(face) {
            return Err(KernelError::failed(format!("the body has no face {face}")));
        }
        self.specs.borrow_mut().push(format!(
            "untrim_surface({face}, holes {holes}, outer {outer}, in place {in_place})"
        ));
        if in_place {
            let mut untrimmed = body.clone();
            untrimmed.history = format!("untrim({},sheet)", body.history);
            return Ok(untrimmed);
        }
        Ok(MockShape {
            history: format!("untrim({feature},sheet)"),
            faces: vec![FaceName::new(
                feature,
                "untrim",
                Some(RoleKey::Face(Box::new(face.clone()))),
            )],
            edges: Vec::new(),
            bounds: body.bounds,
            parts: Vec::new(),
        })
    }

    /// The faces left and a sheet of each named face (every face when none
    /// is named).
    fn unstitch(
        &self,
        body: &MockShape,
        faces: &[FaceName],
    ) -> Result<crate::kernel::Unstitched<MockShape>, KernelError> {
        self.call("unstitch")?;
        for face in faces {
            if !body.has_face(face) {
                return Err(KernelError::failed(format!("the body has no face {face}")));
            }
        }
        let apart = |face: &FaceName| MockShape {
            history: format!("unstitch({face},sheet)"),
            faces: vec![face.clone()],
            edges: Vec::new(),
            bounds: body.bounds,
            parts: Vec::new(),
        };
        self.specs
            .borrow_mut()
            .push(format!("unstitch({} faces)", faces.len()));
        if faces.is_empty() {
            return Ok((None, body.faces.iter().map(apart).collect()));
        }
        let left: Vec<FaceName> = body
            .faces
            .iter()
            .filter(|f| !faces.iter().any(|r| f.matches(r)))
            .cloned()
            .collect();
        let rest = (!left.is_empty()).then(|| MockShape {
            history: format!("unstitch_rest({},sheet)", body.history),
            edges: body
                .edges
                .iter()
                .filter(|(a, b)| left.contains(a) && left.contains(b))
                .cloned()
                .collect(),
            faces: left,
            bounds: body.bounds,
            parts: Vec::new(),
        });
        let pieces = body
            .faces
            .iter()
            .filter(|f| faces.iter().any(|r| f.matches(r)))
            .map(apart)
            .collect();
        Ok((rest, pieces))
    }

    /// The body with its history marked reversed.
    fn reverse_faces(
        &self,
        body: &MockShape,
        faces: &[FaceName],
    ) -> Result<MockShape, KernelError> {
        self.call("reverse_faces")?;
        for face in faces {
            if !body.has_face(face) {
                return Err(KernelError::failed(format!("the body has no face {face}")));
            }
        }
        self.specs
            .borrow_mut()
            .push(format!("reverse_faces({} faces)", faces.len()));
        let mut turned = body.clone();
        turned.history = format!("reversed({})", body.history);
        Ok(turned)
    }

    // Surface modelling: Blend Surface, Boundary Surface, Match Surface and
    // Fair Surface (mitcad#455).

    /// A sheet of one face `<feature>:blend`; the ends described.
    fn blend_surface(
        &self,
        spec: &crate::surface_modelling::BlendSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("blend_surface")?;
        let mut ends = Vec::new();
        for end in [&spec.from, &spec.to] {
            ends.push(describe_body_edges(&end.edges)?);
            ends.push(format!(
                "{} bulge {}",
                end.continuity.as_str(),
                number(end.bulge)
            ));
        }
        self.specs.borrow_mut().push(format!(
            "blend_surface({}: {})",
            spec.feature,
            ends.join(", ")
        ));
        Ok(one_face_sheet(spec.feature, "blend"))
    }

    /// A sheet of one face `<feature>:boundary`; the curves described.
    fn boundary_surface(
        &self,
        spec: &crate::surface_modelling::BoundarySpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        use crate::surface_modelling::{BoundaryCurve, BoundaryCurves};
        self.call("boundary_surface")?;
        let describe = |curves: &[BoundaryCurve<'_, MockShape>]| -> Result<String, KernelError> {
            let mut parts = Vec::new();
            for curve in curves {
                parts.push(match curve {
                    BoundaryCurve::Curves {
                        chain, continuity, ..
                    } => format!(
                        "curves {} {}",
                        describe_chains(std::slice::from_ref(&chain.to_vec())),
                        continuity.as_str()
                    ),
                    BoundaryCurve::Edges { edges, continuity } => {
                        format!("{} {}", describe_body_edges(edges)?, continuity.as_str())
                    }
                    BoundaryCurve::Point(p) => format!(
                        "point ({}, {}, {})",
                        number(p[0]),
                        number(p[1]),
                        number(p[2])
                    ),
                });
            }
            Ok(parts.join("; "))
        };
        let text = match spec.curves {
            BoundaryCurves::Loop(curves) => format!("loop [{}]", describe(curves)?),
            BoundaryCurves::Network { u, v } => {
                format!("u [{}] v [{}]", describe(u)?, describe(v)?)
            }
        };
        self.specs
            .borrow_mut()
            .push(format!("boundary_surface({}: {text})", spec.feature));
        Ok(one_face_sheet(spec.feature, "boundary"))
    }

    /// The body with its history marked matched; the face must be the
    /// body's (its only one when none is named).
    fn match_surface(
        &self,
        body: &MockShape,
        face: Option<&FaceName>,
        target: &crate::surface_modelling::BodyEdges<'_, MockShape>,
        continuity: crate::surfaces::PatchContinuity,
    ) -> Result<MockShape, KernelError> {
        self.call("match_surface")?;
        match face {
            Some(face) if !body.has_face(face) => {
                return Err(KernelError::failed(format!("the body has no face {face}")));
            }
            None if body.faces.len() != 1 => {
                return Err(KernelError::failed(format!(
                    "the body has {} faces; name the one to match",
                    body.faces.len()
                )));
            }
            _ => {}
        }
        self.specs.borrow_mut().push(format!(
            "match_surface({} to {}, {})",
            face.map_or_else(|| "its face".to_owned(), ToString::to_string),
            describe_body_edges(target)?,
            continuity.as_str()
        ));
        let mut matched = body.clone();
        matched.history = format!("matched({},sheet)", body.history);
        Ok(matched)
    }

    /// The body with its history marked faired.
    fn fair_surface(
        &self,
        body: &MockShape,
        faces: &[FaceName],
        fairing: crate::surface_modelling::Fairing,
    ) -> Result<MockShape, KernelError> {
        self.call("fair_surface")?;
        for face in faces {
            if !body.has_face(face) {
                return Err(KernelError::failed(format!("the body has no face {face}")));
            }
        }
        self.specs.borrow_mut().push(format!(
            "fair_surface({} faces, strength {}, {} iterations, keep edges {}, {})",
            faces.len(),
            number(fairing.strength),
            fairing.iterations,
            fairing.keep_edges,
            number(fairing.tolerance)
        ));
        let mut faired = body.clone();
        faired.history = format!("faired({})", body.history);
        Ok(faired)
    }

    // Faces moved by a rigid motion (mitcad#343).

    /// The body with its history marked moved by the translation.
    fn move_faces(
        &self,
        _feature: FeatureUid,
        body: &MockShape,
        faces: &[FaceName],
        motion: &Transform,
    ) -> Result<MockShape, KernelError> {
        self.call("move_faces")?;
        for face in faces {
            if !body.has_face(face) {
                return Err(KernelError::failed(format!("the body has no face {face}")));
            }
        }
        let [x, y, z] = motion.translation;
        Ok(MockShape {
            history: format!(
                "move_faces({},{} faces,{},{},{})",
                body.history,
                faces.len(),
                number(x),
                number(y),
                number(z)
            ),
            ..body.clone()
        })
    }

    // Extrusions of a planar body face (mitcad#535).

    /// A prism of the face, `face_prism(<feature>:<face>:<low>..<high>)`
    /// along the outward normal (`-` reversed), its sides named after the
    /// body's edges of the face and its caps after the key. A face with a
    /// surface other than a plane in `surfaces` is not planar; a mesh body's
    /// face has no surface.
    fn extrude_face(
        &self,
        spec: &crate::kernel::FaceExtrudeSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("extrude_face")?;
        let body = spec.body;
        let face = spec.face;
        let matching: Vec<&FaceName> = body.faces.iter().filter(|f| f.matches(face)).collect();
        if matching.is_empty() {
            return Err(KernelError::failed(format!("the body has no face {face}")));
        }
        if matching.len() > 1 {
            return Err(KernelError::failed(format!(
                "{face} names {} faces; an extrusion takes one face",
                matching.len()
            )));
        }
        if matches!(self.body_kind(body), Ok(BodyKind::Mesh)) {
            return Err(KernelError::failed(format!(
                "face {face} has no surface (a mesh body's triangles)"
            )));
        }
        // A curved face only along a direction, without a taper or a start
        // offset (mitcad#541).
        let tapered = spec.taper1 != 0.0 || (spec.side2.is_some() && spec.taper2 != 0.0);
        if let Some(surface) = self.surfaces.borrow().get(&face.to_string())
            && !matches!(surface, SurfaceGeometry::Plane { .. })
            && (spec.direction.is_none() || tapered || spec.start != 0.0)
        {
            return Err(KernelError::failed(format!(
                "face {face} is not planar: an extrusion takes a planar face"
            )));
        }
        let (low, high) = match spec.side2 {
            Some(two) => (-two, spec.side1),
            None => (0.0, spec.side1),
        };
        self.specs.borrow_mut().push(format!(
            "extrude_face {} {face} key {} {}from {}: {}{}{}{}",
            spec.feature,
            spec.key,
            if spec.reversed { "reversed " } else { "" },
            number(spec.start),
            number(spec.side1),
            spec.side2
                .map(|d| format!("; {}", number(d)))
                .unwrap_or_default(),
            spec.direction
                .map(|d| format!(" along [{}]", d.map(|v| number(round(v))).join(", ")))
                .unwrap_or_default(),
            if tapered {
                format!(
                    " taper {} {}",
                    number(round(spec.taper1)),
                    number(round(spec.taper2))
                )
            } else {
                String::new()
            }
        ));
        let key = || Some(RoleKey::Face(Box::new(spec.key.clone())));
        let start = FaceName::new(spec.feature, "start", key());
        let end = FaceName::new(spec.feature, "end", key());
        let sides: Vec<FaceName> = body
            .edges
            .iter()
            .filter(|(a, b)| a.matches(face) || b.matches(face))
            .map(|(a, b)| {
                let edge = EdgeName::new(a.clone(), b.clone());
                FaceName::new(spec.feature, "side", Some(RoleKey::Edge(Box::new(edge))))
            })
            .collect();
        let mut edges = Vec::new();
        for (i, side) in sides.iter().enumerate() {
            edges.push((side.clone(), sides[(i + 1) % sides.len()].clone()));
            edges.push((side.clone(), start.clone()));
            edges.push((side.clone(), end.clone()));
        }
        let mut faces = vec![start, end];
        faces.extend(sides);
        // The body's bounds, across a face `face_plane` knows (an axis-aligned
        // plane) from the start to the far end along the sweep.
        let bounds = match (body.bounds, self.face_plane(body, face)) {
            (Some(mut b), Ok(plane)) => {
                if let Some(k) = (0..3).find(|k| plane.normal[*k].abs() > 0.5) {
                    let along = if spec.reversed { -1.0 } else { 1.0 } * plane.normal[k];
                    let at = plane.origin[k] + plane.normal[k] * spec.start;
                    let (p, q) = (at + along * low, at + along * high);
                    b.min[k] = p.min(q);
                    b.max[k] = p.max(q);
                }
                Some(b)
            }
            (bounds, _) => bounds,
        };
        Ok(MockShape {
            history: format!(
                "face_prism({}:{face}:{}{}..{})",
                spec.feature,
                if spec.reversed { "-" } else { "" },
                number(spec.start + low),
                number(spec.start + high)
            ),
            faces,
            edges,
            bounds,
            parts: Vec::new(),
        })
    }

    // Revolutions of a planar body face (mitcad#539).

    /// A revolution of the face, `face_revolution(<feature>:<face>:<angle1>)`
    /// (`<angle1>+<angle2>` with two sides), its sides named after the
    /// body's edges of the face and, short of a full turn, its caps after
    /// the key; the body's bounds. A face with a surface other than a plane
    /// in `surfaces` is not planar; a mesh body's face has no surface.
    fn revolve_face(
        &self,
        spec: &crate::kernel::FaceRevolveSpec<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("revolve_face")?;
        let body = spec.body;
        let face = spec.face;
        let matching: Vec<&FaceName> = body.faces.iter().filter(|f| f.matches(face)).collect();
        if matching.is_empty() {
            return Err(KernelError::failed(format!("the body has no face {face}")));
        }
        if matching.len() > 1 {
            return Err(KernelError::failed(format!(
                "{face} names {} faces; a revolution takes one face",
                matching.len()
            )));
        }
        if matches!(self.body_kind(body), Ok(BodyKind::Mesh)) {
            return Err(KernelError::failed(format!(
                "face {face} has no surface (a mesh body's triangles)"
            )));
        }
        if let Some(surface) = self.surfaces.borrow().get(&face.to_string())
            && !matches!(surface, SurfaceGeometry::Plane { .. })
        {
            return Err(KernelError::failed(format!(
                "face {face} is not planar: a revolution takes a planar face"
            )));
        }
        self.specs.borrow_mut().push(format!(
            "revolve_face {} {face} key {} axis {:?} {:?} angles {} {:?}",
            spec.feature,
            spec.key,
            spec.axis.origin,
            spec.axis.direction,
            number(spec.angle1),
            spec.angle2
        ));
        let full = spec.angle1 + spec.angle2.unwrap_or(0.0) >= std::f64::consts::TAU - 1e-9;
        let key = || Some(RoleKey::Face(Box::new(spec.key.clone())));
        let start = FaceName::new(spec.feature, "start", key());
        let end = FaceName::new(spec.feature, "end", key());
        let sides: Vec<FaceName> = body
            .edges
            .iter()
            .filter(|(a, b)| a.matches(face) || b.matches(face))
            .map(|(a, b)| {
                let edge = EdgeName::new(a.clone(), b.clone());
                FaceName::new(spec.feature, "side", Some(RoleKey::Edge(Box::new(edge))))
            })
            .collect();
        let mut edges = Vec::new();
        for (i, side) in sides.iter().enumerate() {
            edges.push((side.clone(), sides[(i + 1) % sides.len()].clone()));
            if !full {
                edges.push((side.clone(), start.clone()));
                edges.push((side.clone(), end.clone()));
            }
        }
        let mut faces = if full { Vec::new() } else { vec![start, end] };
        faces.extend(sides);
        Ok(MockShape {
            history: format!(
                "face_revolution({}:{face}:{}{})",
                spec.feature,
                number(spec.angle1),
                spec.angle2
                    .map(|a| format!("+{}", number(a)))
                    .unwrap_or_default()
            ),
            faces,
            edges,
            bounds: body.bounds,
            parts: Vec::new(),
        })
    }

    // Sweeps of a planar body face (mitcad#543).

    fn face_sweep(
        &self,
        spec: &SweepSpec<'_>,
        profile: &crate::sweeps::ProfileFace<'_, MockShape>,
    ) -> Result<MockShape, KernelError> {
        self.call("face_sweep")?;
        let (body, face) = (profile.body, profile.face);
        let matching = body.faces.iter().filter(|f| f.matches(face)).count();
        if matching != 1 {
            return Err(KernelError::failed(format!(
                "{face} names {matching} faces; a sweep takes one face"
            )));
        }
        let names: Vec<&str> = spec.path.iter().map(|p| p.name.as_str()).collect();
        self.specs.borrow_mut().push(format!(
            "face_sweep {} {face} key {} path {} {:?}",
            spec.feature,
            profile.key,
            names.join(","),
            spec.orientation
        ));
        let key = || Some(RoleKey::Face(Box::new(profile.key.clone())));
        let start = FaceName::new(spec.feature, "start", key());
        let end = FaceName::new(spec.feature, "end", key());
        let sides: Vec<FaceName> = body
            .edges
            .iter()
            .filter(|(a, b)| a.matches(face) || b.matches(face))
            .map(|(a, b)| {
                let edge = EdgeName::new(a.clone(), b.clone());
                FaceName::new(spec.feature, "side", Some(RoleKey::Edge(Box::new(edge))))
            })
            .collect();
        let mut edges = Vec::new();
        for (i, side) in sides.iter().enumerate() {
            edges.push((side.clone(), sides[(i + 1) % sides.len()].clone()));
            edges.push((side.clone(), start.clone()));
            edges.push((side.clone(), end.clone()));
        }
        let mut faces = vec![start, end];
        faces.extend(sides);
        Ok(MockShape {
            history: format!("face_sweep({}:{face})", spec.feature),
            faces,
            edges,
            bounds: None,
            parts: Vec::new(),
        })
    }
}

/// A chain of a body's edges as `3 edges, 1 openings` (the named edges
/// must be the body's).
fn describe_body_edges(
    edges: &crate::surface_modelling::BodyEdges<'_, MockShape>,
) -> Result<String, KernelError> {
    for edge in edges.edges {
        if edges.body.matching_edges(edge).is_empty() {
            return Err(KernelError::failed(format!("the body has no edge {edge}")));
        }
    }
    Ok(format!(
        "{} edges, {} openings",
        edges.edges.len(),
        edges.openings.len()
    ))
}

/// A surface body of one face `<feature>:<role>`.
fn one_face_sheet(feature: FeatureUid, role: &str) -> MockShape {
    MockShape {
        history: format!("{role}({feature},sheet)"),
        faces: vec![FaceName::new(feature, role, None)],
        edges: Vec::new(),
        bounds: None,
        parts: Vec::new(),
    }
}

/// [`MockKernel::open_faces`] of a shape and its parts.
fn open_mock(shape: &MockShape, feature: FeatureUid, roles: &[&str]) -> MockShape {
    let dropped = |face: &FaceName| face.feature == feature && roles.contains(&face.role.as_str());
    MockShape {
        history: format!("open({},sheet)", shape.history),
        faces: shape
            .faces
            .iter()
            .filter(|f| !dropped(f))
            .cloned()
            .collect(),
        edges: shape
            .edges
            .iter()
            .filter(|(a, b)| !dropped(a) && !dropped(b))
            .cloned()
            .collect(),
        bounds: shape.bounds,
        parts: shape
            .parts
            .iter()
            .map(|p| open_mock(p, feature, roles))
            .collect(),
    }
}

/// Chains as `c1>c2 | c3` (mitcad#455).
fn describe_chains(chains: &[Vec<PathCurve>]) -> String {
    chains
        .iter()
        .map(|chain| {
            chain
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(">")
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// A sheet of one face `<feature>:side(<curve>)` per curve of the chains,
/// neighbours in a chain meeting in an edge.
fn sheet_of_chains(history: String, feature: FeatureUid, chains: &[Vec<PathCurve>]) -> MockShape {
    let side = |name: &str| {
        FaceName::side(
            feature,
            SegmentKey::closed(
                crate::ids::EntityUid::parse_curve(name).unwrap_or(crate::ids::EntityUid(0)),
            ),
        )
    };
    let mut faces = Vec::new();
    let mut edges = Vec::new();
    for chain in chains {
        for (i, piece) in chain.iter().enumerate() {
            faces.push(side(&piece.name));
            if i > 0 {
                edges.push((side(&chain[i - 1].name), side(&piece.name)));
            }
        }
    }
    MockShape {
        history,
        faces,
        edges,
        bounds: None,
        parts: Vec::new(),
    }
}

/// A mock gear body: a face per curve and the given ones.
fn mock_gear(
    feature: FeatureUid,
    loops: &[Vec<crate::features::gear_profile::NamedSegment>],
    caps: &[&str],
    points: Vec<[f64; 3]>,
) -> Result<MockShape, KernelError> {
    let mut faces: Vec<FaceName> = caps
        .iter()
        .map(|c| FaceName::new(feature, c, None))
        .collect();
    for segment in loops.iter().flatten() {
        let name: FaceName = format!("{feature}:{}", segment.name)
            .parse()
            .map_err(|_| KernelError::failed(format!("bad face name {}", segment.name)))?;
        if !faces.contains(&name) {
            faces.push(name);
        }
    }
    let bounds = (!points.is_empty()).then(|| BoundingBox {
        min: std::array::from_fn(|i| points.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min)),
        max: std::array::from_fn(|i| {
            points
                .iter()
                .map(|p| p[i])
                .fold(f64::NEG_INFINITY, f64::max)
        }),
    });
    Ok(MockShape {
        history: format!("gear({feature})"),
        faces,
        edges: Vec::new(),
        bounds,
        parts: Vec::new(),
    })
}

/// A mock shape with its faces renamed ([`MockKernel::rename_faces`]).
fn renamed(
    shape: &MockShape,
    wrap: &Option<(FeatureUid, String)>,
    renames: &[(FaceName, FaceName)],
) -> MockShape {
    let rename = |face: &FaceName| -> FaceName {
        if let Some((_, to)) = renames.iter().find(|(from, _)| from == face) {
            return to.clone();
        }
        match wrap {
            Some((feature, role)) => {
                FaceName::new(*feature, role, Some(RoleKey::Face(Box::new(face.clone()))))
            }
            None => face.clone(),
        }
    };
    let mut faces: Vec<FaceName> = Vec::new();
    for face in &shape.faces {
        let new = rename(face);
        if !faces.contains(&new) {
            faces.push(new);
        }
    }
    MockShape {
        history: shape.history.clone(),
        faces,
        edges: shape
            .edges
            .iter()
            .map(|(a, b)| (rename(a), rename(b)))
            .collect(),
        bounds: shape.bounds,
        parts: shape
            .parts
            .iter()
            .map(|p| renamed(p, wrap, renames))
            .collect(),
    }
}

/// The role a mock face's geometry follows: a derived part's face named
/// after its source's (`F7:part(F2:end(…))`) is that face's; the end faces
/// of derived parts are an extrusion's start and end.
/// A box primitive's face (mitcad#524, `MockKernel::box_faces`): the axis
/// it lies across and whether on the bounds' high side.
fn box_side(role: &str) -> Option<(usize, bool)> {
    match role {
        "bottom" => Some((2, false)),
        "top" => Some((2, true)),
        "side0" => Some((1, false)),
        "side1" => Some((0, true)),
        "side2" => Some((1, true)),
        "side3" => Some((0, false)),
        _ => None,
    }
}

fn plain_role(face: &FaceName) -> &str {
    match (face.role.as_str(), &face.key) {
        ("part", Some(RoleKey::Face(inner))) => plain_role(inner),
        (role, _) => role,
    }
}

/// The shape of mock B-rep data ("<faces> <history>"), face i named
/// `<feature>:import(<first_face + i>)`, and its number of faces.
fn read_mock_brep(
    feature: FeatureUid,
    data: &[u8],
    first_face: u32,
) -> Result<(MockShape, u32), KernelError> {
    let text = std::str::from_utf8(data).map_err(|_| KernelError::failed("not mock data"))?;
    let (count, history) = text
        .split_once(' ')
        .ok_or_else(|| KernelError::failed("not mock data"))?;
    let count: u32 = count
        .parse()
        .map_err(|_| KernelError::failed("not mock data"))?;
    let face = |i: u32| FaceName::new(feature, "import", Some(RoleKey::Index(first_face + i)));
    let shape = MockShape {
        history: format!(
            "import({feature}:{first_face}..{},{history})",
            first_face + count
        ),
        faces: (0..count).map(face).collect(),
        edges: (0..count)
            .map(|i| (face(i), face((i + 1) % count)))
            .collect(),
        bounds: Some(BoundingBox {
            min: [0.0; 3],
            max: [10.0; 3],
        }),
        parts: Vec::new(),
    };
    Ok((shape, count))
}

/// The box of points (None without points).
fn points_box(points: &[[f64; 3]]) -> Option<BoundingBox> {
    let first = *points.first()?;
    let mut b = BoundingBox {
        min: first,
        max: first,
    };
    for p in points {
        for (k, c) in p.iter().enumerate() {
            b.min[k] = b.min[k].min(*c);
            b.max[k] = b.max[k].max(*c);
        }
    }
    Some(b)
}

/// A mock shape as the mock kernel stores it.
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredMock {
    history: String,
    faces: Vec<FaceName>,
    edges: Vec<(FaceName, FaceName)>,
    bounds: Option<([f64; 3], [f64; 3])>,
    parts: Vec<StoredMock>,
}

impl StoredMock {
    fn new(shape: &MockShape) -> Self {
        Self {
            history: shape.history.clone(),
            faces: shape.faces.clone(),
            edges: shape.edges.clone(),
            bounds: shape.bounds.map(|b| (b.min, b.max)),
            parts: shape.parts.iter().map(Self::new).collect(),
        }
    }

    fn shape(self) -> MockShape {
        MockShape {
            history: self.history,
            faces: self.faces,
            edges: self.edges,
            bounds: self.bounds.map(|(min, max)| BoundingBox { min, max }),
            parts: self.parts.into_iter().map(Self::shape).collect(),
        }
    }
}

/// A copy of a mock shape moved by `transform`, faces renamed by `instance`.
fn transformed(shape: &MockShape, transform: &Transform, instance: Option<Instance>) -> MockShape {
    let rename = |face: &FaceName| match instance {
        Some(instance) => instance.face(face.clone()),
        None => face.clone(),
    };
    let bounds = shape.bounds.map(|b| {
        let corners: Vec<[f64; 3]> = b.corners().map(|c| transform.apply_point(c)).collect();
        BoundingBox {
            min: std::array::from_fn(|i| {
                corners.iter().map(|c| c[i]).fold(f64::INFINITY, f64::min)
            }),
            max: std::array::from_fn(|i| {
                corners
                    .iter()
                    .map(|c| c[i])
                    .fold(f64::NEG_INFINITY, f64::max)
            }),
        }
    });
    let t = transform.translation;
    let tag = match instance {
        Some(instance) => format!(",inst{}", instance.index),
        None => String::new(),
    };
    MockShape {
        history: format!(
            "moved({},{},{},{}{tag})",
            shape.history,
            number(round(t[0])),
            number(round(t[1])),
            number(round(t[2]))
        ),
        faces: shape.faces.iter().map(rename).collect(),
        edges: shape
            .edges
            .iter()
            .map(|(a, b)| (rename(a), rename(b)))
            .collect(),
        bounds,
        parts: shape
            .parts
            .iter()
            .map(|p| transformed(p, transform, instance))
            .collect(),
    }
}

/// Rounded to 1e-9 so that histories compare.
fn round(value: f64) -> f64 {
    let rounded = (value * 1e9).round() / 1e9;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// One line about an extrusion's spec, for tests to look at.
fn describe_extrude(spec: &ExtrudeFeatureSpec<'_, MockShape>) -> String {
    let start = match &spec.start {
        ExtrudeStart::Offset(offset) => format!("offset {}", number(*offset)),
        ExtrudeStart::Object(bound) => format!("object {}", describe_target(&bound.target)),
    };
    let side = |side: &ExtrudeSide<MockShape>| {
        let end = match &side.end {
            SideEnd::Distance(d) => format!("distance {}", number(*d)),
            SideEnd::Target(bound) => format!(
                "to {} offset {}",
                describe_target(&bound.target),
                number(bound.offset)
            ),
        };
        let thin = side
            .thin
            .map(|w| match w.location {
                WallLocation::Both => format!(
                    " thin Both {} {}",
                    number(w.thickness),
                    number(w.thickness2)
                ),
                _ => format!(" thin {:?} {}", w.location, number(w.thickness)),
            })
            .unwrap_or_default();
        format!("{end} taper {}{thin}", number(side.taper))
    };
    let mut text = format!(
        "extrude {} along {:?} from {start}: {}",
        spec.feature,
        spec.direction,
        side(&spec.side1)
    );
    if let Some(two) = &spec.side2 {
        text.push_str(&format!("; {}", side(two)));
    }
    text
}

fn describe_target(target: &Target<MockShape>) -> String {
    match target {
        Target::Plane(plane) => format!("plane {:?} {:?}", plane.origin, plane.normal),
        Target::Face { face, extend, .. } => format!("face {face} extend {extend}"),
        Target::Body { body, through } => format!("body {} through {through}", body.history),
    }
}
