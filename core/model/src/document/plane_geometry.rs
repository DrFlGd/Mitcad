// SPDX-License-Identifier: MIT
//! Body edges and vertices in a sketch's plane (mitcad#256, mitcad#258):
//! what the sketch Dimension tool picks and the Point tool snaps to, in
//! sketch coordinates, each with what `sketch.project` needs to project
//! it. The kernel lists a body's edges and vertices and their curves
//! ([`Kernel::edges`], [`Kernel::curves_of`], [`Kernel::indexed_element`]);
//! whether they lie in the plane is decided here, within the model's linear
//! tolerance.

use std::collections::BTreeSet;

use super::{Document, ModelError, invalid};
use crate::features::FeatureDef;
use crate::ids::{BodyUid, FeatureUid, OccurrenceUid};
use crate::kernel::{Curve3, ElementKind, Kernel};
use crate::profile::{SketchFrame, cross, dot};
use crate::sketch::geometry::{Curve2, P2};
use crate::sketch::project::{Projected, project, transform_curve};
use crate::topo::TopoName;
use crate::transform::Transform;

/// A body edge or vertex that lies in a sketch's plane.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaneItem {
    /// The edge's or the vertex's name: `sketch.project`'s `source`.
    pub source: TopoName,
    pub body: BodyUid,
    /// Where a body of another component was found: the occurrence path
    /// from the root to its component (`sketch.project`'s `occurrence`).
    /// None for the sketch's own component, which needs no link.
    pub occurrence: Option<Vec<OccurrenceUid>>,
    pub shape: PlaneShape,
}

/// Where a [`PlaneItem`] is, in sketch coordinates: what projecting it
/// gives.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaneShape {
    Vertex(P2),
    Curve(Curve2),
}

/// How far a model curve is from the plane through `origin` with the unit
/// `normal`, at most: zero for a curve in it.
fn distance_from_plane(curve: &Curve3, origin: [f64; 3], normal: [f64; 3]) -> f64 {
    let off = |p: [f64; 3]| dot(std::array::from_fn(|i| p[i] - origin[i]), normal).abs();
    match curve {
        Curve3::Point(p) => off(*p),
        Curve3::Line { start, end } => off(*start).max(off(*end)),
        // The whole conic: an arc of a circle that leaves the plane meets it
        // in two points only, so it lies in the plane only with its circle.
        Curve3::Conic {
            center,
            normal: axis,
            x_axis,
            major,
            minor,
            ..
        } => {
            let y_axis = cross(*axis, *x_axis);
            off(*center) + (major * dot(*x_axis, normal)).hypot(minor * dot(y_axis, normal))
        }
        // A B-spline lies in a plane exactly when its poles do (its basis
        // functions are independent; weights are positive).
        Curve3::BSpline { poles, .. } => poles.iter().map(|p| off(*p)).fold(0.0, f64::max),
    }
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = dot(v, v).sqrt();
    if n > 0.0 {
        std::array::from_fn(|i| v[i] / n)
    } else {
        v
    }
}

/// A body to look at: its shape in its component's coordinates and the
/// transform into the sketch's.
struct Seen<'a, S> {
    body: BodyUid,
    shape: &'a S,
    occurrence: Option<Vec<OccurrenceUid>>,
    to_sketch: Option<Transform>,
}

impl<K: Kernel> Document<K> {
    /// The edges and vertices of the visible bodies before `sketch` that lie
    /// in its plane, in sketch coordinates (the `plane_geometry` query): an
    /// edge whose every point is in the plane within the model's linear
    /// tolerance (lines, arcs, circles, ellipses, splines), and every vertex
    /// in it. Bodies of other components count where an occurrence shows
    /// them, as `sketch.project` with `occurrence` takes them (mitcad#100),
    /// when the design policy lets the sketch reference them.
    pub fn plane_geometry(&self, sketch: FeatureUid) -> Result<Vec<PlaneItem>, ModelError> {
        let position = self.state.require(sketch)?;
        let frame = match (
            self.sketch_output(sketch),
            &self.state.features[position].def,
        ) {
            (Some(output), _) => output.frame,
            (None, FeatureDef::Sketch(def)) => def
                .plane
                .origin_frame()
                .map(|plane| def.place(&plane))
                .ok_or_else(|| invalid("the sketch has no plane yet (it does not evaluate)"))?,
            (None, _) => return Err(invalid(format!("{sketch} is not a sketch"))),
        };
        let component = self.state.features[position].component;
        let bodies = self.bodies_before(position);
        let assembly = &self.state.assembly;
        let placements = &self.result.placements;
        let mut placement = |o: OccurrenceUid| {
            placements.get(&o).copied().unwrap_or_else(|| {
                assembly
                    .occurrence(o)
                    .map_or(Transform::IDENTITY, |o| o.transform)
            })
        };
        let own_policy = self.policy(component);
        // The visible bodies as the design places them: the sketch's own
        // component's once, in its own coordinates; another component's at
        // each occurrence that shows it, moved into the sketch's.
        let mut seen: Vec<Seen<'_, K::Shape>> = Vec::new();
        let mut own: BTreeSet<BodyUid> = BTreeSet::new();
        for instance in self.instances() {
            if !instance.visible {
                continue;
            }
            let Some(shape) = bodies.get(&instance.body) else {
                continue;
            };
            if instance.component == component {
                if own.insert(instance.body) {
                    seen.push(Seen {
                        body: instance.body,
                        shape,
                        occurrence: None,
                        to_sketch: None,
                    });
                }
                continue;
            }
            if !own_policy.reference_others
                || !self.policy(instance.component).references_from_others
            {
                continue;
            }
            let Ok(Some(link)) = crate::links::link_for(assembly, component, &instance.path, None)
            else {
                continue;
            };
            let Ok(to_sketch) = link.transform(assembly, component, &mut placement) else {
                continue;
            };
            seen.push(Seen {
                body: instance.body,
                shape,
                occurrence: Some(instance.path),
                to_sketch: Some(to_sketch),
            });
        }
        let normal = unit(cross(frame.x_axis, frame.y_axis));
        let mut items = Vec::new();
        for body in &seen {
            self.plane_items_of(body, &frame, normal, &mut items);
        }
        Ok(items)
    }

    /// The edges and vertices of one body in the plane.
    fn plane_items_of(
        &self,
        seen: &Seen<'_, K::Shape>,
        frame: &SketchFrame,
        normal: [f64; 3],
        items: &mut Vec<PlaneItem>,
    ) {
        let kernel = &self.kernel;
        let moved = |curve: Curve3| match &seen.to_sketch {
            Some(t) => transform_curve(t, &curve),
            None => curve,
        };
        // The tolerance grows with the body's size; a body whose box does
        // not reach the plane has nothing in it.
        let mut size = 1.0_f64;
        if let Ok(Some(b)) = kernel.bounding_box(seen.shape) {
            let mut low = f64::INFINITY;
            let mut high = f64::NEG_INFINITY;
            for i in 0..8 {
                let corner = [
                    if i & 1 == 0 { b.min[0] } else { b.max[0] },
                    if i & 2 == 0 { b.min[1] } else { b.max[1] },
                    if i & 4 == 0 { b.min[2] } else { b.max[2] },
                ];
                let p = match moved(Curve3::Point(corner)) {
                    Curve3::Point(p) => p,
                    _ => corner,
                };
                let off = dot(std::array::from_fn(|k| p[k] - frame.origin[k]), normal);
                low = low.min(off);
                high = high.max(off);
            }
            let diagonal: f64 = (0..3)
                .map(|k| (b.max[k] - b.min[k]).powi(2))
                .sum::<f64>()
                .sqrt();
            size = size.max(diagonal);
            let tolerance = crate::datum::LINEAR * size;
            if low > tolerance || high < -tolerance {
                return;
            }
        }
        let tolerance = crate::datum::LINEAR * size;
        let item = |source: TopoName, shape: PlaneShape| PlaneItem {
            source,
            body: seen.body,
            occurrence: seen.occurrence.clone(),
            shape,
        };
        // Edges whose every point is in the plane.
        for edge in kernel.edges(seen.shape).unwrap_or_default() {
            let Some(Ok(source @ TopoName::Edge(_))) = edge.name.map(|n| n.parse::<TopoName>())
            else {
                continue;
            };
            let Ok(curves) = kernel.curves_of(seen.shape, &source) else {
                continue;
            };
            let [curve] = &curves[..] else {
                continue;
            };
            let curve = moved(curve.clone());
            if distance_from_plane(&curve, frame.origin, normal) > tolerance {
                continue;
            }
            if let Some(Projected::Curve(c)) = project(std::slice::from_ref(&curve), frame)
                .into_iter()
                .next()
            {
                items.push(item(source, PlaneShape::Curve(c)));
            }
        }
        // Vertices in the plane.
        for index in 0.. {
            let Ok(Some(element)) = kernel.indexed_element(seen.shape, ElementKind::Vertex, index)
            else {
                break;
            };
            let Some(Ok(source @ TopoName::Vertex(_))) =
                element.name.map(|n| n.parse::<TopoName>())
            else {
                continue;
            };
            let [Curve3::Point(at)] = &element.curves[..] else {
                continue;
            };
            let point = moved(Curve3::Point(*at));
            if distance_from_plane(&point, frame.origin, normal) > tolerance {
                continue;
            }
            if let Some(Projected::Point(p)) = project(std::slice::from_ref(&point), frame)
                .into_iter()
                .next()
            {
                items.push(item(source, PlaneShape::Vertex(p)));
            }
        }
    }
}
