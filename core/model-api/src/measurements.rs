// SPDX-License-Identifier: MIT
//! Measurement objects (mitcad#649), now annotations (mitcad#702,
//! `annotations.rs`, `document/annotations.rs`): the commands that add,
//! rename, recolour, show or hide, change and delete them, with
//! `add_annotation`, `edit_annotation` and `delete_annotation` beside them,
//! and how each pair of picks is measured at the timeline marker, as the
//! `measure` query measures them (mitcad#646, mitcad#647). See
//! `commands.md`, "Measurement objects" and "Annotations".
//!
//! The geometry of a dimension is worked out here, in the design's
//! coordinates: the two ends of a length (the closest points, the offset
//! of two parallel planes along their normal, the axes' common
//! perpendicular, two circles' centres), a circle's centre, normal and
//! radius, and the corner and the two rays of an angle's sector.
//! `annotations.rs` lays the dimension out in its plane from them.

use mitcad_vec3::{add, add_scaled, cross, distance, dot, norm, scale, sub, unit};
use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use super::analysis::{MeasuredPicks, element_plane, plane_offset};
use super::annotations::{AnnotationEdit, NewAnnotation};
use super::dispatch::named_enum;
use super::measure_axes::{self, Element};
use super::prelude::*;
use mitcad_model::analysis::Separation;
use mitcad_model::datum::Vec3;
use mitcad_model::kernel::Kernel;
use mitcad_model::{Document, MeasureKind, MeasurePick, NewMeasurement};

/// Two directions closer than this (radians) are parallel: they have no
/// corner between them (the assembly constraints' tolerance, as Measure's).
const PARALLEL: f64 = mitcad_model::internal::constraints::ANGULAR;

/// Lengths up to this (mm) are none when the default kind is chosen: the
/// baseline tolerance of geometric comparisons.
const NOTHING: f64 = 1e-6;

named_enum! {
    #[derive(Debug, Deserialize)]
    #[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
    pub(super) enum MeasurementCommand {
        "add_measurement" => AddMeasurement {
            // (Boxed: a pick is large beside the other commands' fields.)
            a: Box<MeasurePick>,
            #[serde(default)]
            b: Option<Box<MeasurePick>>,
            /// What the picks give first when left out (`default_kind`).
            #[serde(default)]
            kind: Option<MeasureKind>,
            /// The next free `<Kind><n>` when left out.
            #[serde(default)]
            name: Option<String>,
            /// `#rrggbb` or `auto`; `auto` when left out (mitcad#894).
            #[serde(default)]
            color: Option<String>,
        },
        "rename_measurement" => RenameMeasurement {
            name: String,
            new_name: String,
        },
        "set_measurement_color" => SetMeasurementColor {
            name: String,
            color: String,
        },
        "set_measurement_visible" => SetMeasurementVisible {
            name: String,
            visible: bool,
        },
        "set_measurement_kind" => SetMeasurementKind {
            name: String,
            kind: MeasureKind,
        },
        "delete_measurement" => DeleteMeasurement {
            name: String,
        },
        // Annotations (mitcad#702, `annotations.rs`): the measurement
        // commands above are their short forms.
        "add_annotation" => AddAnnotation(Box<NewAnnotation>),
        "edit_annotation" => EditAnnotation(Box<AnnotationEdit>),
        "delete_annotation" => DeleteAnnotation {
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            names: Option<Vec<String>>,
        },
        // The height of the annotations' texts (mitcad#894): mm, or null
        // for the automatic one.
        "set_annotation_text_height" => SetAnnotationTextHeight {
            text_height: Option<f64>,
        },
    }
}

/// A value the picks have, and where its dimension is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Found {
    pub(super) kind: MeasureKind,
    pub(super) value: f64,
    pub(super) drawn: Drawn,
}

/// Where a dimension is drawn, in the design's coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Drawn {
    /// A length between two points.
    Ends([Vec3; 2]),
    /// A circle's radius (from the centre to the rim) or diameter (across
    /// it): its centre, normal and radius, and the ends of the line drawn.
    Circle {
        center: Vec3,
        axis: Vec3,
        radius: f64,
        ends: [Vec3; 2],
    },
    /// An angle: the corner where its rays meet and the unit rays from it
    /// that bound the sector measured (the angle between them is the
    /// value), and half the distance from the corner to the nearer pick's
    /// point (a plane's, an edge's middle, an axis'), the arc's radius
    /// unless it is set.
    Arc {
        center: Vec3,
        from: Vec3,
        to: Vec3,
        reach: Option<f64>,
    },
}

/// The document's methods of this module used across the api, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(super) trait MeasurementsOps<K: Kernel> {
    fn run_measurement_command(&mut self, command: MeasurementCommand) -> Result<Value, ApiError>;
}

impl<K: Kernel> MeasurementsOps<K> for Document<K> {
    fn run_measurement_command(&mut self, command: MeasurementCommand) -> Result<Value, ApiError> {
        Ok(match command {
            MeasurementCommand::AddMeasurement {
                a,
                b,
                kind,
                name,
                color,
            } => {
                let picks = self
                    .measure_picks(&a, b.as_deref())
                    .map_err(|e| ApiError(format!("the measurement's picks: {e}")))?;
                let found = values_of(&picks);
                let kind = match kind {
                    Some(kind) => kind,
                    None => default_kind(&picks, &found).ok_or_else(no_single_value)?,
                };
                check_kind(kind, &found)?;
                let name = self.add_measurement(NewMeasurement {
                    kind,
                    a: *a,
                    b: b.map(|b| *b),
                    name,
                    color,
                })?;
                json!({"name": name, "kind": kind})
            }
            MeasurementCommand::RenameMeasurement { name, new_name } => {
                self.rename_measurement(&name, &new_name)?;
                json!({})
            }
            MeasurementCommand::SetMeasurementColor { name, color } => {
                self.set_measurement_color(&name, &color)?;
                json!({})
            }
            MeasurementCommand::SetMeasurementVisible { name, visible } => {
                self.set_measurement_visible(&name, visible)?;
                json!({})
            }
            MeasurementCommand::SetMeasurementKind { name, kind } => {
                let measurement = self.measurement(&name)?;
                if kind != measurement.kind && (kind.is_leader() || measurement.kind.is_leader()) {
                    return Err(ApiError(format!(
                        "{name}: a {0} stays a {0}, a dimension a dimension",
                        measurement.kind.key()
                    )));
                }
                if kind.is_leader() {
                    return Ok(json!({})); // its own kind: nothing to change
                }
                let picks = self
                    .measure_picks(&measurement.a, measurement.b.as_ref())
                    .map_err(|e| ApiError(format!("{name}: {e}")))?;
                check_kind(kind, &values_of(&picks))
                    .map_err(|e| ApiError(format!("{name}: {e}")))?;
                self.set_measurement_kind(&name, kind)?;
                json!({})
            }
            MeasurementCommand::DeleteMeasurement { name } => {
                self.delete_measurement(&name)?;
                json!({})
            }
            MeasurementCommand::AddAnnotation(new) => self.add_annotation_command(*new)?,
            MeasurementCommand::EditAnnotation(edit) => self.edit_annotation_command(*edit)?,
            MeasurementCommand::DeleteAnnotation { name, names } => {
                let names = super::annotations::names_of(name, names)?;
                self.delete_annotations(&names)?;
                json!({})
            }
            MeasurementCommand::SetAnnotationTextHeight { text_height } => {
                self.set_annotation_text_height(text_height)?;
                json!({})
            }
        })
    }
}

/// Why one pick without a circle has nothing to keep.
pub(super) fn no_single_value() -> ApiError {
    ApiError(
        "one pick keeps its radius or diameter: pick a round edge or face, or pick two".to_owned(),
    )
}

/// Why the picks cannot keep a measurement of this kind, if they cannot.
pub(super) fn check_kind(kind: MeasureKind, found: &[Found]) -> Result<(), ApiError> {
    if found.iter().any(|f| f.kind == kind) {
        return Ok(());
    }
    let have: Vec<&str> = found.iter().map(|f| f.kind.key()).collect();
    Err(ApiError(if have.is_empty() {
        format!(
            "the picks have no {}: one pick keeps the radius or diameter of a round edge or face",
            kind.key().replace('_', " ")
        )
    } else {
        format!(
            "the picks have no {}; they have {}",
            kind.key().replace('_', " "),
            have.join(", ")
        )
    }))
}

/// What a measurement of these picks shows when its kind is not given: a
/// round pick's diameter (an arc's radius); of two picks the offset of
/// parallel planes, else the distance of axes or of circles' centres, else
/// their closest distance, else (touching) the angle between them.
pub(super) fn default_kind(picks: &MeasuredPicks, found: &[Found]) -> Option<MeasureKind> {
    if let Some(circle) = picks.single.as_ref().and_then(|m| m.circle) {
        let full = circle.sweep == 0.0 || circle.sweep >= 2.0 * std::f64::consts::PI - 1e-9;
        return Some(if full {
            MeasureKind::Diameter
        } else {
            MeasureKind::Radius
        });
    }
    let value = |kind: MeasureKind| found.iter().find(|f| f.kind == kind).map(|f| f.value);
    // Two picks always have a distance; one without a circle has nothing.
    let distance = value(MeasureKind::Distance)?;
    if value(MeasureKind::Offset).is_some() {
        return Some(MeasureKind::Offset);
    }
    for kind in [MeasureKind::AxisDistance, MeasureKind::CenterDistance] {
        if value(kind).is_some_and(|v| v > NOTHING) {
            return Some(kind);
        }
    }
    Some(
        if distance <= NOTHING && value(MeasureKind::Angle).is_some() {
            MeasureKind::Angle
        } else {
            MeasureKind::Distance
        },
    )
}

/// The values the picks have, each with where it is drawn: a round pick's
/// radius and diameter; of two picks their closest distance, the angle
/// between them, the offset of two parallel planes, the distance of axes
/// and of circles' centres, where they apply.
pub(super) fn values_of(picks: &MeasuredPicks) -> Vec<Found> {
    let mut found = Vec::new();
    if let Some(circle) = picks.single.as_ref().and_then(|m| m.circle) {
        let (center, r) = (circle.center, circle.radius);
        let across = across(circle.axis);
        let rim = add_scaled(center, across, r);
        let circle_drawn = |ends: [Vec3; 2]| Drawn::Circle {
            center,
            axis: unit(circle.axis).unwrap_or([0.0, 0.0, 0.0]),
            radius: r,
            ends,
        };
        found.push(Found {
            kind: MeasureKind::Radius,
            value: r,
            drawn: circle_drawn([center, rim]),
        });
        found.push(Found {
            kind: MeasureKind::Diameter,
            value: 2.0 * r,
            drawn: circle_drawn([add_scaled(center, across, -r), rim]),
        });
    }
    let Some(between) = &picks.between else {
        return found;
    };
    found.push(Found {
        kind: MeasureKind::Distance,
        value: between.distance,
        drawn: Drawn::Ends([between.on_a, between.on_b]),
    });
    let [ea, eb] = &picks.elements;
    let elements = ea.as_ref().zip(eb.as_ref());
    let axes = elements.map(|(ea, eb)| measure_axes::between(ea, eb, PARALLEL));
    // The kernel's angle where it has one, else the axes' (as the measure
    // query gives it).
    if let Some(angle) = between.angle.or_else(|| axes.and_then(|x| x.angle)) {
        let (center, from, to) = match elements {
            Some((ea, eb)) => angle_drawing(ea, eb, between, angle),
            None => (middle(between), [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        };
        found.push(Found {
            kind: MeasureKind::Angle,
            value: angle,
            drawn: Drawn::Arc {
                center,
                from,
                to,
                reach: elements.and_then(|(ea, eb)| reach(center, ea, eb)),
            },
        });
    }
    // Two parallel planes: along the normal from the closest point on the
    // first to its foot on the second.
    if let (Some(pa), Some(pb)) = (
        ea.as_ref().and_then(element_plane),
        eb.as_ref().and_then(element_plane),
    ) && let Some(offset) = plane_offset(&pa, &pb)
        && let Some(n) = unit(pb.normal)
    {
        let p = between.on_a;
        let foot = add_scaled(p, n, -dot(sub(p, pb.origin), n));
        found.push(Found {
            kind: MeasureKind::Offset,
            value: offset,
            drawn: Drawn::Ends([p, foot]),
        });
    }
    if let Some(axes) = axes {
        if let (Some(d), Some(points)) = (axes.axis_distance, axes.axis_points) {
            found.push(Found {
                kind: MeasureKind::AxisDistance,
                value: d,
                drawn: Drawn::Ends(points),
            });
        }
        if let (
            Some(d),
            Some((
                Element::Axis {
                    origin: ca,
                    center: true,
                    ..
                },
                Element::Axis {
                    origin: cb,
                    center: true,
                    ..
                },
            )),
        ) = (axes.center_distance, elements)
        {
            found.push(Found {
                kind: MeasureKind::CenterDistance,
                value: d,
                drawn: Drawn::Ends([*ca, *cb]),
            });
        }
    }
    found
}

/// The middle of the closest points.
fn middle(between: &Separation) -> Vec3 {
    scale(add(between.on_a, between.on_b), 0.5)
}

/// Half the distance from an angle's corner to the nearer of the picks'
/// points (a plane's point, a straight edge's middle, an axis' point, a
/// point), where one is away from the corner: an arc's radius that suits
/// the picks' size.
fn reach(center: Vec3, ea: &Element, eb: &Element) -> Option<f64> {
    let point = |element: &Element| match *element {
        Element::Plane { origin, .. } => Some(origin),
        Element::Point(point) => Some(point),
        _ => line_point(element),
    };
    [ea, eb]
        .into_iter()
        .filter_map(point)
        .map(|p| distance(p, center))
        .filter(|d| *d > NOTHING)
        .min_by(f64::total_cmp)
        .map(|d| d / 2.0)
}

/// A unit vector square to `axis` (along the coordinate axis it is least
/// along, turned square), for a circle's radius drawn across it.
pub(super) fn across(axis: Vec3) -> Vec3 {
    let n = unit(axis).unwrap_or([0.0, 0.0, 1.0]);
    let e = if n[0].abs() <= n[1].abs() && n[0].abs() <= n[2].abs() {
        [1.0, 0.0, 0.0]
    } else if n[1].abs() <= n[2].abs() {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    unit(cross(cross(n, e), n)).unwrap_or([1.0, 0.0, 0.0])
}

/// The angle between two unit vectors, 0 to π.
fn angle_of(u: Vec3, v: Vec3) -> f64 {
    dot(u, v).clamp(-1.0, 1.0).acos()
}

/// A straight edge's or an axis' line: a point on it and its unit
/// direction.
fn line_of(element: &Element) -> Option<(Vec3, Vec3)> {
    match *element {
        Element::Line { start, end } => Some((start, unit(sub(end, start))?)),
        Element::Axis {
            origin, direction, ..
        } => Some((origin, unit(direction)?)),
        _ => None,
    }
}

/// A plane's point and unit normal.
fn plane_of(element: &Element) -> Option<(Vec3, Vec3)> {
    match *element {
        Element::Plane { origin, normal } => Some((origin, unit(normal)?)),
        _ => None,
    }
}

/// Where a line's ray from a corner points: towards the middle of a
/// straight edge, or an axis' point.
fn line_point(element: &Element) -> Option<Vec3> {
    match *element {
        Element::Line { start, end } => Some(scale(add(start, end), 0.5)),
        Element::Axis { origin, .. } => Some(origin),
        _ => None,
    }
}

/// `direction`, turned round when `point` lies behind `center` along it.
fn towards(direction: Vec3, point: Option<Vec3>, center: Vec3) -> Vec3 {
    match point {
        Some(p) if dot(sub(p, center), direction) < 0.0 => scale(direction, -1.0),
        _ => direction,
    }
}

/// The corner of an angle between two picks and the unit rays of its
/// sector, the first pick's first: where two lines come closest, where two
/// planes meet nearest the closest points (the rays in the planes, square
/// to where they meet), where a line meets a plane (the line and its
/// shadow in the plane). The second ray is turned round where that makes
/// the angle between the rays the value. Parallel picks have the middle of
/// the closest points and two equal rays.
fn angle_drawing(
    ea: &Element,
    eb: &Element,
    between: &Separation,
    value: f64,
) -> (Vec3, Vec3, Vec3) {
    let middle = middle(between);
    let found = match (line_of(ea), line_of(eb), plane_of(ea), plane_of(eb)) {
        (Some(la), Some(lb), _, _) => lines_corner(la, lb, line_point(ea), line_point(eb)),
        (_, _, Some(pa), Some(pb)) => planes_corner(pa, pb, middle),
        (Some(l), None, None, Some(p)) => line_plane_corner(l, line_point(ea), p, false),
        (None, Some(l), Some(p), None) => line_plane_corner(l, line_point(eb), p, true),
        _ => None,
    };
    let (center, from, mut to) = found.unwrap_or((middle, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    let turned = scale(to, -1.0);
    if (angle_of(from, turned) - value).abs() < (angle_of(from, to) - value).abs() {
        to = turned;
    }
    (center, from, to)
}

/// Two lines: the middle of their common perpendicular, each ray along its
/// line towards its edge's middle or axis point; none when parallel.
fn lines_corner(
    (pa, da): (Vec3, Vec3),
    (pb, db): (Vec3, Vec3),
    point_a: Option<Vec3>,
    point_b: Option<Vec3>,
) -> Option<(Vec3, Vec3, Vec3)> {
    if norm(cross(da, db)) <= PARALLEL.sin() {
        return None;
    }
    // The closest points pa + s da and pb + t db.
    let w = sub(pa, pb);
    let b = dot(da, db);
    let (d, e) = (dot(da, w), dot(db, w));
    let den = 1.0 - b * b;
    let s = (b * e - d) / den;
    let t = (e - b * d) / den;
    let center = scale(add(add_scaled(pa, da, s), add_scaled(pb, db, t)), 0.5);
    Some((
        center,
        towards(da, point_a, center),
        towards(db, point_b, center),
    ))
}

/// Two planes: the point of the line where they meet nearest `near`, and in
/// each plane the ray square to that line away from the other plane's
/// outer side; none when parallel.
fn planes_corner(
    (oa, na): (Vec3, Vec3),
    (ob, nb): (Vec3, Vec3),
    near: Vec3,
) -> Option<(Vec3, Vec3, Vec3)> {
    if norm(cross(na, nb)) <= PARALLEL.sin() {
        return None;
    }
    let k = dot(na, nb);
    let det = 1.0 - k * k;
    let ca = dot(na, sub(oa, near));
    let cb = dot(nb, sub(ob, near));
    let alpha = (ca - k * cb) / det;
    let beta = (cb - k * ca) / det;
    let center = add_scaled(add_scaled(near, na, alpha), nb, beta);
    let ray_a = unit(add_scaled(scale(nb, -1.0), na, k))?;
    let ray_b = unit(add_scaled(scale(na, -1.0), nb, k))?;
    Some((center, ray_a, ray_b))
}

/// A line and a plane: where the line meets the plane, the line's ray
/// towards its edge's middle or axis point and its shadow in the plane
/// (`line_second`: the line is the second pick); none when parallel.
fn line_plane_corner(
    (p, d): (Vec3, Vec3),
    point: Option<Vec3>,
    (o, n): (Vec3, Vec3),
    line_second: bool,
) -> Option<(Vec3, Vec3, Vec3)> {
    let dn = dot(d, n);
    if dn.abs() <= PARALLEL.sin() {
        return None;
    }
    let center = add_scaled(p, d, dot(n, sub(o, p)) / dn);
    let ray_line = towards(d, point, center);
    let ray_plane = unit(add_scaled(ray_line, n, -dot(ray_line, n))).unwrap_or_else(|| across(n));
    Some(if line_second {
        (center, ray_plane, ray_line)
    } else {
        (center, ray_line, ray_plane)
    })
}

/// Where the dimension is drawn, in the query's answer: `points` (the two
/// ends of a length, a radius from the centre to the rim, a diameter across
/// the circle), a circle's `center`, `axis` and `radius`, an angle's `arc`
/// {`center`, `from`, `to`}.
pub(super) fn add_drawn(value: &mut Value, drawn: &Drawn) {
    // + 0.0 turns -0.0 into 0.0.
    let v = |v: Vec3| v.map(|x| x + 0.0);
    match *drawn {
        Drawn::Ends([p, q]) => value["points"] = json!([v(p), v(q)]),
        Drawn::Circle {
            center,
            axis,
            radius,
            ends: [p, q],
        } => {
            value["points"] = json!([v(p), v(q)]);
            value["center"] = json!(v(center));
            value["axis"] = json!(v(axis));
            value["radius"] = json!(radius);
        }
        Drawn::Arc {
            center, from, to, ..
        } => {
            value["arc"] = json!({"center": v(center), "from": v(from), "to": v(to)});
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, PI};

    use super::*;

    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} instead of {b}");
    }

    fn near_point(a: Vec3, b: Vec3) {
        for (x, y) in a.into_iter().zip(b) {
            near(x, y);
        }
    }

    fn separation(on_a: Vec3, on_b: Vec3, angle: Option<f64>) -> Separation {
        Separation {
            distance: mitcad_vec3::distance(on_a, on_b),
            on_a,
            on_b,
            inside: false,
            angle,
        }
    }

    #[test]
    fn two_planes_meet_at_their_edge_with_the_angle_between_their_rays() {
        // A block's top (+Z at z = 20) and its front (-Y at y = 0): they
        // meet along X at y = 0, z = 20; 90 degrees.
        let top = Element::Plane {
            origin: [10.0, 10.0, 20.0],
            normal: [0.0, 0.0, 1.0],
        };
        let front = Element::Plane {
            origin: [10.0, 0.0, 10.0],
            normal: [0.0, -1.0, 0.0],
        };
        let touching = separation([5.0, 0.0, 20.0], [5.0, 0.0, 20.0], Some(FRAC_PI_2));
        let (center, from, to) = angle_drawing(&top, &front, &touching, FRAC_PI_2);
        near_point(center, [5.0, 0.0, 20.0]);
        near_point(from, [0.0, 1.0, 0.0]);
        near_point(to, [0.0, 0.0, -1.0]);
        // A chamfer 45 degrees to the top: the rays 45 degrees apart, the
        // second along the chamfer's plane beyond the edge.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let chamfer = Element::Plane {
            origin: [10.0, 0.0, 20.0],
            normal: [0.0, -s, s],
        };
        let (_, from, to) = angle_drawing(&top, &chamfer, &touching, FRAC_PI_4);
        near(angle_of(from, to), FRAC_PI_4);
    }

    #[test]
    fn lines_and_planes_meet_where_they_cross() {
        // Two edges crossing at (10, 0, 0) at 30 degrees.
        let x = Element::Line {
            start: [0.0, 0.0, 0.0],
            end: [10.0, 0.0, 0.0],
        };
        let (c, s) = (FRAC_PI_6.cos(), FRAC_PI_6.sin());
        let slanted = Element::Line {
            start: [10.0, 0.0, 0.0],
            end: [10.0 + 10.0 * c, 10.0 * s, 0.0],
        };
        let at = separation([10.0, 0.0, 0.0], [10.0, 0.0, 0.0], Some(FRAC_PI_6));
        let (center, from, to) = angle_drawing(&x, &slanted, &at, FRAC_PI_6);
        near_point(center, [10.0, 0.0, 0.0]);
        near(angle_of(from, to), FRAC_PI_6);
        // An upright axis through (3, 4) against the XY plane: 90 degrees at
        // (3, 4, 0).
        let axis = Element::Axis {
            origin: [3.0, 4.0, 15.0],
            direction: [0.0, 0.0, 1.0],
            center: false,
        };
        let ground = Element::Plane {
            origin: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        };
        let below = separation([3.0, 4.0, 0.0], [3.0, 4.0, 0.0], Some(FRAC_PI_2));
        let (center, from, to) = angle_drawing(&ground, &axis, &below, FRAC_PI_2);
        near_point(center, [3.0, 4.0, 0.0]);
        near_point(to, [0.0, 0.0, 1.0]);
        near(dot(from, [0.0, 0.0, 1.0]), 0.0);
        near(angle_of(from, to), FRAC_PI_2);
    }

    #[test]
    fn parallel_picks_have_no_corner() {
        let a = Element::Plane {
            origin: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        };
        let b = Element::Plane {
            origin: [0.0, 0.0, 5.0],
            normal: [0.0, 0.0, -1.0],
        };
        let apart = separation([1.0, 1.0, 0.0], [1.0, 1.0, 5.0], Some(PI));
        let (center, from, to) = angle_drawing(&a, &b, &apart, PI);
        near_point(center, [1.0, 1.0, 2.5]);
        near(angle_of(from, to), PI);
    }

    #[test]
    fn across_is_square_to_the_axis() {
        for axis in [
            [0.0, 0.0, 1.0],
            [1.0, 2.0, 3.0],
            [0.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
        ] {
            let a = across(axis);
            near(norm(a), 1.0);
            if let Some(n) = unit(axis) {
                near(dot(a, n), 0.0);
            }
        }
    }
}
