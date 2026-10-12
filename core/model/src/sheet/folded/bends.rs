// SPDX-License-Identifier: MIT
//! Tangent construction and material-side correspondence of native bends.

use super::*;
use crate::features::sheet::{Allowance, Relief, ReliefKind};
use crate::features::sheet_folded::*;
use crate::profile::dot;
use crate::sketch::geometry as g2;
use crate::transform::Transform;

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    a.map(|v| v * s)
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

struct BendMetrics {
    angle: f64,
    radius: f64,
    thickness: f64,
    k: f64,
    allowance: f64,
    inner: f64,
    outer: f64,
}
impl BendMetrics {
    fn new(angle: f64, radius: Option<f64>, rules: &SheetRulesDef<f64>) -> Result<Self, String> {
        let radius = radius.unwrap_or(rules.bend_radius);
        if !angle.is_finite() || angle.abs() <= 1e-9 || angle.abs() >= std::f64::consts::PI - 1e-9 {
            return Err("sheet bend angle must be nonzero and less than 180 degrees".into());
        }
        if !radius.is_finite() || radius <= TOLERANCE {
            return Err("sheet inside radius must be positive".into());
        }
        let Allowance::KFactor { k } = &rules.allowance;
        let k = *k;
        let setback = (angle.abs() / 2.0).tan();
        Ok(Self {
            angle,
            radius,
            thickness: rules.thickness,
            k,
            allowance: angle.abs() * (radius + k * rules.thickness),
            inner: radius * setback,
            outer: (radius + rules.thickness) * setback,
        })
    }
    fn offset(&self, position: SheetPosition) -> f64 {
        match position {
            SheetPosition::Inside => -self.outer,
            SheetPosition::Outside => -self.inner,
            SheetPosition::BendLine => -self.allowance / 2.0,
        }
    }
}

/// Axis follows the canonical boundary orientation e x m = material normal.
struct PortFrame {
    origin: [f64; 3],
    m: [f64; 3],
    e: [f64; 3],
    n: [f64; 3],
    to_operation: Isometry2,
    width: f64,
}

fn port_frame(state: &SheetState, reference: &BoundaryRef) -> Result<PortFrame, String> {
    let edge = boundary(state, reference)?;
    let (a, b) = line(edge)?;
    let patch = chart(state, reference.panel)?;
    let (frame, parity) = plane(patch)?;
    let along = g2::unit(g2::sub(b, a));
    let m = [along[1], -along[0]];
    let e = g2::scale(along, -parity.sign());
    let start = if parity == MaterialParity::Positive {
        b
    } else {
        a
    };
    Ok(PortFrame {
        origin: frame.point(start),
        m: add(scale(frame.x_axis, m[0]), scale(frame.y_axis, m[1])),
        e: add(scale(frame.x_axis, e[0]), scale(frame.y_axis, e[1])),
        n: scale(frame.normal(), parity.sign()),
        to_operation: Isometry2 {
            origin: [-g2::dot(start, m), -g2::dot(start, e)],
            x_axis: [m[0], e[0]],
            y_axis: [m[1], e[1]],
        },
        width: g2::dist(a, b),
    })
}

fn free(state: &SheetState, reference: &BoundaryRef) -> Result<(), String> {
    let edge = boundary(state, reference)?;
    if state.joins.iter().any(|p| {
        [&p.first, &p.second].iter().any(|s| {
            s.boundary.domain == SheetDomainRef::Panel(reference.panel)
                && s.boundary.piece == edge.piece
        })
    }) {
        return Err("sheet flange boundary is already joined to another material domain".into());
    }
    Ok(())
}

fn extent(patch: &SheetPatch, map: Isometry2) -> Result<([f64; 2], [f64; 2]), String> {
    let (a, b) = bounds(patch)?;
    let points = [a, [a[0], b[1]], b, [b[0], a[1]]].map(|p| map.point(p));
    Ok((
        std::array::from_fn(|i| points.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min)),
        std::array::from_fn(|i| {
            points
                .iter()
                .map(|p| p[i])
                .fold(f64::NEG_INFINITY, f64::max)
        }),
    ))
}

fn margin(min: [f64; 2], max: [f64; 2]) -> f64 {
    1.0 + 10.0 * ((max[0] - min[0]).abs() + (max[1] - min[1]).abs())
}

fn side(
    patch: &SheetPatch,
    piece: &CurvePieceRef,
    to_axial: Isometry2,
) -> Result<ShapePortSide, String> {
    let edges: Vec<_> = patch
        .regions
        .iter()
        .flat_map(|r| &r.loops)
        .flat_map(|l| &l.edges)
        .filter(|e| &e.piece == piece)
        .collect();
    if edges.len() != 1 {
        return Err("tangent port must be one unique uninterrupted straight boundary".into());
    }
    let edge = edges[0];
    let (a, b) = line(edge)?;
    let first = to_axial.point(a)[1];
    let last = to_axial.point(b)[1];
    let range = edge.curve.range;
    let derivative = (last - first) / (range.last - range.first);
    Ok(ShapePortSide {
        boundary: BoundaryPieceRef {
            domain: patch.domain,
            piece: edge.piece.clone(),
        },
        span: range,
        axial: AxialMap {
            offset: first - derivative * range.first,
            scale: derivative,
        },
    })
}

fn pair(
    state: &mut SheetState,
    first: ShapePortSide,
    second: ShapePortSide,
    width: f64,
) -> Result<(), String> {
    let coverage = |s: &ShapePortSide| {
        let a = s.axial.offset + s.axial.scale * s.span.first;
        let b = s.axial.offset + s.axial.scale * s.span.last;
        [a.min(b), a.max(b)]
    };
    for values in [coverage(&first), coverage(&second)] {
        if values[0].abs() > TOLERANCE || (values[1] - width).abs() > TOLERANCE {
            return Err("tangent ports do not cover the same full axial interval".into());
        }
    }
    let token = state
        .joins
        .iter()
        .map(|p| p.token)
        .max()
        .map_or(0, |v| v + 1);
    state.joins.push(ShapePort {
        token,
        axial_interval: ParamSpan {
            first: 0.0,
            last: width,
        },
        first,
        second,
    });
    Ok(())
}

struct Construction {
    cylinder: CylinderEmbedding,
    moving: SheetEmbedding,
}
fn construction(
    frame: &PortFrame,
    tangent: f64,
    axial_start: f64,
    metrics: &BendMetrics,
) -> Construction {
    let p = add(
        add(frame.origin, scale(frame.m, tangent)),
        scale(frame.e, axial_start),
    );
    let sign = metrics.angle.signum();
    let centre = add(
        p,
        scale(
            frame.n,
            if sign > 0.0 {
                metrics.radius + metrics.thickness
            } else {
                -metrics.radius
            },
        ),
    );
    let a = scale(frame.n, -sign);
    let theta = metrics.angle.abs();
    let radial = add(scale(a, theta.cos()), scale(frame.m, theta.sin()));
    let front_radius = if sign > 0.0 {
        metrics.radius + metrics.thickness
    } else {
        metrics.radius
    };
    let tangent_direction = add(
        scale(frame.m, theta.cos()),
        scale(frame.n, sign * theta.sin()),
    );
    Construction {
        cylinder: CylinderEmbedding {
            centre,
            axis: frame.e,
            radial_start: a,
            angular_direction: frame.m,
            inside_radius: metrics.radius,
            neutral_radius: metrics.radius + metrics.k * metrics.thickness,
            angle_extent: theta,
            inner_is_front: sign < 0.0,
        },
        moving: SheetEmbedding::Plane {
            front_frame: SketchFrame {
                origin: add(centre, scale(radial, front_radius)),
                x_axis: tangent_direction,
                y_axis: frame.e,
            },
            front_to_back_sign: MaterialParity::Negative,
        },
    }
}

fn relief_tool(
    feature: FeatureUid,
    slot: u32,
    relief: &Relief<f64>,
    tangent: f64,
    end: f64,
    upper: bool,
    outside: f64,
) -> Result<MaterialRegion, String> {
    if [relief.width, relief.depth]
        .iter()
        .any(|v| !v.is_finite() || *v <= TOLERANCE)
    {
        return Err("sheet endpoint relief requires positive width and depth".into());
    }
    let (v0, v1) = if upper {
        (end, end + relief.width)
    } else {
        (end - relief.width, end)
    };
    let back = tangent - relief.depth;
    match relief.kind {
        ReliefKind::Rectangular | ReliefKind::Tear => {
            rectangle(feature, slot, [back, v0], [outside, v1])
        }
        ReliefKind::Round => {
            let radius = relief.width / 2.0;
            if relief.depth < radius + TOLERANCE {
                return Err("round sheet relief depth must include its semicircular tip".into());
            }
            let centre = [back + radius, (v0 + v1) / 2.0];
            region(
                feature,
                slot,
                vec![
                    SegmentGeometry::Line {
                        start: [outside, v0],
                        end: [outside, v1],
                    },
                    SegmentGeometry::Line {
                        start: [outside, v1],
                        end: [centre[0], v1],
                    },
                    SegmentGeometry::Arc {
                        center: centre,
                        radius,
                        start_angle: std::f64::consts::FRAC_PI_2,
                        end_angle: 3.0 * std::f64::consts::FRAC_PI_2,
                    },
                    SegmentGeometry::Line {
                        start: [centre[0], v0],
                        end: [outside, v0],
                    },
                ],
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_strip<K: Kernel>(
    ctx: &EvalContext<'_, K>,
    state: &mut SheetState,
    metrics: &BendMetrics,
    construction: Construction,
    moving: SheetPatch,
    stationary: ShapePortSide,
    moving_side: ShapePortSide,
    width: f64,
    strip_regions: Option<Vec<MaterialRegion>>,
) -> Result<(), String> {
    let id = BendId {
        feature: ctx.uid,
        slot: 0,
    };
    let strip = SheetPatch {
        domain: SheetDomainRef::Bend(id),
        regions: strip_regions.unwrap_or(vec![rectangle(
            ctx.uid,
            40,
            [0.0, 0.0],
            [metrics.allowance, width],
        )?]),
        embedding: SheetEmbedding::Cylinder(construction.cylinder.clone()),
    };
    let left = side(&strip, &generated(ctx.uid, 43), Isometry2::IDENTITY)?;
    let right = side(&strip, &generated(ctx.uid, 41), Isometry2::IDENTITY)?;
    state.bends.push(BendDomain {
        id,
        stationary: stationary.boundary.clone(),
        moving: moving_side.boundary.clone(),
        folded_embedding: construction.cylinder,
        signed_angle: metrics.angle,
        k_factor: metrics.k,
        radius: metrics.radius,
        allowance: metrics.allowance,
        axial_interval: ParamSpan {
            first: 0.0,
            last: width,
        },
        rules: state.rules,
        rules_version: state.rules_version.clone(),
    });
    pair(state, stationary, left, width)?;
    pair(state, right, moving_side, width)?;
    state.charts.push(strip);
    state.charts.push(moving);
    Ok(())
}

pub(crate) fn flange<K: Kernel>(
    d: &SheetFlangeDef<f64>,
    ctx: &mut EvalContext<'_, K>,
) -> Result<FeatureOutput<K::Shape>, String> {
    let (mut state, rules) = read(ctx, d.body)?;
    free(&state, &d.boundary)?;
    let metrics = BendMetrics::new(d.angle, d.radius, &rules)?;
    let length = d.length.value
        - match d.length.mode {
            SheetLengthMode::Tangent => 0.0,
            SheetLengthMode::Inner => metrics.inner,
            SheetLengthMode::Outer => metrics.outer,
        };
    if !length.is_finite() || length <= TOLERANCE {
        return Err("sheet flange length is shorter than its requested setback".into());
    }
    let frame = port_frame(&state, &d.boundary)?;
    let (start, end) = d.offsets.as_ref().map_or((0.0, 0.0), |o| (o.start, o.end));
    if [start, end].iter().any(|v| !v.is_finite() || *v < 0.0)
        || start + end >= frame.width - TOLERANCE
    {
        return Err("sheet flange offsets leave no positive axial width".into());
    }
    let width = frame.width - start - end;
    let tangent = metrics.offset(d.position);
    let (min, max) = extent(chart(&state, d.boundary.panel)?, frame.to_operation)?;
    let far = max[0] + margin(min, max);
    let mut tools = vec![rectangle(
        ctx.uid,
        10,
        [tangent, start],
        [far, frame.width - end],
    )?];
    let relief = d.relief.as_ref().unwrap_or(&rules.bend_relief);
    if (start > TOLERANCE || end > TOLERANCE) && relief.depth < rules.thickness - TOLERANCE {
        return Err(
            "sheet endpoint relief must extend at least one thickness behind the tangent".into(),
        );
    }
    if start > TOLERANCE {
        tools.push(relief_tool(
            ctx.uid, 20, relief, tangent, start, false, far,
        )?);
    }
    if end > TOLERANCE {
        tools.push(relief_tool(
            ctx.uid,
            30,
            relief,
            tangent,
            frame.width - end,
            true,
            far,
        )?);
    }
    trim(
        ctx,
        &mut state,
        &[SheetDomainRef::Panel(d.boundary.panel)],
        SheetTrimOperation::Difference,
        &tools,
        &[frame.to_operation],
    )?;
    let mut axial = frame.to_operation;
    axial.origin[1] -= start;
    let stationary = side(
        chart(&state, d.boundary.panel)?,
        &generated(ctx.uid, 13),
        axial,
    )?;
    let constructed = construction(&frame, tangent, start, &metrics);
    let moving = SheetPatch {
        domain: SheetDomainRef::Panel(PanelId {
            feature: ctx.uid,
            slot: 0,
        }),
        regions: vec![rectangle(ctx.uid, 50, [0.0, 0.0], [length, width])?],
        embedding: constructed.moving.clone(),
    };
    let moving_side = side(&moving, &generated(ctx.uid, 53), Isometry2::IDENTITY)?;
    add_strip(
        ctx,
        &mut state,
        &metrics,
        constructed,
        moving,
        stationary,
        moving_side,
        width,
        None,
    )?;
    publish(ctx, state, &rules, None)
}

pub(crate) fn bend<K: Kernel>(
    d: &SheetBendDef<f64>,
    ctx: &mut EvalContext<'_, K>,
) -> Result<FeatureOutput<K::Shape>, String> {
    let (mut state, rules) = read(ctx, d.body)?;
    let metrics = BendMetrics::new(d.angle, d.radius, &rules)?;
    let moving_body = d.moving_body.unwrap_or(d.body);
    if moving_body == d.body {
        if d.resolved_mode() != BendPortMode::VirtualSharp {
            return Err("same-body sheet bend requires a virtual-sharp seam".into());
        }
        virtual_bend(ctx, &mut state, &rules, &metrics, &d.stationary, &d.moving)?;
        publish(ctx, state, &rules, None)
    } else {
        let (moving, moving_rules) = read(ctx, moving_body)?;
        if d.resolved_mode() != BendPortMode::TangentPorts {
            return Err("separate sheet bodies require tangent-port mode".into());
        }
        if moving.owner != state.owner || moving.rules != state.rules || moving_rules != rules {
            return Err("sheet Bend bodies must have the same owner and rule reference".into());
        }
        positioned_bend(
            ctx,
            &mut state,
            moving,
            &rules,
            &metrics,
            &d.stationary,
            &d.moving,
        )?;
        publish(ctx, state, &rules, Some(moving_body))
    }
}

pub(crate) fn fold<K: Kernel>(
    d: &SheetFoldDef<f64>,
    ctx: &mut EvalContext<'_, K>,
) -> Result<FeatureOutput<K::Shape>, String> {
    let (mut state, rules) = read(ctx, d.body)?;
    let metrics = BendMetrics::new(d.angle, d.radius, &rules)?;
    let sketch = ctx.sketch(d.line.sketch)?;
    let crate::sketch::geometry::Curve2::Line { a, b } =
        crate::features::sketch::curve_of(&sketch.solved, d.line.curve)
            .ok_or("fold sketch line is unavailable")?
    else {
        return Err("sheet Fold requires a straight sketch line".into());
    };
    let patch = chart(&state, d.panel)?.clone();
    let (frame, parity) = plane(&patch)?;
    let reference = state
        .panels
        .iter()
        .find(|p| p.id == d.panel)
        .map_or(frame, |p| p.frame);
    let world_a = sketch.frame.point(*a);
    let world_b = sketch.frame.point(*b);
    if [world_a, world_b]
        .iter()
        .any(|p| dot(sub(*p, reference.origin), reference.normal()).abs() > TOLERANCE)
    {
        return Err("fold line is not in the selected panel reference plane".into());
    }
    let project = |p: [f64; 3]| {
        let delta = sub(p, frame.origin);
        [dot(delta, frame.x_axis), dot(delta, frame.y_axis)]
    };
    let a = project(world_a);
    let b = project(world_b);
    let along = g2::unit(g2::sub(b, a));
    let m = if d.stationary_side == StationarySide::Left {
        [along[1], -along[0]]
    } else {
        [-along[1], along[0]]
    };
    let e = g2::scale(g2::perp(m), -parity.sign());
    let start = if g2::dot(g2::sub(b, a), e) > 0.0 {
        a
    } else {
        b
    };
    let port = PortFrame {
        origin: frame.point(start),
        m: add(scale(frame.x_axis, m[0]), scale(frame.y_axis, m[1])),
        e: add(scale(frame.x_axis, e[0]), scale(frame.y_axis, e[1])),
        n: scale(frame.normal(), parity.sign()),
        to_operation: Isometry2 {
            origin: [-g2::dot(start, m), -g2::dot(start, e)],
            x_axis: [m[0], e[0]],
            y_axis: [m[1], e[1]],
        },
        width: g2::dist(a, b),
    };
    fold_panel(
        ctx,
        &mut state,
        &rules,
        &metrics,
        d.panel,
        &port,
        d.position,
        d.relief.as_ref().unwrap_or(&rules.bend_relief),
    )?;
    publish(ctx, state, &rules, None)
}

// Implementations below preserve semantic chart identity while moving only
// the graph branch on the moving side of the selected seam.

fn branch(
    state: &SheetState,
    start: SheetDomainRef,
    fixed: SheetDomainRef,
    skip_pair: bool,
) -> Result<BTreeSet<SheetDomainRef>, String> {
    let mut reached = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(domain) = pending.pop() {
        if !reached.insert(domain) {
            continue;
        }
        if domain == fixed {
            return Err("sheet graph has a second path to the stationary panel".into());
        }
        for port in &state.joins {
            let a = port.first.boundary.domain;
            let b = port.second.boundary.domain;
            if skip_pair && ((a == start && b == fixed) || (b == start && a == fixed)) {
                continue;
            }
            if a == domain {
                pending.push(b);
            } else if b == domain {
                pending.push(a);
            }
        }
    }
    Ok(reached)
}

fn move_branch(state: &mut SheetState, domains: &BTreeSet<SheetDomainRef>, transform: &Transform) {
    for p in &mut state.charts {
        if domains.contains(&p.domain) {
            p.embedding = p.embedding.transformed(transform);
        }
    }
    for b in &mut state.bends {
        if domains.contains(&SheetDomainRef::Bend(b.id))
            && let SheetEmbedding::Cylinder(c) =
                SheetEmbedding::Cylinder(b.folded_embedding.clone()).transformed(transform)
        {
            b.folded_embedding = c;
        }
    }
}

fn moving_transform(
    frame: &PortFrame,
    tangent: f64,
    axial: f64,
    construction: &Construction,
) -> Transform {
    let source = SketchFrame {
        origin: add(
            add(frame.origin, scale(frame.m, tangent)),
            scale(frame.e, axial),
        ),
        x_axis: frame.m,
        y_axis: frame.e,
    };
    let SheetEmbedding::Plane {
        front_frame: target,
        ..
    } = &construction.moving
    else {
        unreachable!()
    };
    Transform::between_frames(&source, target)
}

#[allow(clippy::too_many_arguments)]
fn fold_panel<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    state: &mut SheetState,
    rules: &SheetRulesDef<f64>,
    metrics: &BendMetrics,
    panel: PanelId,
    frame: &PortFrame,
    position: SheetPosition,
    relief: &Relief<f64>,
) -> Result<(), String> {
    let old = chart(state, panel)?.clone();
    let stationary_domain = old.domain;
    let moving_domain = SheetDomainRef::Panel(PanelId {
        feature: ctx.uid,
        slot: 0,
    });
    let strip_domain = SheetDomainRef::Bend(BendId {
        feature: ctx.uid,
        slot: 0,
    });
    let tangent = metrics.offset(position);
    let far_tangent = tangent + metrics.allowance;
    let (min, max) = extent(&old, frame.to_operation)?;
    let pad = margin(min, max);
    let outside = max[0] + pad;
    if min[1] < -TOLERANCE || max[1] > frame.width + TOLERANCE {
        if !relief.depth.is_finite() || relief.depth < rules.thickness - TOLERANCE {
            return Err(
                "sheet endpoint relief must extend at least one thickness behind the tangent"
                    .into(),
            );
        }
        if !relief.width.is_finite() || relief.width <= TOLERANCE {
            return Err("sheet endpoint relief requires a positive explicit clearance".into());
        }
        if relief.kind == ReliefKind::Round && relief.depth < relief.width / 2.0 + TOLERANCE {
            return Err("round sheet relief depth must include its semicircular tip".into());
        }
    }
    let mut moving = old.clone();
    moving.domain = moving_domain;
    let mut strip = old.clone();
    strip.domain = strip_domain;
    copy_lineage(
        &mut state.lineage,
        stationary_domain,
        moving_domain,
        &old.regions,
    );
    copy_lineage(
        &mut state.lineage,
        stationary_domain,
        strip_domain,
        &old.regions,
    );
    state.charts.extend([moving, strip]);
    let moving_tool = rectangle(ctx.uid, 50, [far_tangent, 0.0], [outside, frame.width])?;
    trim(
        ctx,
        state,
        &[moving_domain],
        SheetTrimOperation::Intersection,
        &[moving_tool],
        &[frame.to_operation],
    )?;
    let strip_tool = rectangle(ctx.uid, 40, [tangent, 0.0], [far_tangent, frame.width])?;
    trim(
        ctx,
        state,
        &[strip_domain],
        SheetTrimOperation::Intersection,
        &[strip_tool],
        &[frame.to_operation],
    )?;
    let mut removal = vec![rectangle(
        ctx.uid,
        10,
        [tangent, 0.0],
        [outside, frame.width],
    )?];
    if min[1] < -TOLERANCE {
        removal.push(relief_tool(
            ctx.uid, 20, relief, tangent, 0.0, false, outside,
        )?);
    }
    if max[1] > frame.width + TOLERANCE {
        removal.push(relief_tool(
            ctx.uid,
            30,
            relief,
            tangent,
            frame.width,
            true,
            outside,
        )?);
    }
    trim(
        ctx,
        state,
        &[stationary_domain],
        SheetTrimOperation::Difference,
        &removal,
        &[frame.to_operation],
    )?;
    let moving = state
        .charts
        .iter()
        .find(|p| p.domain == moving_domain)
        .expect("moving domain")
        .clone();
    let strip = state
        .charts
        .iter()
        .find(|p| p.domain == strip_domain)
        .expect("bend domain")
        .clone();
    let mut map = frame.to_operation;
    map.origin[0] -= tangent;
    let strip_regions = mapped_regions(&strip.regions, map);
    remap_lineage(&mut state.lineage, strip_domain, &strip.regions, map);
    let stationary = side(
        chart(state, panel)?,
        &generated(ctx.uid, 13),
        frame.to_operation,
    )?;
    let moving_side = side(&moving, &generated(ctx.uid, 53), frame.to_operation)?;
    let constructed = construction(frame, tangent, 0.0, metrics);
    let transform = moving_transform(frame, far_tangent, 0.0, &constructed);
    // Classify existing ports using their original material boundaries. A
    // port crossing the new strip cannot be silently assigned to one branch.
    let mut moving_neighbors = Vec::new();
    for port in &mut state.joins {
        for (first, second) in [(&mut port.first, &mut port.second)] {
            let (own, other) = if first.boundary.domain == stationary_domain {
                (first, second)
            } else if second.boundary.domain == stationary_domain {
                (second, first)
            } else {
                continue;
            };
            let edges: Vec<_> = old
                .regions
                .iter()
                .flat_map(|r| &r.loops)
                .flat_map(|l| &l.edges)
                .filter(|e| e.piece == own.boundary.piece)
                .collect();
            if edges.len() != 1 {
                return Err("existing sheet port has ambiguous boundary provenance".into());
            }
            let (a, b) = line(edges[0])?;
            let a = frame.to_operation.point(a);
            let b = frame.to_operation.point(b);
            if a[0] >= far_tangent - TOLERANCE
                && b[0] >= far_tangent - TOLERANCE
                && a[1] >= -TOLERANCE
                && b[1] >= -TOLERANCE
                && a[1] <= frame.width + TOLERANCE
                && b[1] <= frame.width + TOLERANCE
            {
                own.boundary.domain = moving_domain;
                moving_neighbors.push(other.boundary.domain);
            } else if a[0] > tangent + TOLERANCE || b[0] > tangent + TOLERANCE {
                return Err("fold strip crosses an existing tangent port".into());
            }
        }
    }
    let mut moved = BTreeSet::new();
    for neighbor in moving_neighbors {
        moved.extend(branch(state, neighbor, stationary_domain, false)?);
    }
    moved.insert(moving_domain);
    move_branch(state, &moved, &transform);
    for bend in &mut state.bends {
        for boundary in [&mut bend.stationary, &mut bend.moving] {
            if boundary.domain == stationary_domain
                && moving
                    .regions
                    .iter()
                    .flat_map(|r| &r.loops)
                    .flat_map(|l| &l.edges)
                    .any(|e| e.piece == boundary.piece)
            {
                boundary.domain = moving_domain;
            }
        }
    }
    let moving = state
        .charts
        .iter()
        .find(|p| p.domain == moving_domain)
        .expect("moved panel")
        .clone();
    state
        .charts
        .retain(|p| p.domain != moving_domain && p.domain != strip_domain);
    add_strip(
        ctx,
        state,
        metrics,
        constructed,
        moving,
        stationary,
        moving_side,
        frame.width,
        Some(strip_regions),
    )?;
    let _ = rules;
    Ok(())
}

fn virtual_bend<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    state: &mut SheetState,
    _rules: &SheetRulesDef<f64>,
    metrics: &BendMetrics,
    stationary_ref: &BoundaryRef,
    moving_ref: &BoundaryRef,
) -> Result<(), String> {
    if stationary_ref.panel == moving_ref.panel {
        return Err("sheet Bend needs two distinct labelled panel domains".into());
    }
    let stationary_old = chart(state, stationary_ref.panel)?.clone();
    let moving_old = chart(state, moving_ref.panel)?.clone();
    let frame = port_frame(state, stationary_ref)?;
    let moving_edge = boundary(state, moving_ref)?.clone();
    let (a, b) = line(&moving_edge)?;
    let (moving_frame, moving_parity) = plane(&moving_old)?;
    let end = add(frame.origin, scale(frame.e, frame.width));
    let a = moving_frame.point(a);
    let b = moving_frame.point(b);
    if !((distance(a, frame.origin) <= TOLERANCE && distance(b, end) <= TOLERANCE)
        || (distance(b, frame.origin) <= TOLERANCE && distance(a, end) <= TOLERANCE))
        || dot(scale(moving_frame.normal(), moving_parity.sign()), frame.n) < 1.0 - 1e-9
    {
        return Err("virtual-sharp Bend requires a full shared coplanar material seam".into());
    }
    let fixed = stationary_old.domain;
    let moving_domain = moving_old.domain;
    if state
        .joins
        .iter()
        .filter(|p| {
            (p.first.boundary.domain == fixed && p.second.boundary.domain == moving_domain)
                || (p.second.boundary.domain == fixed && p.first.boundary.domain == moving_domain)
        })
        .count()
        > 1
    {
        return Err("sheet graph has multiple seams between the selected panels".into());
    }
    let moved = branch(state, moving_domain, fixed, true)?;
    state.joins.retain(|p| {
        !((p.first.boundary.domain == fixed && p.second.boundary.domain == moving_domain)
            || (p.second.boundary.domain == fixed && p.first.boundary.domain == moving_domain))
    });
    let moving_map = Isometry2 {
        origin: [
            dot(sub(moving_frame.origin, frame.origin), frame.m),
            dot(sub(moving_frame.origin, frame.origin), frame.e),
        ],
        x_axis: [
            dot(moving_frame.x_axis, frame.m),
            dot(moving_frame.x_axis, frame.e),
        ],
        y_axis: [
            dot(moving_frame.y_axis, frame.m),
            dot(moving_frame.y_axis, frame.e),
        ],
    };
    let tangent = -metrics.allowance / 2.0;
    let far_tangent = metrics.allowance / 2.0;
    let (smin, smax) = extent(&stationary_old, frame.to_operation)?;
    let (mmin, mmax) = extent(&moving_old, moving_map)?;
    let pad = margin(smin, smax) + margin(mmin, mmax);
    let far = smax[0].max(mmax[0]) + pad;
    let strip_domain = SheetDomainRef::Bend(BendId {
        feature: ctx.uid,
        slot: 0,
    });
    let mut half = stationary_old.clone();
    half.domain = strip_domain;
    state.charts.push(half);
    copy_lineage(
        &mut state.lineage,
        fixed,
        strip_domain,
        &stationary_old.regions,
    );
    let strip_mask = rectangle(ctx.uid, 40, [tangent, 0.0], [far_tangent, frame.width])?;
    trim(
        ctx,
        state,
        &[strip_domain],
        SheetTrimOperation::Intersection,
        std::slice::from_ref(&strip_mask),
        &[frame.to_operation],
    )?;
    let mut first = state
        .charts
        .iter()
        .find(|p| p.domain == strip_domain)
        .expect("strip")
        .clone();
    let mut smap = frame.to_operation;
    smap.origin[0] -= tangent;
    remap_lineage(&mut state.lineage, strip_domain, &first.regions, smap);
    first.regions = mapped_regions(&first.regions, smap);
    state.charts.retain(|p| p.domain != strip_domain);
    let mut half = moving_old.clone();
    half.domain = strip_domain;
    state.charts.push(half);
    copy_lineage(
        &mut state.lineage,
        moving_domain,
        strip_domain,
        &moving_old.regions,
    );
    trim(
        ctx,
        state,
        &[strip_domain],
        SheetTrimOperation::Intersection,
        &[strip_mask],
        &[moving_map],
    )?;
    let half = state
        .charts
        .iter()
        .find(|p| p.domain == strip_domain)
        .expect("strip")
        .clone();
    let mut mmap = moving_map;
    mmap.origin[0] -= tangent;
    let tools = mapped_regions(&half.regions, mmap);
    remap_lineage(&mut state.lineage, strip_domain, &half.regions, mmap);
    state.charts.retain(|p| p.domain != strip_domain);
    state.charts.push(first);
    trim_with_lineage(
        ctx,
        state,
        &[strip_domain],
        SheetTrimOperation::Union,
        &tools,
        &[Isometry2::IDENTITY],
        Some(strip_domain),
    )?;
    let strip_regions = state
        .charts
        .iter()
        .find(|p| p.domain == strip_domain)
        .expect("strip")
        .regions
        .clone();
    state.charts.retain(|p| p.domain != strip_domain);
    trim(
        ctx,
        state,
        &[fixed],
        SheetTrimOperation::Difference,
        &[rectangle(
            ctx.uid,
            10,
            [tangent, -pad],
            [far, frame.width + pad],
        )?],
        &[frame.to_operation],
    )?;
    trim(
        ctx,
        state,
        &[moving_domain],
        SheetTrimOperation::Intersection,
        &[rectangle(
            ctx.uid,
            50,
            [far_tangent, -pad],
            [far, frame.width + pad],
        )?],
        &[moving_map],
    )?;
    let stationary = side(
        chart(state, stationary_ref.panel)?,
        &generated(ctx.uid, 13),
        frame.to_operation,
    )?;
    let moving_side = side(
        chart(state, moving_ref.panel)?,
        &generated(ctx.uid, 53),
        moving_map,
    )?;
    let constructed = construction(&frame, tangent, 0.0, metrics);
    let transform = moving_transform(&frame, far_tangent, 0.0, &constructed);
    move_branch(state, &moved, &transform);
    let moving = chart(state, moving_ref.panel)?.clone();
    state.charts.retain(|p| p.domain != moving_domain);
    add_strip(
        ctx,
        state,
        metrics,
        constructed,
        moving,
        stationary,
        moving_side,
        frame.width,
        Some(strip_regions),
    )
}

fn positioned_bend<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    state: &mut SheetState,
    mut moving: SheetState,
    _rules: &SheetRulesDef<f64>,
    metrics: &BendMetrics,
    stationary_ref: &BoundaryRef,
    moving_ref: &BoundaryRef,
) -> Result<(), String> {
    free(state, stationary_ref)?;
    free(&moving, moving_ref)?;
    let frame = port_frame(state, stationary_ref)?;
    let constructed = construction(&frame, 0.0, 0.0, metrics);
    let moving_patch = chart(&moving, moving_ref.panel)?.clone();
    let (moving_frame, parity) = plane(&moving_patch)?;
    let edge = boundary(&moving, moving_ref)?.clone();
    let (a, b) = line(&edge)?;
    let a = moving_frame.point(a);
    let b = moving_frame.point(b);
    let SheetEmbedding::Plane {
        front_frame: expected,
        front_to_back_sign,
    } = &constructed.moving
    else {
        unreachable!()
    };
    let end = add(expected.origin, scale(frame.e, frame.width));
    if !((distance(a, expected.origin) <= TOLERANCE && distance(b, end) <= TOLERANCE)
        || (distance(b, expected.origin) <= TOLERANCE && distance(a, end) <= TOLERANCE))
        || dot(
            scale(moving_frame.normal(), parity.sign()),
            scale(expected.normal(), front_to_back_sign.sign()),
        ) < 1.0 - 1e-9
    {
        return Err("positioned sheet tangent ports do not match the requested radius, angle and material sides".into());
    }
    let stationary = side(
        chart(state, stationary_ref.panel)?,
        &boundary(state, stationary_ref)?.piece,
        frame.to_operation,
    )?;
    let moving_axial = Isometry2 {
        origin: [0.0, dot(sub(moving_frame.origin, expected.origin), frame.e)],
        x_axis: [0.0, dot(moving_frame.x_axis, frame.e)],
        y_axis: [0.0, dot(moving_frame.y_axis, frame.e)],
    };
    let moving_side = side(&moving_patch, &edge.piece, moving_axial)?;
    for port in &mut moving.joins {
        port.token = state
            .joins
            .iter()
            .map(|p| p.token)
            .max()
            .map_or(0, |v| v + 1);
        state.joins.push(port.clone());
    }
    state.bends.extend(moving.bends);
    state.opened.extend(moving.opened);
    state.lineage.extend(moving.lineage);
    state.charts.extend(
        moving
            .charts
            .into_iter()
            .filter(|p| p.domain != moving_patch.domain),
    );
    add_strip(
        ctx,
        state,
        metrics,
        constructed,
        moving_patch,
        stationary,
        moving_side,
        frame.width,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ComponentUid;

    fn empty_state() -> SheetState {
        SheetState {
            schema: 2,
            algorithm: ALGORITHM_VERSION,
            body: BodyUid::new(FeatureUid(2), 0),
            owner: ComponentUid::ROOT,
            shape_generation: String::new(),
            rules: FeatureUid(1),
            rules_version: String::new(),
            panels: Vec::new(),
            charts: Vec::new(),
            bends: Vec::new(),
            joins: Vec::new(),
            opened: Vec::new(),
            lineage: Vec::new(),
            invalidated: None,
            blockers: Vec::new(),
        }
    }
    fn domain(slot: u32) -> SheetDomainRef {
        SheetDomainRef::Panel(PanelId {
            feature: FeatureUid(2),
            slot,
        })
    }
    fn link(a: SheetDomainRef, b: SheetDomainRef, token: u32) -> ShapePort {
        let side = |domain| ShapePortSide {
            boundary: BoundaryPieceRef {
                domain,
                piece: generated(FeatureUid(2), token),
            },
            span: ParamSpan {
                first: 0.0,
                last: 1.0,
            },
            axial: AxialMap {
                offset: 0.0,
                scale: 20.0,
            },
        };
        ShapePort {
            token,
            axial_interval: ParamSpan {
                first: 0.0,
                last: 20.0,
            },
            first: side(a),
            second: side(b),
        }
    }

    #[test]
    fn moving_branch_preserves_connected_bends_but_rejects_an_alternate_stationary_path() {
        let mut state = empty_state();
        state.joins = vec![link(domain(0), domain(1), 0), link(domain(1), domain(2), 1)];
        assert_eq!(
            branch(&state, domain(1), domain(0), true).unwrap(),
            BTreeSet::from([domain(1), domain(2)])
        );
        state.joins.push(link(domain(2), domain(0), 2));
        assert!(
            branch(&state, domain(1), domain(0), true)
                .unwrap_err()
                .contains("second path")
        );
    }

    #[test]
    fn back_reference_l_keeps_entered_material_side_for_both_angle_signs() {
        let frame = PortFrame {
            origin: [0.0, 20.0, -2.0],
            m: [1.0, 0.0, 0.0],
            e: [0.0, -1.0, 0.0],
            n: [0.0, 0.0, 1.0],
            to_operation: Isometry2::IDENTITY,
            width: 20.0,
        };
        for (sign, centre, endpoint, direction, inner_is_front) in [
            (
                1.0,
                [0.0, 20.0, 3.0],
                [5.0, 20.0, 3.0],
                [0.0, 0.0, 1.0],
                false,
            ),
            (
                -1.0,
                [0.0, 20.0, -5.0],
                [3.0, 20.0, -5.0],
                [0.0, 0.0, -1.0],
                true,
            ),
        ] {
            let metrics = BendMetrics {
                angle: sign * std::f64::consts::FRAC_PI_2,
                radius: 3.0,
                thickness: 2.0,
                k: 0.3,
                allowance: 1.8 * std::f64::consts::PI,
                inner: 3.0,
                outer: 5.0,
            };
            let result = construction(&frame, 0.0, 0.0, &metrics);
            assert_eq!(result.cylinder.centre, centre);
            assert_eq!(result.cylinder.inner_is_front, inner_is_front);
            assert!((result.cylinder.neutral_radius - 3.6).abs() < 1e-12);
            let SheetEmbedding::Plane {
                front_frame,
                front_to_back_sign,
            } = result.moving
            else {
                panic!("moving plane");
            };
            assert!(distance(front_frame.origin, endpoint) < 1e-12);
            assert!(distance(front_frame.x_axis, direction) < 1e-12);
            assert_eq!(front_to_back_sign, MaterialParity::Negative);
        }
    }

    #[test]
    fn tear_uses_exact_entered_gap_and_round_depth_includes_its_tip() {
        let tear = relief_tool(
            FeatureUid(8),
            100,
            &Relief {
                kind: ReliefKind::Tear,
                width: 2.0,
                depth: 3.0,
            },
            -5.0,
            3.0,
            false,
            40.0,
        )
        .unwrap();
        let p = SheetPatch {
            domain: domain(0),
            regions: vec![tear],
            embedding: SheetEmbedding::Plane {
                front_frame: SketchFrame::XY,
                front_to_back_sign: MaterialParity::Positive,
            },
        };
        assert_eq!(bounds(&p).unwrap(), ([-8.0, 1.0], [40.0, 3.0]));
        let round = relief_tool(
            FeatureUid(8),
            200,
            &Relief {
                kind: ReliefKind::Round,
                width: 2.0,
                depth: 3.0,
            },
            -5.0,
            3.0,
            false,
            40.0,
        )
        .unwrap();
        assert!(round.loops[0].edges.iter().any(|e|matches!(e.curve.geometry,SegmentGeometry::Arc{center:[x,y],radius,..} if (x+7.0).abs()<1e-12&&(y-2.0).abs()<1e-12&&(radius-1.0).abs()<1e-12)));
        assert!(
            relief_tool(
                FeatureUid(8),
                200,
                &Relief {
                    kind: ReliefKind::Round,
                    width: 4.0,
                    depth: 1.0
                },
                -5.0,
                3.0,
                false,
                40.0
            )
            .is_err()
        );
    }

    #[test]
    fn face_registers_named_coplanar_adjacency_without_cap_topology() {
        let patches: Vec<_> = (0..2)
            .map(|slot| SheetPatch {
                domain: domain(slot),
                regions: vec![
                    rectangle(
                        FeatureUid(10 + u64::from(slot)),
                        0,
                        [10.0 * f64::from(slot), 0.0],
                        [10.0 * f64::from(slot + 1), 20.0],
                    )
                    .unwrap(),
                ],
                embedding: SheetEmbedding::Plane {
                    front_frame: SketchFrame::XY,
                    front_to_back_sign: MaterialParity::Positive,
                },
            })
            .collect();
        let joins = crate::sheet::planar_joins(&patches).unwrap();
        assert_eq!(joins.len(), 1);
        assert_eq!(joins[0].first.boundary.domain, domain(0));
        assert_eq!(joins[0].second.boundary.domain, domain(1));
        assert_eq!(
            joins[0].axial_interval,
            ParamSpan {
                first: 0.0,
                last: 20.0
            }
        );
        assert_ne!(
            joins[0].first.boundary.piece.curve,
            joins[0].second.boundary.piece.curve
        );
    }

    #[test]
    fn coincident_alias_cannot_escape_its_proven_fragment_with_an_explicit_range() {
        let mut state = empty_state();
        let piece = generated(FeatureUid(2), 10);
        let alias = generated(FeatureUid(3), 20);
        let first = MaterialEdge {
            piece: piece.clone(),
            curve: BoundedCurve2 {
                geometry: SegmentGeometry::Line {
                    start: [0.0, 0.0],
                    end: [1.0, 0.0],
                },
                range: ParamSpan {
                    first: 0.0,
                    last: 1.0,
                },
            },
        };
        let second = MaterialEdge {
            piece: piece.clone(),
            curve: BoundedCurve2 {
                geometry: SegmentGeometry::Line {
                    start: [0.0, 1.0],
                    end: [1.0, 1.0],
                },
                range: ParamSpan {
                    first: 2.0,
                    last: 3.0,
                },
            },
        };
        state.lineage.push(CurveLineage {
            boundary: BoundaryPieceRef {
                domain: domain(0),
                piece,
            },
            curve_generation: CurveLineage::generation(&first.curve),
            aliases: vec![alias.clone()],
            sources: vec![SourceSpan {
                source: TrimSource::Tool {
                    piece: alias.clone(),
                },
                span: first.curve.range,
            }],
        });
        state.charts.push(SheetPatch {
            domain: domain(0),
            regions: vec![MaterialRegion {
                loops: vec![MaterialLoop {
                    edges: vec![first.clone(), second.clone()],
                }],
            }],
            embedding: SheetEmbedding::Plane {
                front_frame: SketchFrame::XY,
                front_to_back_sign: MaterialParity::Positive,
            },
        });
        let mut reference = BoundaryRef {
            panel: PanelId {
                feature: FeatureUid(2),
                slot: 0,
            },
            curve: alias.curve,
            piece: Some(0),
            range: Some(first.curve.range),
        };
        assert_eq!(boundary(&state, &reference).unwrap(), &first);
        reference.range = Some(second.curve.range);
        assert!(boundary(&state, &reference).is_err());
    }

    #[test]
    fn chart_remapping_and_successive_trims_preserve_only_proven_aliases() {
        let mut state = empty_state();
        let piece = generated(FeatureUid(2), 10);
        let mut alias = generated(FeatureUid(3), 20);
        alias.piece = 7;
        let edge = MaterialEdge {
            piece: piece.clone(),
            curve: BoundedCurve2 {
                geometry: SegmentGeometry::Line {
                    start: [2.0, 3.0],
                    end: [8.0, 3.0],
                },
                range: ParamSpan {
                    first: 0.0,
                    last: 1.0,
                },
            },
        };
        let regions = vec![MaterialRegion {
            loops: vec![MaterialLoop {
                edges: vec![edge.clone()],
            }],
        }];
        let sources = vec![SourceSpan {
            source: TrimSource::Tool {
                piece: alias.clone(),
            },
            span: ParamSpan {
                first: 4.0,
                last: 8.0,
            },
        }];
        state.lineage.push(CurveLineage {
            boundary: BoundaryPieceRef {
                domain: domain(0),
                piece: piece.clone(),
            },
            curve_generation: CurveLineage::generation(&edge.curve),
            aliases: vec![alias.clone()],
            sources: sources.clone(),
        });
        copy_lineage(&mut state.lineage, domain(0), domain(1), &regions);
        let map = Isometry2 {
            origin: [10.0, -5.0],
            x_axis: [-1.0, 0.0],
            y_axis: [0.0, 1.0],
        };
        remap_lineage(&mut state.lineage, domain(1), &regions, map);
        let mapped = mapped_regions(&regions, map);
        let mapped_edge = mapped[0].loops[0].edges[0].clone();
        assert_eq!(mapped_edge.curve.range, edge.curve.range.reversed());
        assert!(state.lineage[0].applies_to(domain(0), &edge));
        assert!(state.lineage[1].applies_to(domain(1), &mapped_edge));
        assert!(!state.lineage[1].applies_to(domain(1), &edge));
        assert_eq!(
            state.lineage[1].sources, sources,
            "chart remapping must not invent an ancestral parameter mapping"
        );
        state.charts.push(SheetPatch {
            domain: domain(1),
            regions: mapped.clone(),
            embedding: SheetEmbedding::Plane {
                front_frame: SketchFrame::XY,
                front_to_back_sign: MaterialParity::Positive,
            },
        });
        let reference = BoundaryRef {
            panel: PanelId {
                feature: FeatureUid(2),
                slot: 1,
            },
            curve: alias.curve.clone(),
            piece: Some(7),
            range: Some(mapped_edge.curve.range),
        };
        assert_eq!(boundary(&state, &reference).unwrap(), &mapped_edge);
        let inherited = inherited_aliases(
            &state.lineage,
            domain(1),
            &mapped,
            &piece,
            ParamSpan {
                first: 0.2,
                last: 0.8,
            },
        )
        .unwrap();
        assert!(inherited.contains(&alias));
        assert!(inherited.contains(&piece));
        assert!(
            inherited_aliases(
                &state.lineage,
                domain(1),
                &mapped,
                &piece,
                ParamSpan {
                    first: 2.0,
                    last: 3.0
                }
            )
            .is_err()
        );
    }
}
