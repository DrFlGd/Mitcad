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
    for (i, c) in d.corners.iter().enumerate() {
        let slot = d
            .corner_slots
            .iter()
            .find(|s| s.corner == *c)
            .map_or(i as u32, |s| s.slot);
        apply(ctx, &mut state, c, slot * 4, Some(d.radius), None)?;
    }
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
    for (i, c) in d.corners.iter().enumerate() {
        let slot = d
            .corner_slots
            .iter()
            .find(|s| s.corner == *c)
            .map_or(i as u32, |s| s.slot);
        apply(
            ctx,
            &mut state,
            c,
            slot * 4,
            None,
            Some([d.distances.first, d.distances.second]),
        )?;
    }
    publish(ctx, state, &rules, None)
}

fn apply<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    state: &mut SheetState,
    c: &SheetCorner,
    slot: u32,
    radius: Option<f64>,
    distances: Option<[f64; 2]>,
) -> Result<(), String> {
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
    let amounts = if let Some(r) = radius {
        let tangent = r / (beta / 2.0).tan();
        [tangent, tangent]
    } else {
        distances.expect("corner mode")
    };
    if amounts[0] >= norm(sub(a0, vertex)) - TOLERANCE
        || amounts[1] >= norm(sub(b1, vertex)) - TOLERANCE
    {
        return Err("sheet corner exceeds its adjacent material boundaries".into());
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
    let tool = region(ctx.uid, slot, curves)?;
    let convex = cross(sub(vertex, a0), sub(b1, vertex)) > 0.0;
    trim(
        ctx,
        state,
        &[SheetDomainRef::Panel(c.panel)],
        if convex {
            SheetTrimOperation::Difference
        } else {
            SheetTrimOperation::Union
        },
        &[tool],
        &[Isometry2::IDENTITY],
    )
}
