// SPDX-License-Identifier: MIT
//! Adapt legacy reference-plane profiles to directed material charts once.

use super::*;
use crate::sketch::geometry::{Curve2, Nurbs, angle_after, dist};

/// The adapter's methods of [`BoundedCurve2`], which lives below the model
/// with the kernel's sheet types (mitcad#777) while these need the
/// sketch's curves.
pub(crate) trait BoundedCurve2Ext: Sized {
    fn from_geometry(geometry: SegmentGeometry) -> Result<Self, String>;
    fn support(&self) -> Result<Curve2, String>;
    fn point(&self, parameter: f64) -> Result<[f64; 2], String>;
    fn ends(&self) -> Result<([f64; 2], [f64; 2]), String>;
    fn geometry_piece(&self) -> Result<SegmentGeometry, String>;
    fn orientation_samples(&self) -> Result<Vec<[f64; 2]>, String>;
}

impl BoundedCurve2Ext for BoundedCurve2 {
    fn from_geometry(geometry: SegmentGeometry) -> Result<Self, String> {
        let (first, last) = match &geometry {
            SegmentGeometry::Line { .. } => (0.0, 1.0),
            SegmentGeometry::Circle { .. } | SegmentGeometry::Ellipse { .. } => {
                (0.0, std::f64::consts::TAU)
            }
            SegmentGeometry::Arc {
                start_angle,
                end_angle,
                ..
            }
            | SegmentGeometry::EllipseArc {
                start_angle,
                end_angle,
                ..
            } => (*start_angle, angle_after(*start_angle, *end_angle)),
            SegmentGeometry::BSpline { knots, .. } => (
                *knots.first().ok_or("empty sheet spline")?,
                *knots.last().ok_or("empty sheet spline")?,
            ),
        };
        if !first.is_finite() || !last.is_finite() || first >= last {
            return Err("invalid sheet curve parameter range".into());
        }
        Ok(Self {
            geometry,
            range: ParamSpan { first, last },
        })
    }

    fn support(&self) -> Result<Curve2, String> {
        Ok(match &self.geometry {
            SegmentGeometry::Line { start, end } => Curve2::Line { a: *start, b: *end },
            SegmentGeometry::Circle { center, radius, .. }
            | SegmentGeometry::Arc { center, radius, .. } => Curve2::Circle {
                center: *center,
                radius: *radius,
                start: 0.0,
            },
            SegmentGeometry::Ellipse {
                center,
                major_radius,
                minor_radius,
                rotation,
            }
            | SegmentGeometry::EllipseArc {
                center,
                major_radius,
                minor_radius,
                rotation,
                ..
            } => Curve2::Ellipse {
                center: *center,
                major: *major_radius,
                minor: *minor_radius,
                rotation: *rotation,
            },
            SegmentGeometry::BSpline {
                degree,
                poles,
                weights,
                knots,
                multiplicities,
                periodic,
            } => {
                if *periodic {
                    return Err("periodic sheet support requires a complete closed curve".into());
                }
                let knots: Vec<_> = knots
                    .iter()
                    .zip(multiplicities)
                    .flat_map(|(k, m)| std::iter::repeat_n(*k, *m as usize))
                    .collect();
                if poles.len() <= *degree as usize
                    || knots.len() != poles.len() + *degree as usize + 1
                    || (!weights.is_empty() && weights.len() != poles.len())
                {
                    return Err("invalid sheet spline support".into());
                }
                Curve2::Nurbs(Nurbs {
                    degree: *degree as usize,
                    control: poles.clone(),
                    weights: weights.clone(),
                    knots,
                })
            }
        })
    }

    fn point(&self, parameter: f64) -> Result<[f64; 2], String> {
        Ok(self.support()?.point(parameter))
    }

    fn ends(&self) -> Result<([f64; 2], [f64; 2]), String> {
        Ok((self.point(self.range.first)?, self.point(self.range.last)?))
    }

    fn geometry_piece(&self) -> Result<SegmentGeometry, String> {
        if let SegmentGeometry::BSpline {
            periodic: true,
            knots,
            ..
        } = &self.geometry
        {
            if Some(&self.range.first.min(self.range.last)) == knots.first()
                && Some(&self.range.first.max(self.range.last)) == knots.last()
            {
                return Ok(self.geometry.clone());
            }
            return Err("a partial periodic spline needs a bounded kernel representation".into());
        }
        let support = self.support()?;
        let a = self.range.first.min(self.range.last);
        let b = self.range.first.max(self.range.last);
        if matches!(
            self.geometry,
            SegmentGeometry::Circle { .. } | SegmentGeometry::Ellipse { .. }
        ) && (b - a - std::f64::consts::TAU).abs() < 1e-10
        {
            return Ok(support.whole());
        }
        Ok(support.piece(a, b))
    }

    fn orientation_samples(&self) -> Result<Vec<[f64; 2]>, String> {
        if let SegmentGeometry::BSpline {
            poles,
            periodic: true,
            ..
        } = &self.geometry
        {
            let mut points = poles.clone();
            if self.range.first > self.range.last {
                points.reverse();
            }
            return Ok(points);
        }
        // Samples determine winding only; all material geometry remains exact.
        let support = self.support()?;
        let count = if matches!(self.geometry, SegmentGeometry::Line { .. }) {
            1
        } else {
            128
        };
        Ok((0..=count)
            .map(|i| {
                support.point(
                    self.range.first
                        + (self.range.last - self.range.first) * (i as f64) / (count as f64),
                )
            })
            .collect())
    }
}

/// The adapter's constructor of [`MaterialRegion`] (mitcad#777, as
/// [`BoundedCurve2Ext`]).
pub(crate) trait MaterialRegionExt: Sized {
    fn from_legacy(region: &SheetRegion) -> Result<Self, String>;
}

impl MaterialRegionExt for MaterialRegion {
    fn from_legacy(region: &SheetRegion) -> Result<Self, String> {
        let mut loops = Vec::new();
        for (loop_index, old) in region.loops.iter().enumerate() {
            let edges: Vec<_> = old
                .segments
                .iter()
                .map(|s| {
                    Ok(MaterialEdge {
                        // A repeated alias deliberately remains ambiguous. A traversal
                        // index is not a stable source-piece identity.
                        piece: CurvePieceRef {
                            curve: s.id.clone(),
                            piece: 0,
                        },
                        curve: BoundedCurve2::from_geometry(s.geometry.clone())?,
                    })
                })
                .collect::<Result<_, String>>()?;
            if edges.is_empty() {
                return Err("empty material loop".into());
            }
            let orient = |reverse_first: bool| -> Result<Vec<MaterialEdge>, String> {
                let mut directed = edges.clone();
                if reverse_first {
                    directed[0].curve.range = directed[0].curve.range.reversed();
                }
                if directed.len() == 1
                    && matches!(
                        directed[0].curve.geometry,
                        SegmentGeometry::BSpline { periodic: true, .. }
                    )
                {
                    return Ok(directed);
                }
                let start = directed[0].curve.ends()?.0;
                let mut end = directed[0].curve.ends()?.1;
                for edge in directed.iter_mut().skip(1) {
                    let (a, b) = edge.curve.ends()?;
                    if dist(end, a) <= TOLERANCE {
                        end = b;
                    } else if dist(end, b) <= TOLERANCE {
                        edge.curve.range = edge.curve.range.reversed();
                        end = a;
                    } else {
                        return Err("sheet boundary is not a connected ordered loop".into());
                    }
                }
                if dist(start, end) > TOLERANCE {
                    return Err("sheet boundary is not closed".into());
                }
                Ok(directed)
            };
            let mut directed = orient(false).or_else(|_| orient(true))?;
            let mut points = Vec::new();
            for edge in &directed {
                points.extend(edge.curve.orientation_samples()?);
            }
            let area: f64 = points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .take(points.len())
                .map(|(a, b)| a[0] * b[1] - a[1] * b[0])
                .sum();
            if !area.is_finite() || area.abs() <= TOLERANCE * TOLERANCE {
                return Err("sheet boundary has indeterminate winding".into());
            }
            if (area > 0.0) != (loop_index == 0) {
                directed.reverse();
                for edge in &mut directed {
                    edge.curve.range = edge.curve.range.reversed();
                }
            }
            loops.push(MaterialLoop { edges: directed });
        }
        Ok(Self { loops })
    }
}

/// The adapter's constructor of [`SheetPatch`] (mitcad#777, as
/// [`BoundedCurve2Ext`]).
pub(crate) trait SheetPatchExt: Sized {
    fn from_legacy(
        panel: &SheetPlanarPatch,
        thickness: f64,
        reference_side: SheetReferenceSide,
    ) -> Result<Self, String>;
}

impl SheetPatchExt for SheetPatch {
    fn from_legacy(
        panel: &SheetPlanarPatch,
        thickness: f64,
        reference_side: SheetReferenceSide,
    ) -> Result<Self, String> {
        let mut frame = panel.frame;
        if reference_side == SheetReferenceSide::Back {
            let normal = frame.normal();
            for (i, n) in normal.iter().enumerate() {
                frame.origin[i] -= thickness * n;
            }
        }
        Ok(Self {
            domain: SheetDomainRef::Panel(panel.id),
            regions: vec![MaterialRegion::from_legacy(&panel.region)?],
            embedding: SheetEmbedding::Plane {
                front_frame: frame,
                front_to_back_sign: MaterialParity::Positive,
            },
        })
    }
}

/// Exact full-side adjacency within one native Face. These semantic seams
/// survive the union of coplanar caps and are available to subsequent bends.
pub(crate) fn planar_joins(patches: &[SheetPatch]) -> Result<Vec<ShapePort>, String> {
    use mitcad_vec3::dot;
    let mut joins = Vec::new();
    for (i, a) in patches.iter().enumerate() {
        for b in &patches[i + 1..] {
            let (
                SheetEmbedding::Plane {
                    front_frame: fa,
                    front_to_back_sign: sa,
                },
                SheetEmbedding::Plane {
                    front_frame: fb,
                    front_to_back_sign: sb,
                },
            ) = (&a.embedding, &b.embedding)
            else {
                continue;
            };
            if dot(fa.normal(), fb.normal()) * sa.sign() * sb.sign() < 1.0 - 1e-9 {
                continue;
            }
            for ea in a
                .regions
                .iter()
                .flat_map(|r| &r.loops)
                .flat_map(|l| &l.edges)
            {
                if !matches!(ea.curve.geometry, SegmentGeometry::Line { .. }) {
                    continue;
                }
                let (pa, qa) = ea.curve.ends()?;
                let pa = fa.point(pa);
                let qa = fa.point(qa);
                let delta: [f64; 3] = std::array::from_fn(|j| qa[j] - pa[j]);
                let length = dot(delta, delta).sqrt();
                if length <= TOLERANCE {
                    continue;
                }
                let axis = delta.map(|v| v / length);
                for eb in b
                    .regions
                    .iter()
                    .flat_map(|r| &r.loops)
                    .flat_map(|l| &l.edges)
                {
                    if !matches!(eb.curve.geometry, SegmentGeometry::Line { .. }) {
                        continue;
                    }
                    let (pb, qb) = eb.curve.ends()?;
                    let pb = fb.point(pb);
                    let qb = fb.point(qb);
                    let distance = |x: [f64; 3], y: [f64; 3]| {
                        let d = std::array::from_fn(|j| x[j] - y[j]);
                        dot(d, d).sqrt()
                    };
                    if distance(pa, qb) > TOLERANCE || distance(qa, pb) > TOLERANCE {
                        continue;
                    }
                    let side = |patch: &SheetPatch,
                                edge: &MaterialEdge,
                                first: [f64; 3],
                                last: [f64; 3]| {
                        let at =
                            |point: [f64; 3]| dot(std::array::from_fn(|j| point[j] - pa[j]), axis);
                        let range = edge.curve.range;
                        let scale = (at(last) - at(first)) / (range.last - range.first);
                        ShapePortSide {
                            boundary: BoundaryPieceRef {
                                domain: patch.domain,
                                piece: edge.piece.clone(),
                            },
                            span: range,
                            axial: AxialMap {
                                offset: at(first) - scale * range.first,
                                scale,
                            },
                        }
                    };
                    joins.push(ShapePort {
                        token: joins.len() as u32,
                        axial_interval: ParamSpan {
                            first: 0.0,
                            last: length,
                        },
                        first: side(a, ea, pa, qa),
                        second: side(b, eb, pb, qb),
                    });
                }
            }
        }
    }
    Ok(joins)
}
