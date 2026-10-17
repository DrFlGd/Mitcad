// SPDX-License-Identifier: MIT
//! Decoded items moved into a component's coordinates by an affine map
//! (an element's placement, a mapped item's transform that may scale or
//! mirror), so that the features are built in the component with frames
//! of their own: the frames are orthonormal again, and what the map does to
//! a profile is done to the profile's curves (a circle under a non-uniform
//! scale becomes an ellipse).
//!
//! CSG primitives become the extrusions and revolutions that make them
//! first (a block is an extruded rectangle, a cylinder an extruded circle,
//! a cone and a sphere revolutions), so that they move like those; a
//! pyramid becomes a faceted solid.

use std::f64::consts::{PI, TAU};

use mitcad_ifc::geometry::{AdvancedFace, Area, Bound, Item, Path, Profile, Seg, Surface};
use mitcad_ifc::math::{self, Matrix};

/// A frame whose axes are the map's images of `m`'s x and y axes made
/// orthonormal (z their cross product), at the image of its origin; with
/// the map from `m`'s coordinates to the frame's.
fn image_frame(a: &Matrix, m: &Matrix) -> Option<(Matrix, Matrix)> {
    let am = math::mul(a, m);
    let u = math::column(&am, 0);
    let v = math::column(&am, 1);
    let x = math::normalize(u)?;
    let z = math::normalize(math::cross(u, v))?;
    let y = math::cross(z, x);
    let frame = math::from_axes(math::origin(&am), x, y, z);
    let to_frame = math::mul(&math::inverse(&frame)?, &am);
    Some((frame, to_frame))
}

fn profile_in(profile: &Profile, map: &Matrix) -> Profile {
    let mut p = profile.transformed(map);
    // Every point stays in the plane: z = 0 exactly.
    for area in &mut p.areas {
        flatten(&mut area.outer);
        for h in &mut area.holes {
            flatten(h);
        }
    }
    p
}

fn flatten(path: &mut Path) {
    for s in &mut path.segments {
        match s {
            Seg::Line { start, end } => {
                start[2] = 0.0;
                end[2] = 0.0;
            }
            Seg::Spline { poles, .. } => poles.iter_mut().for_each(|p| p[2] = 0.0),
            Seg::Arc { frame, .. } | Seg::Ellipse { frame, .. } => frame[2][3] = 0.0,
            // (No profile holds a helix.)
            Seg::Helix { .. } => {}
        }
    }
}

/// A rectangle profile from (0, 0) to (x, y).
fn rectangle(x: f64, y: f64) -> Profile {
    let c = [[0.0, 0.0, 0.0], [x, 0.0, 0.0], [x, y, 0.0], [0.0, y, 0.0]];
    Profile {
        kind: "IFCBLOCK".to_owned(),
        name: String::new(),
        areas: vec![Area {
            outer: Path {
                segments: (0..4)
                    .map(|i| Seg::Line {
                        start: c[i],
                        end: c[(i + 1) % 4],
                    })
                    .collect(),
            },
            holes: Vec::new(),
        }],
    }
}

fn disk(radius: f64) -> Profile {
    Profile {
        kind: "IFCRIGHTCIRCULARCYLINDER".to_owned(),
        name: String::new(),
        areas: vec![Area {
            outer: Path {
                segments: vec![Seg::Arc {
                    frame: math::IDENTITY,
                    radius,
                    start: 0.0,
                    sweep: TAU,
                }],
            },
            holes: Vec::new(),
        }],
    }
}

/// The xz plane of a position as a frame (x along x, y along z): the
/// plane revolution profiles of primitives lie in.
fn xz_frame(position: &Matrix) -> Matrix {
    math::mul(
        position,
        &math::from_axes([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
    )
}

/// A primitive as the item that makes it (see the module docs); other
/// items as they are.
pub fn primitive_as_item(item: &Item) -> Item {
    match item {
        Item::Block { position, size } => Item::Extrusion {
            position: *position,
            profile: rectangle(size[0], size[1]),
            direction: [0.0, 0.0, 1.0],
            depth: size[2],
        },
        Item::Cylinder {
            position,
            radius,
            height,
        } => Item::Extrusion {
            position: *position,
            profile: disk(*radius),
            direction: [0.0, 0.0, 1.0],
            depth: *height,
        },
        Item::Cone {
            position,
            radius,
            height,
        } => {
            let pts = [[0.0, 0.0, 0.0], [*radius, 0.0, 0.0], [0.0, *height, 0.0]];
            Item::Revolution {
                position: xz_frame(position),
                profile: Profile {
                    kind: "IFCRIGHTCIRCULARCONE".to_owned(),
                    name: String::new(),
                    areas: vec![Area {
                        outer: Path {
                            segments: (0..3)
                                .map(|i| Seg::Line {
                                    start: pts[i],
                                    end: pts[(i + 1) % 3],
                                })
                                .collect(),
                        },
                        holes: Vec::new(),
                    }],
                },
                axis_origin: [0.0; 3],
                axis: [0.0, 1.0, 0.0],
                angle: TAU,
            }
        }
        Item::Sphere { position, radius } => Item::Revolution {
            position: xz_frame(position),
            profile: Profile {
                kind: "IFCSPHERE".to_owned(),
                name: String::new(),
                areas: vec![Area {
                    outer: Path {
                        segments: vec![
                            Seg::Line {
                                start: [0.0, -radius, 0.0],
                                end: [0.0, *radius, 0.0],
                            },
                            Seg::Arc {
                                frame: math::IDENTITY,
                                radius: *radius,
                                start: PI / 2.0,
                                sweep: -PI,
                            },
                        ],
                    },
                    holes: Vec::new(),
                }],
            },
            axis_origin: [0.0; 3],
            axis: [0.0, 1.0, 0.0],
            angle: TAU,
        },
        Item::Pyramid { position, size } => {
            let [x, y, h] = *size;
            let p = |q: [f64; 3]| math::apply(position, q);
            let base = [
                p([0.0, 0.0, 0.0]),
                p([x, 0.0, 0.0]),
                p([x, y, 0.0]),
                p([0.0, y, 0.0]),
            ];
            let apex = p([x / 2.0, y / 2.0, h]);
            let mut faces = vec![vec![vec![base[0], base[3], base[2], base[1]]]];
            for i in 0..4 {
                faces.push(vec![vec![base[i], base[(i + 1) % 4], apex]]);
            }
            Item::Faceted {
                faces,
                closed: true,
            }
        }
        Item::Boolean { op, first, second } => Item::Boolean {
            op: *op,
            first: Box::new(primitive_as_item(first)),
            second: Box::new(primitive_as_item(second)),
        },
        other => other.clone(),
    }
}

/// An item moved by `a` (see the module docs), or why it cannot be.
pub fn transformed(item: &Item, a: &Matrix) -> Result<Item, String> {
    let det = math::determinant(a);
    let similarity = math::uniform_scale(a, 1e-9);
    let item = primitive_as_item(item);
    Ok(match &item {
        Item::Extrusion {
            position,
            profile,
            direction,
            depth,
        } => {
            let (frame, to_frame) =
                image_frame(a, position).ok_or("the map flattens the profile")?;
            let along = math::apply_vector(
                &math::mul(&to_frame, &math::IDENTITY),
                math::scale(*direction, *depth),
            );
            let length = math::length(along);
            Item::Extrusion {
                position: frame,
                profile: profile_in(profile, &to_frame),
                direction: math::normalize(along).ok_or("the map flattens the extrusion")?,
                depth: length,
            }
        }
        Item::TaperedExtrusion {
            position,
            profile,
            end_profile,
            direction,
            depth,
        } => {
            let (frame, to_frame) =
                image_frame(a, position).ok_or("the map flattens the profile")?;
            let along = math::apply_vector(&to_frame, math::scale(*direction, *depth));
            Item::TaperedExtrusion {
                position: frame,
                profile: profile_in(profile, &to_frame),
                end_profile: profile_in(end_profile, &to_frame),
                direction: math::normalize(along).ok_or("the map flattens the extrusion")?,
                depth: math::length(along),
            }
        }
        Item::Revolution {
            position,
            profile,
            axis_origin,
            axis,
            angle,
        } => {
            if similarity.is_none() {
                return Err("a revolution under a non-uniform scale".to_owned());
            }
            let (frame, to_frame) =
                image_frame(a, position).ok_or("the map flattens the profile")?;
            let origin = math::apply(&to_frame, *axis_origin);
            let dir = math::normalize(math::apply_vector(&to_frame, *axis)).ok_or("a zero axis")?;
            Item::Revolution {
                position: frame,
                profile: profile_in(profile, &to_frame),
                axis_origin: origin,
                axis: dir,
                angle: if det < 0.0 { -angle } else { *angle },
            }
        }
        Item::Sweep {
            position,
            profile,
            directrix,
            reference,
        } => {
            let s = similarity.ok_or("a sweep under a non-uniform scale")?;
            if det < 0.0 {
                return Err("a mirrored sweep".to_owned());
            }
            let m = math::mul(a, position);
            // The profile keeps its own coordinates, scaled.
            let scaled = [[s, 0.0, 0.0, 0.0], [0.0, s, 0.0, 0.0], [0.0, 0.0, s, 0.0]];
            let rigid = math::mul(&m, &math::inverse(&scaled).expect("s > 0"));
            Item::Sweep {
                position: rigid,
                profile: profile.transformed(&scaled),
                directrix: directrix.transformed(&m),
                reference: reference.and_then(|r| math::normalize(math::apply_vector(&m, r))),
            }
        }
        Item::SweptDisk {
            directrix,
            radius,
            inner_radius,
        } => {
            let s = similarity.ok_or("a swept disk under a non-uniform scale")?;
            Item::SweptDisk {
                directrix: directrix.transformed(a),
                radius: radius * s,
                inner_radius: inner_radius.map(|r| r * s),
            }
        }
        Item::Boolean { op, first, second } => Item::Boolean {
            op: *op,
            first: Box::new(transformed(first, a)?),
            second: Box::new(transformed(second, a)?),
        },
        Item::HalfSpace {
            plane,
            below,
            boundary,
        } => {
            let origin = math::apply(a, math::origin(plane));
            let inverse = math::inverse(a).ok_or("a singular map")?;
            // Normals map by the inverse transpose.
            let n = math::column(plane, 2);
            let normal = math::normalize([
                inverse[0][0] * n[0] + inverse[1][0] * n[1] + inverse[2][0] * n[2],
                inverse[0][1] * n[0] + inverse[1][1] * n[1] + inverse[2][1] * n[2],
                inverse[0][2] * n[0] + inverse[1][2] * n[1] + inverse[2][2] * n[2],
            ])
            .ok_or("a degenerate plane")?;
            let x = math::apply_vector(a, math::column(plane, 0));
            let new_plane = math::frame(origin, Some(normal), Some(x));
            let boundary = match boundary {
                None => None,
                Some((frame, polygon)) => {
                    let (f, to_frame) =
                        image_frame(a, frame).ok_or("the map flattens the boundary")?;
                    let along = math::apply_vector(&to_frame, [0.0, 0.0, 1.0]);
                    if along[0].abs() + along[1].abs() > 1e-9 * math::length(along) {
                        return Err("a boundary skewed by the map".to_owned());
                    }
                    let mut polygon = polygon.transformed(&to_frame);
                    flatten(&mut polygon);
                    Some((f, polygon))
                }
            };
            Item::HalfSpace {
                plane: new_plane,
                below: *below,
                boundary,
            }
        }
        Item::Faceted { faces, closed } => Item::Faceted {
            faces: faces
                .iter()
                .map(|loops| {
                    loops
                        .iter()
                        .map(|l| {
                            let mut pts: Vec<[f64; 3]> =
                                l.iter().map(|p| math::apply(a, *p)).collect();
                            if det < 0.0 {
                                pts.reverse();
                            }
                            pts
                        })
                        .collect()
                })
                .collect(),
            closed: *closed,
        },
        Item::Mesh {
            points,
            triangles,
            closed,
        } => Item::Mesh {
            points: points.iter().map(|p| math::apply(a, *p)).collect(),
            triangles: if det < 0.0 {
                triangles.iter().map(|t| [t[0], t[2], t[1]]).collect()
            } else {
                triangles.clone()
            },
            closed: *closed,
        },
        Item::Advanced { faces, closed } => {
            if similarity.is_none() && faces.iter().any(|f| !affine_surface(&f.surface)) {
                return Err("curved faces under a non-uniform scale".to_owned());
            }
            Item::Advanced {
                faces: faces
                    .iter()
                    .map(|f| advanced_face(f, a, det < 0.0, similarity))
                    .collect(),
                closed: *closed,
            }
        }
        Item::Mapped { map, transform } => Item::Mapped {
            map: *map,
            transform: math::mul(a, transform),
        },
        Item::Unsupported { .. } => item.clone(),
        Item::Block { .. }
        | Item::Cylinder { .. }
        | Item::Cone { .. }
        | Item::Sphere { .. }
        | Item::Pyramid { .. } => unreachable!("primitives are items that make them"),
    })
}

/// Surfaces that stay of their kind under any affine map.
fn affine_surface(s: &Surface) -> bool {
    matches!(
        s,
        Surface::Plane { .. } | Surface::BSpline { .. } | Surface::Extrusion { .. }
    )
}

fn moved_frame(a: &Matrix, frame: &Matrix, scale: f64) -> Matrix {
    let o = math::apply(a, math::origin(frame));
    let x = math::scale(math::apply_vector(a, math::column(frame, 0)), 1.0 / scale);
    let z = math::scale(math::apply_vector(a, math::column(frame, 2)), 1.0 / scale);
    math::frame(o, Some(z), Some(x))
}

fn advanced_face(f: &AdvancedFace, a: &Matrix, mirrored: bool, scale: Option<f64>) -> AdvancedFace {
    let s = scale.unwrap_or(1.0);
    let surface = match &f.surface {
        Surface::Plane { frame } => {
            let (frame, _) = image_frame(a, frame).unwrap_or((*frame, math::IDENTITY));
            Surface::Plane { frame }
        }
        Surface::Cylinder { frame, radius } => Surface::Cylinder {
            frame: moved_frame(a, frame, s),
            radius: radius * s,
        },
        Surface::Cone {
            frame,
            radius,
            semi_angle,
        } => Surface::Cone {
            frame: moved_frame(a, frame, s),
            radius: radius * s,
            semi_angle: *semi_angle,
        },
        Surface::Sphere { frame, radius } => Surface::Sphere {
            frame: moved_frame(a, frame, s),
            radius: radius * s,
        },
        Surface::Torus {
            frame,
            major,
            minor,
        } => Surface::Torus {
            frame: moved_frame(a, frame, s),
            major: major * s,
            minor: minor * s,
        },
        Surface::BSpline {
            u_degree,
            v_degree,
            poles,
            weights,
            u_knots,
            u_multiplicities,
            v_knots,
            v_multiplicities,
        } => Surface::BSpline {
            u_degree: *u_degree,
            v_degree: *v_degree,
            poles: poles
                .iter()
                .map(|row| row.iter().map(|p| math::apply(a, *p)).collect())
                .collect(),
            weights: weights.clone(),
            u_knots: u_knots.clone(),
            u_multiplicities: u_multiplicities.clone(),
            v_knots: v_knots.clone(),
            v_multiplicities: v_multiplicities.clone(),
        },
        Surface::Extrusion { curve, direction } => Surface::Extrusion {
            curve: curve.transformed(a),
            direction: math::apply_vector(a, *direction),
        },
        Surface::Revolution {
            curve,
            origin,
            axis,
        } => Surface::Revolution {
            curve: curve.transformed(a),
            origin: math::apply(a, *origin),
            axis: math::apply_vector(a, *axis),
        },
    };
    // Under a mirror the own normals of planes (rebuilt from their axes'
    // images), B-splines and swept surfaces turn against the image of the
    // face's; those of cylinders, cones, spheres and tori stay outward.
    let flips = mirrored
        && matches!(
            f.surface,
            Surface::Plane { .. }
                | Surface::BSpline { .. }
                | Surface::Extrusion { .. }
                | Surface::Revolution { .. }
        );
    AdvancedFace {
        surface,
        bounds: f
            .bounds
            .iter()
            .map(|b| {
                let mut edges: Vec<_> = b
                    .edges
                    .iter()
                    .map(|e| {
                        let mut e = e.clone();
                        e.curve = e.curve.transformed(a);
                        e.start = math::apply(a, e.start);
                        e.end = math::apply(a, e.end);
                        e
                    })
                    .collect();
                if mirrored {
                    // Loops keep the face on their left only when run the
                    // other way in a mirror image.
                    edges.reverse();
                    for e in &mut edges {
                        std::mem::swap(&mut e.start, &mut e.end);
                        e.vertices.swap(0, 1);
                        e.forward = !e.forward;
                    }
                }
                Bound {
                    outer: b.outer,
                    edges,
                }
            })
            .collect(),
        same_sense: f.same_sense != flips,
    }
}

/// A box that holds an item (in its coordinates), from its profiles'
/// and points' samples; None for what has none.
pub fn bounds(item: &Item) -> Option<([f64; 3], [f64; 3])> {
    let mut points: Vec<[f64; 3]> = Vec::new();
    let profile_points = |position: &Matrix, profile: &Profile, out: &mut Vec<[f64; 3]>| {
        for area in &profile.areas {
            for s in &area.outer.segments {
                out.extend(s.sample(8).into_iter().map(|p| math::apply(position, p)));
            }
        }
    };
    match &primitive_as_item(item) {
        Item::Extrusion {
            position,
            profile,
            direction,
            depth,
        }
        | Item::TaperedExtrusion {
            position,
            profile,
            direction,
            depth,
            ..
        } => {
            let mut base = Vec::new();
            profile_points(position, profile, &mut base);
            let shift = math::apply_vector(position, math::scale(*direction, *depth));
            points.extend(base.iter().map(|p| math::add(*p, shift)));
            points.extend(base);
        }
        Item::Revolution {
            position,
            profile,
            axis_origin,
            ..
        } => {
            let mut base = Vec::new();
            profile_points(position, profile, &mut base);
            let c = math::apply(position, *axis_origin);
            let r = base
                .iter()
                .map(|p| math::length(math::sub(*p, c)))
                .fold(0.0, f64::max);
            points.push(math::add(c, [r, r, r]));
            points.push(math::sub(c, [r, r, r]));
        }
        Item::Sweep {
            position,
            profile,
            directrix,
            ..
        } => {
            let mut base = Vec::new();
            profile_points(&math::IDENTITY, profile, &mut base);
            let r = base.iter().map(|p| math::length(*p)).fold(0.0, f64::max);
            for s in &directrix.segments {
                for p in s.sample(8) {
                    let p = math::apply(position, p);
                    points.push(math::add(p, [r, r, r]));
                    points.push(math::sub(p, [r, r, r]));
                }
            }
        }
        Item::SweptDisk {
            directrix, radius, ..
        } => {
            for s in &directrix.segments {
                for p in s.sample(8) {
                    points.push(math::add(p, [*radius; 3]));
                    points.push(math::sub(p, [*radius; 3]));
                }
            }
        }
        Item::Boolean { op, first, second } => {
            let a = bounds(first);
            return match op {
                mitcad_ifc::geometry::BoolOp::Union => union(a, bounds(second)),
                _ => a,
            };
        }
        Item::Faceted { faces, .. } => points.extend(faces.iter().flatten().flatten()),
        Item::Mesh { points: pts, .. } => points.extend(pts),
        Item::Advanced { faces, .. } => {
            for f in faces {
                for b in &f.bounds {
                    for e in &b.edges {
                        for s in &e.curve.segments {
                            points.extend(s.sample(8));
                        }
                    }
                }
            }
        }
        _ => return None,
    }
    let first = *points.first()?;
    let mut lo = first;
    let mut hi = first;
    for p in &points {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    Some((lo, hi))
}

fn union(
    a: Option<([f64; 3], [f64; 3])>,
    b: Option<([f64; 3], [f64; 3])>,
) -> Option<([f64; 3], [f64; 3])> {
    match (a, b) {
        (Some((a0, a1)), Some((b0, b1))) => Some((
            std::array::from_fn(|k| a0[k].min(b0[k])),
            std::array::from_fn(|k| a1[k].max(b1[k])),
        )),
        (a, None) => a,
        (None, b) => b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrusions_move_and_scale() {
        let item = Item::Block {
            position: math::IDENTITY,
            size: [10.0, 20.0, 30.0],
        };
        // A non-uniform scale and a turn of 90 degrees about z.
        let a = [
            [0.0, -2.0, 0.0, 5.0],
            [3.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.5, 1.0],
        ];
        let Item::Extrusion {
            position,
            profile,
            direction,
            depth,
        } = transformed(&item, &a).expect("moves")
        else {
            panic!("an extrusion");
        };
        assert!(math::is_rigid(&position, 1e-12));
        assert!((depth - 15.0).abs() < 1e-12);
        assert!((direction[2] - 1.0).abs() < 1e-12);
        // The rectangle is 30 x 40 now (x scaled by 3, y by 2).
        let pts: Vec<[f64; 3]> = profile.areas[0]
            .outer
            .segments
            .iter()
            .map(Seg::start)
            .collect();
        let w = pts.iter().map(|p| p[0]).fold(f64::MIN, f64::max)
            - pts.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        let h = pts.iter().map(|p| p[1]).fold(f64::MIN, f64::max)
            - pts.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
        assert!((w * h - 1200.0).abs() < 1e-9, "{w} x {h}");
        let (lo, hi) = bounds(&Item::Extrusion {
            position,
            profile,
            direction,
            depth,
        })
        .expect("bounded");
        assert!((hi[2] - lo[2] - 15.0).abs() < 1e-9);
    }

    #[test]
    fn half_spaces_keep_their_side() {
        let item = Item::HalfSpace {
            plane: math::frame([0.0, 0.0, 5.0], None, None),
            below: false,
            boundary: None,
        };
        let a = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
        ];
        let Item::HalfSpace { plane, below, .. } = transformed(&item, &a).expect("moves") else {
            panic!("a half space");
        };
        assert!(!below);
        assert_eq!(math::origin(&plane), [0.0, 0.0, 10.0]);
        assert!((math::column(&plane, 2)[2] - 1.0).abs() < 1e-12);
    }
}
