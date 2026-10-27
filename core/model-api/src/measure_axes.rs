// SPDX-License-Identifier: MIT
//! What Measure finds between picks with an axis, a direction or a normal
//! (mitcad#647): the angle between any two of them, and the distances of
//! axes from axes, lines, points and planes, worked out here on the
//! geometry the kernel describes (`Kernel::face_geometry`,
//! `Kernel::edge_geometry`), in the design's coordinates.

use mitcad_vec3::{add_scaled, cross, distance, dot, norm, sub, unit};

use mitcad_model::datum::Vec3;

/// The geometry of a pick that Measure works with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Element {
    /// A planar face or a datum plane: a point of it and its normal (a
    /// face's out of the material).
    Plane { origin: Vec3, normal: Vec3 },
    /// A straight edge from `start` to `end`.
    Line { start: Vec3, end: Vec3 },
    /// The axis of a cylindrical, conical or toroidal face, of a circle or
    /// an arc (`center`: `origin` is its centre), or a construction axis:
    /// a point of it and its unit direction.
    Axis {
        origin: Vec3,
        direction: Vec3,
        center: bool,
    },
    /// A vertex, a construction point or a sphere's centre.
    Point(Vec3),
}

/// What lies between two elements; the fields that apply are set.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Between {
    /// Between two directions or normals, radians (see [`angle`]).
    pub angle: Option<f64>,
    /// With an axis on one side at least: the distance of the axis from
    /// the other side's axis, line, point or parallel plane.
    pub axis_distance: Option<f64>,
    /// Where `axis_distance` is measured: on the first element, then on
    /// the second.
    pub axis_points: Option<[Vec3; 2]>,
    /// Between the centres of two circles or arcs.
    pub center_distance: Option<f64>,
}

impl Element {
    /// The axis of a curved face, a circle or a construction axis, as the
    /// query reports it: its point and unit direction.
    pub(super) fn axis(&self) -> Option<(Vec3, Vec3)> {
        match self {
            Self::Axis {
                origin, direction, ..
            } => Some((*origin, *direction)),
            _ => None,
        }
    }

    /// A line that the element lies along: an axis or a straight edge.
    fn line(&self) -> Option<(Vec3, Vec3)> {
        match self {
            Self::Axis {
                origin, direction, ..
            } => Some((*origin, *direction)),
            Self::Line { start, end } => Some((*start, unit(sub(*end, *start))?)),
            _ => None,
        }
    }

    /// The unit normal of a plane.
    fn normal(&self) -> Option<Vec3> {
        match self {
            Self::Plane { normal, .. } => unit(*normal),
            _ => None,
        }
    }
}

/// The angle between two elements, radians: two normals 0 to π (as the
/// kernel measures planar faces); two lines (axes or straight edges) 0 to
/// π/2; a line against a plane 0 to π/2 from the plane (as a straight
/// edge against a planar face). None when either has no direction.
pub(super) fn angle(a: &Element, b: &Element) -> Option<f64> {
    let full = |u: Vec3, v: Vec3| dot(u, v).clamp(-1.0, 1.0).acos();
    let acute = |u: Vec3, v: Vec3| dot(u, v).abs().min(1.0).acos();
    // + 0.0 turns -0.0 into 0.0.
    let value = match (a.normal(), b.normal(), a.line(), b.line()) {
        (Some(na), Some(nb), _, _) => full(na, nb),
        (_, _, Some((_, da)), Some((_, db))) => acute(da, db),
        (Some(n), _, _, Some((_, d))) | (_, Some(n), Some((_, d)), _) => {
            dot(n, d).abs().min(1.0).asin()
        }
        _ => return None,
    };
    Some(value + 0.0)
}

/// The angle and the axis distances of two elements; `parallel` is the
/// largest angle (radians) at which two directions count as parallel.
pub(super) fn between(a: &Element, b: &Element, parallel: f64) -> Between {
    let mut result = Between {
        angle: angle(a, b),
        ..Between::default()
    };
    if let (
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
    ) = (a, b)
    {
        result.center_distance = Some(distance(*ca, *cb) + 0.0);
    }
    // Axis distances need an axis on one side at least: two straight edges,
    // planes or points have their closest distance only.
    let points = match (a, b) {
        (Element::Axis { .. }, _) => axis_points(a, b, parallel),
        (_, Element::Axis { .. }) => axis_points(b, a, parallel).map(|[p, q]| [q, p]),
        _ => None,
    };
    if let Some(points) = points {
        result.axis_distance = Some(distance(points[0], points[1]) + 0.0);
        result.axis_points = Some(points.map(|p| p.map(|x| x + 0.0)));
    }
    result
}

/// Where the axis `axis` comes nearest to `other`: the point on the axis,
/// then the one on `other`. For another axis or a straight edge's line:
/// the ends of the common perpendicular of skew lines, the point where
/// they meet, or for parallel lines the axis's point and its foot on the
/// other; for a point, its foot on the axis; for a plane parallel to the
/// axis, the axis's point and its foot on the plane (None when the plane
/// is not parallel: its angle tells).
fn axis_points(axis: &Element, other: &Element, parallel: f64) -> Option<[Vec3; 2]> {
    let (o, d) = axis.line()?;
    let foot = |p: Vec3, origin: Vec3, direction: Vec3| {
        add_scaled(origin, direction, dot(sub(p, origin), direction))
    };
    match other {
        Element::Point(p) => Some([foot(*p, o, d), *p]),
        Element::Plane { origin, .. } => {
            let n = other.normal()?;
            if dot(n, d).abs() > parallel.sin() {
                return None;
            }
            Some([o, add_scaled(o, n, -dot(sub(o, *origin), n))])
        }
        Element::Axis { .. } | Element::Line { .. } => {
            let (p, e) = other.line()?;
            let c = cross(d, e);
            if norm(c) <= parallel.sin() {
                return Some([o, foot(o, p, e)]);
            }
            // The common perpendicular: o + s d and p + t e with the
            // segment between them square to both lines.
            let w = sub(o, p);
            let b = dot(d, e);
            let (dw, ew) = (dot(d, w), dot(e, w));
            let denominator = 1.0 - b * b;
            let s = (b * ew - dw) / denominator;
            let t = (ew - b * dw) / denominator;
            Some([add_scaled(o, d, s), add_scaled(p, e, t)])
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, PI};

    use super::*;

    const PARALLEL: f64 = 1e-6;

    fn axis(origin: Vec3, direction: Vec3) -> Element {
        Element::Axis {
            origin,
            direction: unit(direction).unwrap(),
            center: false,
        }
    }

    fn circle(center: Vec3, normal: Vec3) -> Element {
        Element::Axis {
            origin: center,
            direction: unit(normal).unwrap(),
            center: true,
        }
    }

    fn near(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("a value");
        assert!(
            (actual - expected).abs() < 1e-9,
            "{actual} instead of {expected}"
        );
    }

    fn near_points(actual: Option<[Vec3; 2]>, expected: [Vec3; 2]) {
        let actual = actual.expect("points");
        for (p, q) in actual.iter().zip(&expected) {
            assert!(
                distance(*p, *q) < 1e-9,
                "{actual:?} instead of {expected:?}"
            );
        }
    }

    #[test]
    fn parallel_axes_are_their_distance_apart() {
        let a = axis([0.0, 0.0, 5.0], [0.0, 0.0, 1.0]);
        let b = axis([30.0, 40.0, -7.0], [0.0, 0.0, -1.0]);
        let found = between(&a, &b, PARALLEL);
        near(found.angle, 0.0);
        near(found.axis_distance, 50.0);
        near_points(found.axis_points, [[0.0, 0.0, 5.0], [30.0, 40.0, 5.0]]);
        assert_eq!(found.center_distance, None);
    }

    #[test]
    fn skew_axes_meet_their_common_perpendicular() {
        // Along X at z = 0 and along Y through (5, 0, 12): the
        // perpendicular from (5, 0, 0) to (5, 0, 12).
        let a = axis([-3.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let b = axis([5.0, 9.0, 12.0], [0.0, 1.0, 0.0]);
        let found = between(&a, &b, PARALLEL);
        near(found.angle, FRAC_PI_2);
        near(found.axis_distance, 12.0);
        near_points(found.axis_points, [[5.0, 0.0, 0.0], [5.0, 0.0, 12.0]]);
        // Either way round.
        let back = between(&b, &a, PARALLEL);
        near(back.axis_distance, 12.0);
        near_points(back.axis_points, [[5.0, 0.0, 12.0], [5.0, 0.0, 0.0]]);
    }

    #[test]
    fn intersecting_axes_meet_at_a_point() {
        let a = axis([0.0, 0.0, 0.0], [1.0, 1.0, 0.0]);
        let b = axis([10.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let found = between(&a, &b, PARALLEL);
        near(found.angle, FRAC_PI_4);
        near(found.axis_distance, 0.0);
        near_points(found.axis_points, [[10.0, 10.0, 0.0], [10.0, 10.0, 0.0]]);
    }

    #[test]
    fn an_axis_against_a_straight_edge_a_point_and_planes() {
        let a = axis([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        // A straight edge at 30 degrees from the axis, in the plane x = 4.
        let edge = Element::Line {
            start: [4.0, 0.0, 0.0],
            end: [4.0, FRAC_PI_6.sin(), FRAC_PI_6.cos()],
        };
        let found = between(&a, &edge, PARALLEL);
        near(found.angle, FRAC_PI_6);
        near(found.axis_distance, 4.0);
        // A point: its distance from the axis and its foot.
        let point = between(&Element::Point([3.0, 4.0, 9.0]), &a, PARALLEL);
        assert_eq!(point.angle, None);
        near(point.axis_distance, 5.0);
        near_points(point.axis_points, [[3.0, 4.0, 9.0], [0.0, 0.0, 9.0]]);
        // A plane parallel to the axis: the offset.
        let wall = Element::Plane {
            origin: [7.0, 0.0, 0.0],
            normal: [-1.0, 0.0, 0.0],
        };
        let found = between(&a, &wall, PARALLEL);
        near(found.angle, 0.0);
        near(found.axis_distance, 7.0);
        near_points(found.axis_points, [[0.0, 0.0, 0.0], [7.0, 0.0, 0.0]]);
        // A plane across it: the angle against the plane, no distance.
        let tilted = Element::Plane {
            origin: [0.0, 0.0, 3.0],
            normal: [0.0, 1.0, 1.0],
        };
        let found = between(&tilted, &a, PARALLEL);
        near(found.angle, FRAC_PI_4);
        assert_eq!(found.axis_distance, None);
        assert_eq!(found.axis_points, None);
    }

    #[test]
    fn circles_report_their_centres_distance() {
        // Two coaxial circles 6 apart: the same axis, centres 6 apart.
        let a = circle([1.0, 2.0, 0.0], [0.0, 0.0, 1.0]);
        let b = circle([1.0, 2.0, 6.0], [0.0, 0.0, -1.0]);
        let found = between(&a, &b, PARALLEL);
        near(found.center_distance, 6.0);
        near(found.axis_distance, 0.0);
        near(found.angle, 0.0);
        // A circle against a construction axis at 90 degrees.
        let x = axis([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let found = between(&a, &x, PARALLEL);
        near(found.angle, FRAC_PI_2);
        near(found.axis_distance, 2.0);
        assert_eq!(found.center_distance, None);
    }

    #[test]
    fn angles_without_axes_follow_the_kernel() {
        let up = Element::Plane {
            origin: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
        };
        let down = Element::Plane {
            origin: [0.0, 0.0, 5.0],
            normal: [0.0, 0.0, -1.0],
        };
        near(angle(&up, &down), PI);
        let edge = Element::Line {
            start: [0.0; 3],
            end: [1.0, 0.0, 1.0],
        };
        near(angle(&up, &edge), FRAC_PI_4);
        let found = between(&up, &edge, PARALLEL);
        assert_eq!(found.axis_distance, None);
        assert_eq!(angle(&up, &Element::Point([0.0; 3])), None);
    }
}
