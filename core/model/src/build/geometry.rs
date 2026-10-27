// SPDX-License-Identifier: MIT
//! Vectors and frames of the BUILD rules: a frame is the rigid transform
//! from its own coordinates (x, y = z × x and z as the columns) to those it
//! is given in, as joints have them (`joints.rs`).

use crate::kernel::BoundingBox;
use crate::transform::Transform;

// The vector helpers live in `mitcad-vec3`; the build rules take them from
// here.
pub use mitcad_vec3::Vec3 as V3;
pub use mitcad_vec3::{add, cross, dot, norm, scale, sub, unit};

pub const X: V3 = [1.0, 0.0, 0.0];
pub const Y: V3 = [0.0, 1.0, 0.0];
pub const Z: V3 = [0.0, 0.0, 1.0];

/// A unit vector at right angles to the unit vector `v`.
pub fn perpendicular(v: V3) -> V3 {
    let other = if v[0].abs() < 0.9 { X } else { Y };
    unit(cross(v, other)).unwrap_or(Y)
}

/// The frame at `origin` with the x and z axes (x made square to z).
pub fn frame(origin: V3, x: V3, z: V3) -> Transform {
    let z = unit(z).unwrap_or(Z);
    let x = unit(sub(x, scale(z, dot(x, z)))).unwrap_or_else(|| perpendicular(z));
    crate::joints::frame(origin, x, z)
}

/// A frame's origin, x, y and z axes.
pub fn parts(t: &Transform) -> [V3; 4] {
    crate::joints::frame_parts(t)
}

pub fn z_axis(t: &Transform) -> V3 {
    parts(t)[3]
}

pub fn x_axis(t: &Transform) -> V3 {
    parts(t)[1]
}

pub fn inverse(t: &Transform) -> Transform {
    crate::assembly::inverse(t)
}

/// How far along the unit `direction` from `start` the ray meets the plane
/// through `point` across `normal`; None when it runs along the plane.
pub fn ray_plane(start: V3, direction: V3, point: V3, normal: V3) -> Option<f64> {
    let along = dot(direction, normal);
    (along.abs() > 1e-9).then(|| dot(sub(point, start), normal) / along)
}

/// The farthest a box (moved by `placement`) reaches along the unit
/// `direction` from `start`, and the nearest.
pub fn box_along(
    bounds: &BoundingBox,
    placement: &Transform,
    start: V3,
    direction: V3,
) -> (f64, f64) {
    let mut near = f64::INFINITY;
    let mut far = f64::NEG_INFINITY;
    for corner in bounds.corners() {
        let d = dot(sub(placement.apply_point(corner), start), direction);
        near = near.min(d);
        far = far.max(d);
    }
    (near, far)
}

/// A box moved by `placement`, as the box of its moved corners.
pub fn moved_box(bounds: &BoundingBox, placement: &Transform) -> BoundingBox {
    let corners: Vec<V3> = bounds.corners().map(|c| placement.apply_point(c)).collect();
    BoundingBox {
        min: std::array::from_fn(|i| corners.iter().map(|c| c[i]).fold(f64::INFINITY, f64::min)),
        max: std::array::from_fn(|i| {
            corners
                .iter()
                .map(|c| c[i])
                .fold(f64::NEG_INFINITY, f64::max)
        }),
    }
}

/// Whether a point lies in a box widened by `tolerance`.
pub fn in_box(point: V3, bounds: &BoundingBox, tolerance: f64) -> bool {
    (0..3).all(|i| point[i] >= bounds.min[i] - tolerance && point[i] <= bounds.max[i] + tolerance)
}

/// The angle between two unit vectors, radians.
pub fn angle_between(a: V3, b: V3) -> f64 {
    dot(a, b).clamp(-1.0, 1.0).acos()
}
