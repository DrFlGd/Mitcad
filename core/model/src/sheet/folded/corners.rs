// SPDX-License-Identifier: MIT
//! Ordered contour corners are trimmed through the material thickness.

use super::*;
use crate::features::sheet_folded::{SheetCorner, SheetCornerChamferDef, SheetCornerRoundDef};
use crate::sketch::geometry::{add, cross, dist, dot, norm, scale, sub, unit};

pub(crate) fn corner_round<K: Kernel>(
    d: &SheetCornerRoundDef<f64>,
    ctx: &mut EvalContext<'_, K>,
) -> Result<FeatureOutput<K::Shape>, String> {
    if !d.radius.is_finite() || d.radius <= TOLERANCE {
        return Err("sheet corner radius must be positive".into());
    }
    let (mut state, rules) = read(ctx, d.body)?;
    let corners = slots(&d.corners, &d.corner_slots);
    apply_all(ctx, &mut state, corners, Some(d.radius), None)?;
    publish(ctx, state, &rules, None)
}

pub(crate) fn corner_chamfer<K: Kernel>(
    d: &SheetCornerChamferDef<f64>,
    ctx: &mut EvalContext<'_, K>,
) -> Result<FeatureOutput<K::Shape>, String> {
    if [d.distances.first, d.distances.second]
        .iter()
        .any(|v| !v.is_finite() || *v <= TOLERANCE)
    {
        return Err("sheet corner distances must be positive".into());
    }
    let (mut state, rules) = read(ctx, d.body)?;
    let corners = slots(&d.corners, &d.corner_slots);
    apply_all(
        ctx,
        &mut state,
        corners,
        None,
        Some([d.distances.first, d.distances.second]),
    )?;
    publish(ctx, state, &rules, None)
}

/// Each corner with the first slot of its tool's curves.
fn slots(
    corners: &[SheetCorner],
    settled: &[crate::features::sheet_folded::CornerSlot],
) -> Vec<(SheetCorner, u32)> {
    corners
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let slot = settled
                .iter()
                .find(|s| s.corner == *c)
                .map_or(i as u32, |s| s.slot);
            (c.clone(), slot * 4)
        })
        .collect()
}

/// A corner's tool in its panel's chart and whether it removes material
/// (a convex corner) or adds it, from the sheet before the feature: every
/// corner of a feature is found on its input, so that an earlier corner's
/// trim (which splits and renumbers the pieces of the curves) does not
/// change what a later one names (mitcad#280).
fn corner_tool(
    feature: FeatureUid,
    state: &SheetState,
    c: &SheetCorner,
    slot: u32,
    radius: Option<f64>,
    distances: Option<[f64; 2]>,
) -> Result<(MaterialRegion, bool), String> {
    let incoming = BoundaryRef {
        panel: c.panel,
        curve: c.incoming.clone(),
        piece: c.incoming_piece,
        range: None,
    };
    let outgoing = BoundaryRef {
        panel: c.panel,
        curve: c.outgoing.clone(),
        piece: c.outgoing_piece,
        range: None,
    };
    let a = boundary(state, &incoming)?.clone();
    let b = boundary(state, &outgoing)?.clone();
    let (a0, vertex) = line(&a)?;
    let (v1, b1) = line(&b)?;
    if dist(vertex, v1) > TOLERANCE {
        return Err(
            "sheet corner references must be ordered incoming and outgoing at one vertex".into(),
        );
    }
    if state.joins.iter().any(|p| {
        [&p.first, &p.second].iter().any(|side| {
            side.boundary.domain == SheetDomainRef::Panel(c.panel)
                && (side.boundary.piece == a.piece || side.boundary.piece == b.piece)
        })
    }) {
        return Err("sheet corner touches a tangent port".into());
    }
    let va = unit(sub(a0, vertex));
    let vb = unit(sub(b1, vertex));
    let beta = dot(va, vb).clamp(-1.0, 1.0).acos();
    if beta <= 1e-9 || std::f64::consts::PI - beta <= 1e-9 {
        return Err("sheet corner is degenerate".into());
    }
    let mut amounts = if let Some(r) = radius {
        let tangent = r / (beta / 2.0).tan();
        [tangent, tangent]
    } else {
        distances.expect("corner mode")
    };
    // A corner may take a whole adjacent boundary, to its far vertex
    // (mitcad#242: a chamfer as long as the side of a joined face); within
    // the modelling tolerance it ends exactly there.
    let lengths = [norm(sub(a0, vertex)), norm(sub(b1, vertex))];
    // A round whose one side is shorter than its tangent length and ends
    // on an arc: the round takes that side whole and is tangent to the arc
    // beyond it (mitcad#280, a round next to an earlier round).
    if let Some(r) = radius {
        let convex = cross(sub(vertex, a0), sub(b1, vertex)) > 0.0;
        if amounts[1] > lengths[1] + TOLERANCE && amounts[0] <= lengths[0] + TOLERANCE {
            let next = neighbour(state, c.panel, &b, true)?;
            return round_past(feature, slot, r, (a0, vertex), b1, &next, convex);
        }
        if amounts[0] > lengths[0] + TOLERANCE && amounts[1] <= lengths[1] + TOLERANCE {
            let previous = neighbour(state, c.panel, &a, false)?;
            return round_past(feature, slot, r, (b1, vertex), a0, &previous, convex);
        }
    }
    for (amount, length) in amounts.iter_mut().zip(lengths) {
        if *amount > length + TOLERANCE {
            return Err("sheet corner exceeds its adjacent material boundaries".into());
        }
        if (*amount - length).abs() <= 1e-9 * length.max(1.0) {
            *amount = length;
        }
    }
    let pa = add(vertex, scale(va, amounts[0]));
    let pb = add(vertex, scale(vb, amounts[1]));
    let mut curves = vec![SegmentGeometry::Line {
        start: vertex,
        end: pa,
    }];
    if let Some(r) = radius {
        let centre = add(vertex, scale(unit(add(va, vb)), r / (beta / 2.0).sin()));
        let start = (pa[1] - centre[1]).atan2(pa[0] - centre[0]);
        let end = (pb[1] - centre[1]).atan2(pb[0] - centre[0]);
        let (start_angle, end_angle) = if cross(sub(pa, centre), sub(pb, centre)) > 0.0 {
            (start, end)
        } else {
            (end, start)
        };
        curves.push(SegmentGeometry::Arc {
            center: centre,
            radius: r,
            start_angle,
            end_angle,
        });
    } else {
        curves.push(SegmentGeometry::Line { start: pa, end: pb });
    }
    curves.push(SegmentGeometry::Line {
        start: pb,
        end: vertex,
    });
    let tool = region(feature, slot, curves)?;
    let convex = cross(sub(vertex, a0), sub(b1, vertex)) > 0.0;
    Ok((tool, convex))
}

/// The edge after (`after`) or before `edge` in its contour of `panel`.
fn neighbour(
    state: &SheetState,
    panel: PanelId,
    edge: &MaterialEdge,
    after: bool,
) -> Result<MaterialEdge, String> {
    let patch = chart(state, panel)?;
    for contour in patch.regions.iter().flat_map(|r| &r.loops) {
        if let Some(i) = contour.edges.iter().position(|e| e == edge) {
            let n = contour.edges.len();
            let j = if after { (i + 1) % n } else { (i + n - 1) % n };
            return Ok(contour.edges[j].clone());
        }
    }
    Err("sheet corner's boundary is not in its panel's contour".into())
}

/// The angle of a point about a centre.
fn angle_of(centre: [f64; 2], p: [f64; 2]) -> f64 {
    (p[1] - centre[1]).atan2(p[0] - centre[0])
}

/// The arc of a circle from `p` to `q` that turns less than half a turn.
fn minor_arc(centre: [f64; 2], radius: f64, p: [f64; 2], q: [f64; 2]) -> SegmentGeometry {
    let (a, b) = (angle_of(centre, p), angle_of(centre, q));
    let ccw = (b - a).rem_euclid(std::f64::consts::TAU);
    let (start_angle, end_angle) = if ccw <= std::f64::consts::PI {
        (a, a + ccw)
    } else {
        (b, b + (a - b).rem_euclid(std::f64::consts::TAU))
    };
    SegmentGeometry::Arc {
        center: centre,
        radius,
        start_angle,
        end_angle,
    }
}

/// A round of radius `r` at the corner `vertex` between the line from
/// `far` (the side long enough) and a short side ending at `short_end`,
/// beyond which the contour continues on the arc `next`: the round's circle
/// is tangent to the long side's line and to the arc's circle, and the
/// tool is the material between the contour (the long side's end, the
/// short side, the arc up to the tangent point) and the round. Of the
/// circles tangent to both, the one touching the long side within it and
/// the arc within its extent, on the short side's side of the long line,
/// must be the only one.
fn round_past(
    feature: FeatureUid,
    slot: u32,
    r: f64,
    (far, vertex): ([f64; 2], [f64; 2]),
    short_end: [f64; 2],
    next: &MaterialEdge,
    convex: bool,
) -> Result<(MaterialRegion, bool), String> {
    let (centre, radius) = match next.curve.geometry {
        SegmentGeometry::Circle { center, radius, .. }
        | SegmentGeometry::Arc { center, radius, .. } => (center, radius),
        _ => return Err("sheet corner exceeds its adjacent material boundaries".into()),
    };
    let u = unit(sub(vertex, far));
    let length = norm(sub(vertex, far));
    // The normal of the long line towards the short side.
    let mut n = [-u[1], u[0]];
    if dot(n, sub(short_end, vertex)) < 0.0 {
        n = scale(n, -1.0);
    }
    let w = sub(add(vertex, scale(n, r)), centre);
    let on_arc = |p: [f64; 2]| {
        let (lo, hi) = (
            next.curve.range.first.min(next.curve.range.last),
            next.curve.range.first.max(next.curve.range.last),
        );
        // (Sampled: within half a step's chord of a sample.)
        let reach = radius * (hi - lo) / 4096.0 + 1e-6;
        (0..=2048).any(|k| {
            let t = lo + (hi - lo) * f64::from(k) / 2048.0;
            next.curve.point(t).is_ok_and(|q| dist(p, q) <= reach)
        })
    };
    let mut found = Vec::new();
    for d in [radius + r, (radius - r).abs()] {
        let b = dot(w, u);
        let disc = b * b - (dot(w, w) - d * d);
        if disc < 0.0 {
            continue;
        }
        for s in [-b - disc.sqrt(), -b + disc.sqrt()] {
            // On the long side, at or before the corner.
            if s > TOLERANCE || s < -length - TOLERANCE {
                continue;
            }
            let p = add(add(vertex, scale(u, s)), scale(n, r));
            let to = unit(sub(p, centre));
            let touch = add(centre, scale(to, radius));
            if (norm(sub(p, centre)) - d).abs() > 1e-9 * d.max(1.0) || !on_arc(touch) {
                continue;
            }
            found.push((p, add(vertex, scale(u, s)), touch));
        }
    }
    let [(p, t1, t2)] = found.as_slice() else {
        return Err(format!(
            "sheet corner exceeds its adjacent material boundaries ({} rounds tangent to the \
             arc beyond its short side)",
            found.len()
        ));
    };
    let curves = vec![
        SegmentGeometry::Line {
            start: *t1,
            end: vertex,
        },
        SegmentGeometry::Line {
            start: vertex,
            end: short_end,
        },
        minor_arc(centre, radius, short_end, *t2),
        minor_arc(*p, r, *t2, *t1),
    ];
    Ok((region(feature, slot, curves)?, convex))
}

/// The corners' tools found on the input (`corner_tool`), then each trims
/// its panel.
fn apply_all<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    state: &mut SheetState,
    corners: Vec<(SheetCorner, u32)>,
    radius: Option<f64>,
    distances: Option<[f64; 2]>,
) -> Result<(), String> {
    let tools = corners
        .iter()
        .map(|(c, slot)| {
            corner_tool(ctx.uid, state, c, *slot, radius, distances).map(|t| (c.panel, t))
        })
        .collect::<Result<Vec<_>, String>>()?;
    for (panel, (tool, convex)) in tools {
        trim(
            ctx,
            state,
            &[SheetDomainRef::Panel(panel)],
            if convex {
                SheetTrimOperation::Difference
            } else {
                SheetTrimOperation::Union
            },
            &[tool],
            &[Isometry2::IDENTITY],
        )?;
    }
    Ok(())
}
