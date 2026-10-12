// SPDX-License-Identifier: MIT
//! Thickness-normal sheet Cut across bends (mitcad#208, production form
//! mitcad#209).
//!
//! The named bends are developed while the fixed panel stays in place; every
//! chart in that panel's plane is trimmed by the tool regions in one
//! operation whose seams are the registered ports, so a contour crossing a
//! tangent line splits both sides of the seam at the same axial position.
//! Charts keep their coordinates, so restoring the input configuration only
//! restores the embeddings: the cut is folded with the material. A domain the
//! tools cover completely is removed with its ports and bend record; material
//! the cut disconnects becomes several bodies ([`super::publish_pieces`]).
//! Corner reliefs ([`super::relief`]) are the same operation with generated
//! tool contours.

use super::develop::{bend_record, domain_label, loop_area, planar_map, reconfigure};
use super::*;
use crate::features::sheet_cut::SheetCutDef;
use crate::profile::dot;
use std::collections::BTreeMap;

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] - b[i])
}

/// A trimmed edge before its identity is settled.
struct Pending {
    token: u32,
    edge: MaterialEdge,
    record: CurveLineage,
}

/// What [`settle`] installed.
pub(crate) struct Settled {
    /// The piece of each result edge token, for the trim's seam ports.
    pub tokens: BTreeMap<(SheetDomainRef, u32), CurvePieceRef>,
    /// Domains the tools covered completely; their charts are removed.
    pub removed: BTreeSet<SheetDomainRef>,
    /// Whether any boundary of the result comes from a tool.
    pub used_tools: bool,
}

/// Replaces the regions of every trimmed domain, keeps each fragment's proven
/// aliases, numbers fragments of one curve so every boundary piece of a chart
/// is unique, and keeps exteriors counter-clockwise. A domain without
/// material left is removed from the charts and reported.
pub(crate) fn settle(
    state: &mut SheetState,
    inputs: &[TrimDomain],
    result: &SheetTrimResult,
) -> Result<Settled, String> {
    if result.domains.len() != inputs.len() {
        return Err("sheet trim omitted a requested material domain".into());
    }
    let input_lineage = state.lineage.clone();
    let mut settled = Settled {
        tokens: BTreeMap::new(),
        removed: BTreeSet::new(),
        used_tools: false,
    };
    for output in &result.domains {
        let input = inputs
            .iter()
            .find(|d| d.domain == output.domain)
            .ok_or("sheet trim returned ambiguous domain membership")?;
        if output.regions.is_empty() {
            settled.removed.insert(output.domain);
            continue;
        }
        let mut regions: Vec<Vec<Vec<Pending>>> = Vec::new();
        for r in &output.regions {
            let mut loops = Vec::new();
            for l in &r.loops {
                let mut edges = Vec::new();
                for edge in &l.edges {
                    if edge.lineage.is_empty() {
                        return Err("sheet trim returned a boundary without provenance".into());
                    }
                    let mut aliases: Vec<_> = edge
                        .lineage
                        .iter()
                        .map(|s| match &s.source {
                            TrimSource::Domain { boundary } => boundary.piece.clone(),
                            TrimSource::Tool { piece } => {
                                settled.used_tools = true;
                                piece.clone()
                            }
                        })
                        .collect();
                    canonical_aliases(&mut aliases);
                    let piece = aliases[0].clone();
                    for source in &edge.lineage {
                        if let TrimSource::Domain { boundary } = &source.source {
                            aliases.extend(inherited_aliases(
                                &input_lineage,
                                boundary.domain,
                                &input.regions,
                                &boundary.piece,
                                source.span,
                            )?);
                        }
                    }
                    canonical_aliases(&mut aliases);
                    edges.push(Pending {
                        token: edge.token,
                        edge: MaterialEdge {
                            piece: piece.clone(),
                            curve: edge.curve.clone(),
                        },
                        record: CurveLineage {
                            boundary: BoundaryPieceRef {
                                domain: output.domain,
                                piece,
                            },
                            curve_generation: String::new(),
                            aliases,
                            sources: edge.lineage.clone(),
                        },
                    });
                }
                loops.push(edges);
            }
            regions.push(loops);
        }
        // Fragments of one curve get distinct piece numbers, ordered by their
        // midpoints: a reference to the curve alone is then ambiguous, as it
        // must be. (Lines come back with their own normalized parameters.)
        #[allow(clippy::type_complexity)]
        let mut by_curve: BTreeMap<String, Vec<(f64, f64, usize, usize, usize)>> = BTreeMap::new();
        for (r, loops) in regions.iter().enumerate() {
            for (l, edges) in loops.iter().enumerate() {
                for (e, p) in edges.iter().enumerate() {
                    let range = p.edge.curve.range;
                    let mid = p.edge.curve.point((range.first + range.last) / 2.0)?;
                    by_curve
                        .entry(serde_json::to_string(&p.edge.piece.curve).expect("curve id"))
                        .or_default()
                        .push((mid[0], mid[1], r, l, e));
                }
            }
        }
        for fragments in by_curve.values_mut() {
            let pieces: BTreeSet<u32> = fragments
                .iter()
                .map(|(_, _, r, l, e)| regions[*r][*l][*e].edge.piece.piece)
                .collect();
            if pieces.len() == fragments.len() {
                continue;
            }
            fragments.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            for (k, (_, _, r, l, e)) in fragments.iter().enumerate() {
                let p = &mut regions[*r][*l][*e];
                p.edge.piece.piece = k as u32;
                p.record.boundary.piece.piece = k as u32;
                if !p.record.aliases.contains(&p.edge.piece) {
                    p.record.aliases.push(p.edge.piece.clone());
                    canonical_aliases(&mut p.record.aliases);
                }
            }
        }
        let mut settled_regions = Vec::new();
        for loops in regions {
            let mut settled_loops = Vec::new();
            for (index, mut edges) in loops.into_iter().enumerate() {
                let mut material = MaterialLoop {
                    edges: edges.iter().map(|p| p.edge.clone()).collect(),
                };
                if (loop_area(&material)? > 0.0) != (index == 0) {
                    material.edges.reverse();
                    edges.reverse();
                    for e in &mut material.edges {
                        e.curve.range = e.curve.range.reversed();
                    }
                }
                for (p, edge) in edges.into_iter().zip(&material.edges) {
                    let mut record = p.record;
                    record.curve_generation = CurveLineage::generation(&edge.curve);
                    state.lineage.push(record);
                    settled
                        .tokens
                        .insert((output.domain, p.token), edge.piece.clone());
                }
                settled_loops.push(material);
            }
            settled_regions.push(MaterialRegion {
                loops: settled_loops,
            });
        }
        state
            .charts
            .iter_mut()
            .find(|p| p.domain == output.domain)
            .ok_or("sheet trim returned an unknown material domain")?
            .regions = settled_regions;
    }
    let removed = &settled.removed;
    state.charts.retain(|p| !removed.contains(&p.domain));
    state
        .lineage
        .retain(|l| !removed.contains(&l.boundary.domain));
    Ok(settled)
}

fn port_side(
    tokens: &BTreeMap<(SheetDomainRef, u32), CurvePieceRef>,
    axial: ParamSpan,
    side: &TrimEndpointSpan,
) -> Result<ShapePortSide, String> {
    let piece = tokens
        .get(&(side.domain, side.edge_token))
        .ok_or("a trimmed seam refers to an unknown boundary")?
        .clone();
    let scale = (axial.last - axial.first) / (side.range.last - side.range.first);
    if !scale.is_finite() || scale == 0.0 {
        return Err("a trimmed seam has a degenerate parameter map".into());
    }
    Ok(ShapePortSide {
        boundary: BoundaryPieceRef {
            domain: side.domain,
            piece,
        },
        span: side.range,
        axial: AxialMap {
            offset: axial.first - scale * side.range.first,
            scale,
        },
    })
}

/// A seam outside the cut must survive whole. The trim returns lines with
/// their own normalized parameters, so the seam's ends are found again on
/// the settled boundary of the same piece and its span and axial map are
/// rebuilt for them; a boundary the cut shortened or split is refused.
pub(super) fn rebind(
    before: &SheetState,
    after: &SheetState,
    side: &ShapePortSide,
) -> Option<ShapePortSide> {
    let edges = |state: &SheetState| -> Vec<MaterialEdge> {
        state
            .charts
            .iter()
            .filter(|p| p.domain == side.boundary.domain)
            .flat_map(|p| &p.regions)
            .flat_map(|r| &r.loops)
            .flat_map(|l| &l.edges)
            .filter(|e| e.piece.curve == side.boundary.piece.curve)
            .cloned()
            .collect()
    };
    let old: Vec<_> = edges(before)
        .into_iter()
        .filter(|e| {
            let (lo, hi) = (
                e.curve.range.first.min(e.curve.range.last),
                e.curve.range.first.max(e.curve.range.last),
            );
            e.piece == side.boundary.piece
                && side.span.first.min(side.span.last) >= lo - 1e-9
                && side.span.first.max(side.span.last) <= hi + 1e-9
        })
        .collect();
    let [old] = old.as_slice() else {
        return None;
    };
    let (p0, p1) = (
        old.curve.point(side.span.first).ok()?,
        old.curve.point(side.span.last).ok()?,
    );
    // Fragments of the same curve elsewhere in the chart may have been
    // renumbered; the seam is found again by position on any of them.
    for edge in edges(after) {
        let SegmentGeometry::Line { start, end } = edge.curve.geometry else {
            continue;
        };
        let d = crate::sketch::geometry::sub(end, start);
        let len = crate::sketch::geometry::dot(d, d);
        let at = |p: [f64; 2]| {
            crate::sketch::geometry::dot(crate::sketch::geometry::sub(p, start), d) / len
        };
        let (t0, t1) = (at(p0), at(p1));
        let (lo, hi) = (
            edge.curve.range.first.min(edge.curve.range.last),
            edge.curve.range.first.max(edge.curve.range.last),
        );
        let on = |t: f64, p: [f64; 2]| {
            t >= lo - 1e-9
                && t <= hi + 1e-9
                && edge
                    .curve
                    .point(t)
                    .is_ok_and(|q| crate::sketch::geometry::dist(p, q) <= TOLERANCE)
        };
        if len > 0.0 && on(t0, p0) && on(t1, p1) {
            let a0 = side.axial.offset + side.axial.scale * side.span.first;
            let a1 = side.axial.offset + side.axial.scale * side.span.last;
            let scale = (a1 - a0) / (t1 - t0);
            return Some(ShapePortSide {
                boundary: BoundaryPieceRef {
                    domain: side.boundary.domain,
                    piece: edge.piece.clone(),
                },
                span: ParamSpan {
                    first: t0,
                    last: t1,
                },
                axial: AxialMap {
                    offset: a0 - scale * t0,
                    scale,
                },
            });
        }
    }
    None
}

/// The result of [`developed_cut`]: the input configuration with the cut
/// material, possibly disconnected.
pub(crate) struct CutOutcome {
    pub state: SheetState,
    pub removed: BTreeSet<SheetDomainRef>,
    pub used_tools: bool,
}

/// Removes `tools` (regions in the chart coordinates of `stationary`) from
/// every chart in that panel's plane once `open` bends are developed, then
/// restores the input configuration. `what` names the operation in errors.
pub(crate) fn developed_cut<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    state: &SheetState,
    thickness: f64,
    stationary: PanelId,
    open: &[BendId],
    tools: &[MaterialRegion],
    what: &str,
) -> Result<CutOutcome, String> {
    let mut targets = BTreeMap::new();
    for bend in open {
        bend_record(state, *bend, thickness)?;
        targets.insert(*bend, true);
    }
    let mut work = state.clone();
    reconfigure(&mut work, thickness, stationary, &targets)?;
    let fixed_embedding = chart(&work, stationary)?.embedding.clone();
    // Every chart developed into the fixed plane is cut in one operation.
    let domains: Vec<TrimDomain> = work
        .charts
        .iter()
        .filter_map(|p| {
            planar_map(&p.embedding, &fixed_embedding)
                .ok()
                .map(|to_operation| TrimDomain {
                    domain: p.domain,
                    regions: p.regions.clone(),
                    to_operation,
                })
        })
        .collect();
    let inside = |domain: SheetDomainRef| domains.iter().any(|d| d.domain == domain);
    let seams: Vec<TrimSeam> = work
        .joins
        .iter()
        .filter(|p| inside(p.first.boundary.domain) && inside(p.second.boundary.domain))
        .map(|p| {
            let side = |s: &ShapePortSide| TrimSeamSide {
                boundary: s.boundary.clone(),
                span: s.span,
                axial: s.axial,
            };
            TrimSeam {
                token: p.token,
                first: side(&p.first),
                second: side(&p.second),
                axial_interval: p.axial_interval,
            }
        })
        .collect();
    let result = ctx
        .kernel
        .sheet_trim_domain(&SheetTrimSpec {
            operation: SheetTrimOperation::Difference,
            domains: &domains,
            tools,
            seams: &seams,
            tolerance: TOLERANCE,
        })
        .map_err(|e| e.to_string())?;
    let before = work.clone();
    let settled = settle(&mut work, &domains, &result)?;
    if work.charts.is_empty() {
        return Err(format!("{what} removes all material of the sheet"));
    }
    // A seam to material outside the cut plane must survive whole: cutting it
    // needs that bend opened as well.
    let mut joins = Vec::new();
    for port in work
        .joins
        .iter()
        .filter(|p| !(inside(p.first.boundary.domain) && inside(p.second.boundary.domain)))
    {
        let (Some(first), Some(second)) = (
            rebind(&before, &work, &port.first),
            rebind(&before, &work, &port.second),
        ) else {
            let (cut, folded) = if inside(port.first.boundary.domain) {
                (port.first.boundary.domain, port.second.boundary.domain)
            } else {
                (port.second.boundary.domain, port.first.boundary.domain)
            };
            return Err(format!(
                "{what} reaches the seam between {} and {}; open {} for it",
                domain_label(cut),
                domain_label(folded),
                match folded {
                    SheetDomainRef::Bend(_) => "that bend",
                    SheetDomainRef::Panel(_) => "the bend between them",
                }
            ));
        };
        joins.push(ShapePort {
            first,
            second,
            ..port.clone()
        });
    }
    let first_token = joins.iter().map(|p| p.token).max().map_or(0, |v| v + 1);
    for (next, port) in (first_token..).zip(&result.ports) {
        joins.push(ShapePort {
            token: next,
            axial_interval: port.axial_interval,
            first: port_side(&settled.tokens, port.axial_interval, &port.first)?,
            second: port_side(&settled.tokens, port.axial_interval, &port.second)?,
        });
    }
    work.joins = joins;
    for message in result.diagnostics {
        ctx.warn(message);
    }
    // Restore the input configuration: only embeddings change. A bend whose
    // material is gone loses its record and any open state with it.
    for patch in &mut work.charts {
        patch.embedding = state
            .charts
            .iter()
            .find(|p| p.domain == patch.domain)
            .ok_or("the cut changed the material domains")?
            .embedding
            .clone();
    }
    let removed = &settled.removed;
    work.bends = state
        .bends
        .iter()
        .filter(|b| !removed.contains(&SheetDomainRef::Bend(b.id)))
        .cloned()
        .collect();
    work.opened
        .retain(|o| !removed.contains(&SheetDomainRef::Bend(o.bend)));
    Ok(CutOutcome {
        state: work,
        removed: settled.removed,
        used_tools: settled.used_tools,
    })
}

/// The cut profiles in the chart coordinates of the fixed panel. Each
/// sketch lies on that panel's front or back surface (with the bends
/// developed the panel does not move, so its input chart serves).
fn sketch_tools<K: Kernel>(
    d: &SheetCutDef,
    ctx: &mut EvalContext<'_, K>,
    state: &SheetState,
    thickness: f64,
) -> Result<Vec<MaterialRegion>, String> {
    let fixed = SheetDomainRef::Panel(d.stationary);
    let (frame, parity) = plane(chart(state, d.stationary)?)?;
    let f = frame.normal().map(|v| v * parity.sign());
    let mut tools = Vec::new();
    let mut sketches: Vec<FeatureUid> = d.profiles.iter().map(|p| p.sketch).collect();
    sketches.sort();
    sketches.dedup();
    for sketch_uid in sketches {
        let sketch = ctx.sketch(sketch_uid)?;
        let s = sketch.frame;
        let offset = dot(sub(s.origin, frame.origin), f);
        if dot(s.normal(), frame.normal()).abs() < 1.0 - 1e-9
            || (offset.abs() > 1e-6 && (offset - thickness).abs() > 1e-6)
        {
            return Err(format!(
                "the cut sketch {sketch_uid} is not on a surface of the developed {}",
                domain_label(fixed)
            ));
        }
        let o = sub(s.origin, frame.origin);
        let to_chart = Isometry2 {
            origin: [dot(o, frame.x_axis), dot(o, frame.y_axis)],
            x_axis: [dot(s.x_axis, frame.x_axis), dot(s.x_axis, frame.y_axis)],
            y_axis: [dot(s.y_axis, frame.x_axis), dot(s.y_axis, frame.y_axis)],
        };
        for profile in d.profiles.iter().filter(|p| p.sketch == sketch_uid) {
            // A changed sketch keeps resolving the region with the most curves
            // in common, so a dimension edit that changes adjacency keeps the cut.
            let region = sketch
                .resolve_region(&profile.region)
                .ok_or_else(|| format!("cut profile {} is unavailable", profile.region))?;
            let material = MaterialRegion::from_legacy(&SheetRegion {
                loops: region
                    .loops
                    .iter()
                    .map(|l| SheetLoop {
                        segments: l
                            .segments
                            .iter()
                            .map(|s| SheetSegment {
                                id: SheetCurveId::Sketch {
                                    sketch: sketch_uid,
                                    segment: s.key.clone(),
                                },
                                geometry: s.geometry.clone(),
                            })
                            .collect(),
                    })
                    .collect(),
            })?;
            tools.extend(mapped_regions(&[material], to_chart));
        }
    }
    Ok(tools)
}

pub(crate) fn cut<K: Kernel>(
    d: &SheetCutDef,
    ctx: &mut EvalContext<'_, K>,
) -> Result<FeatureOutput<K::Shape>, String> {
    let (state, rules) = read(ctx, d.body)?;
    if d.profiles.is_empty() {
        return Err("no profiles selected".into());
    }
    let tools = sketch_tools(d, ctx, &state, rules.thickness)?;
    let outcome = developed_cut(
        ctx,
        &state,
        rules.thickness,
        d.stationary,
        &d.open_bends,
        &tools,
        "the cut",
    )?;
    if !outcome.used_tools {
        ctx.warn("the cut profiles do not reach the sheet material");
    }
    publish_pieces(ctx, outcome.state, &rules, None)
}
