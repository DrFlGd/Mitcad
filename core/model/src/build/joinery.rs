// SPDX-License-Identifier: MIT
//! Joinery at junctions of timber members (mitcad#478; `commands.md`,
//! "BUILD: timber construction", *Joinery*): a housing, a notch or a
//! half-lap, a rectangular cut sized from the other member's section, kept
//! in the members' derived parts (`cuts`) and bound to the members: the
//! coordinate along a member is an expression of the other member's slide
//! on it, or of its own length near end B.

use serde::Deserialize;

use super::geometry::{V3, Z, cross, dot, inverse, scale, unit, z_axis};
use super::junctions::Junction;
use super::place::{edit_part, refuse, value_in};
use super::wood::Member;
use crate::document::{Document, ModelError};
use crate::expr::Unit;
use crate::features::derived_part::PartCut;
use crate::features::{FeatureDef, GeomRef, ValueInput};
use crate::ids::{ComponentUid, FeatureUid};
use crate::joints::{JointKind, Motion};
use crate::kernel::Kernel;

/// `build_connect`'s `joint`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointInput {
    /// `housing`, `notch` or `half_lap`.
    pub kind: String,
    #[serde(default)]
    pub depth: Option<ValueInput>,
    /// The member a notch is cut in: `a` (the default) or `b`.
    #[serde(default)]
    pub on: Option<String>,
}

/// A joinery cut planned: the member, its derived part and the cut.
#[derive(Debug, Clone)]
pub(crate) struct CutPlan {
    pub component: ComponentUid,
    pub part: FeatureUid,
    pub cut: PartCut<ValueInput>,
}

/// The members' derived parts and the cuts each is to have.
pub(crate) type RefitPlan = Vec<(FeatureUid, Vec<PartCut<ValueInput>>)>;

/// A box in the host: its axes (e3 = e1 x e2) and the range along each.
struct HostBox {
    axes: [V3; 3],
    ranges: [[f64; 2]; 3],
}

/// Three axes with e3 = e1 x e2: the third flipped (and its range with it)
/// when it was given the other way.
fn right_handed(e1: V3, e2: V3, e3: V3, r3: [f64; 2]) -> (V3, [f64; 2]) {
    let c = cross(e1, e2);
    if dot(c, e3) >= 0.0 {
        (c, r3)
    } else {
        (c, [-r3[1], -r3[0]])
    }
}

/// The overlap of two ranges.
fn overlap(p: (f64, f64), q: (f64, f64)) -> [f64; 2] {
    [p.0.max(q.0), p.1.min(q.1)]
}

impl<K: Kernel> Document<K> {
    /// The cuts a joint makes, planned against the members as they are.
    pub(crate) fn plan_joinery(
        &self,
        junction_id: &str,
        joint: &JointInput,
    ) -> Result<(Vec<CutPlan>, Vec<String>), ModelError> {
        let junction: Junction = self
            .build_junctions(None)
            .into_iter()
            .find(|j| j.id == junction_id)
            .ok_or_else(|| refuse(format!("no junction {junction_id}")))?;
        let a = self
            .member(junction.a.component)
            .ok_or_else(|| refuse("the junction's parts are gone"))?;
        let b = self
            .member(junction.b.component)
            .ok_or_else(|| refuse("the junction's parts are gone"))?;
        for m in [&a, &b] {
            if !m.info.is_cut_to_length() || m.info.meta.system.is_some() {
                return Err(refuse(format!(
                    "{} is no timber member: joinery is cut into members",
                    m.info.name
                )));
            }
        }
        let depth = match &joint.depth {
            Some(v) => Some(value_in(self, v, Unit::MM)?),
            None => None,
        };
        if depth.is_some_and(|d| d.is_nan() || d <= 0.0) {
            return Err(refuse("the joint's depth must be greater than zero"));
        }
        let warnings = Vec::new();
        let host = inverse(&self.host_placement());
        let mut cuts = Vec::new();
        match joint.kind.as_str() {
            "housing" => {
                if !matches!(junction.a.at.as_str(), "end_a" | "end_b") {
                    return Err(refuse(format!(
                        "a housing takes an end on a side ({} is {})",
                        junction.id, junction.kind
                    )));
                }
                let end = host.after(&junction.a.frame);
                let into_a = scale(z_axis(&end), -1.0);
                let contact = dot(end.translation, into_a);
                let (b_lo, _) = b.extent(into_a);
                let thick = contact - b_lo;
                let d = depth.unwrap_or((thick / 3.0).min(15.0));
                let u = a.placement.apply_vector([1.0, 0.0, 0.0]);
                let v = a.placement.apply_vector([0.0, 1.0, 0.0]);
                let region = HostBox::new(
                    u,
                    v,
                    overlap(a.extent(u), b.extent(u)),
                    overlap(a.extent(v), b.extent(v)),
                    into_a,
                    [contact - d, contact + 1.0],
                );
                cuts.push(self.cut_in(&b, &a, &junction, "housing", &region, d)?);
            }
            "notch" | "half_lap" => {
                let n = unit(cross(a.axis(), b.axis())).ok_or_else(|| {
                    refuse(format!(
                        "the members of {} run along one axis: no {} there",
                        junction.id, joint.kind
                    ))
                })?;
                let u = a.axis();
                let v = unit(cross(n, u)).expect("square to n");
                let fu = overlap(a.extent(u), b.extent(u));
                let fv = overlap(a.extent(v), b.extent(v));
                if joint.kind == "notch" {
                    let (on, other) = if joint.on.as_deref() == Some("b") {
                        (&b, &a)
                    } else {
                        (&a, &b)
                    };
                    // From `on` towards the other.
                    let c = |m: &Member| {
                        let bb = &m.bounds;
                        dot(
                            m.placement.apply_point(std::array::from_fn(|i| {
                                (bb.min[i] + bb.max[i]) / 2.0
                            })),
                            n,
                        )
                    };
                    let towards = if c(other) >= c(on) { n } else { scale(n, -1.0) };
                    let (on_lo, on_hi) = on.extent(towards);
                    let (other_lo, _) = other.extent(towards);
                    let lap = on_hi - other_lo;
                    let d = depth.unwrap_or(if lap > 1e-6 {
                        lap
                    } else {
                        (on_hi - on_lo) / 3.0
                    });
                    let region = HostBox::new(u, v, fu, fv, towards, [on_hi - d, on_hi + 1.0]);
                    cuts.push(self.cut_in(on, other, &junction, "notch", &region, d)?);
                } else {
                    let up = if dot(n, Z) < 0.0 { scale(n, -1.0) } else { n };
                    let o = overlap(a.extent(up), b.extent(up));
                    let t = o[1] - o[0];
                    if t <= 1e-6 || fu[1] - fu[0] <= 1e-6 || fv[1] - fv[0] <= 1e-6 {
                        return Err(refuse(
                            "a half-lap needs the members overlapping: place them on one plane",
                        ));
                    }
                    let da = depth.unwrap_or(t / 2.0).min(t);
                    let db = t - da;
                    let below = HostBox::new(u, v, fu, fv, up, [o[0] - 1.0, o[0] + da]);
                    cuts.push(self.cut_in(&a, &b, &junction, "half_lap", &below, da)?);
                    if db > 1e-6 {
                        let above = HostBox::new(u, v, fu, fv, up, [o[1] - db, o[1] + 1.0]);
                        cuts.push(self.cut_in(&b, &a, &junction, "half_lap", &above, db)?);
                    }
                }
            }
            other => {
                return Err(refuse(format!(
                    "the joint is housing, notch or half_lap, not {other}"
                )));
            }
        }
        Ok((cuts, warnings))
    }

    /// A cut of `region` (host) in member `m`, bound along it to `other`'s
    /// slide on it or to its own length near end B.
    fn cut_in(
        &self,
        m: &Member,
        other: &Member,
        junction: &Junction,
        kind: &str,
        region: &HostBox,
        depth: f64,
    ) -> Result<CutPlan, ModelError> {
        let part = m
            .info
            .part
            .ok_or_else(|| refuse(format!("{} has no derived part", m.info.name)))?;
        let to_local = inverse(&m.placement);
        let corner = (0..3).fold([0.0; 3], |acc, k| {
            super::geometry::add(acc, scale(region.axes[k], region.ranges[k][0]))
        });
        let origin = to_local.apply_point(corner);
        let x = to_local.apply_vector(region.axes[0]);
        let y = to_local.apply_vector(region.axes[1]);
        let size = region.ranges.map(|r| r[1] - r[0]);
        let tidy = |v: f64| {
            let r = (v * 1e6).round() / 1e6;
            if r == 0.0 { 0.0 } else { r }
        };
        let along = self.bound_along(m, other, origin[2]);
        Ok(CutPlan {
            component: m.info.component,
            part,
            cut: PartCut {
                kind: kind.to_owned(),
                with: Some(other.info.component),
                junction: Some(junction.id.clone()),
                origin: [
                    ValueInput::Number(tidy(origin[0])),
                    ValueInput::Number(tidy(origin[1])),
                    along,
                ],
                x: x.map(tidy),
                y: y.map(tidy),
                size: size.map(|s| ValueInput::Number(tidy(s))),
                depth: Some(tidy(depth)),
            },
        })
    }

    /// The coordinate `z` along member `m` as an expression: of `other`'s
    /// slide when it slides on `m`, else of `m`'s length near end B, else a
    /// number.
    fn bound_along(&self, m: &Member, other: &Member, z: f64) -> ValueInput {
        let params = self.parameters();
        let plus = |name: String, k: f64| {
            let k = (k * 1e6).round() / 1e6;
            if k.abs() < 1e-9 {
                ValueInput::Name(name)
            } else if k > 0.0 {
                ValueInput::Name(format!("{name} + {}", super::round(k)))
            } else {
                ValueInput::Name(format!("{name} - {}", super::round(-k)))
            }
        };
        if let (Some(joint), Some(part)) = (other.info.record.joint, m.info.part)
            && let Some(FeatureDef::Joint(j)) = self.feature(joint).map(|e| &e.def)
            && j.kind == JointKind::Slider
            && matches!(&j.b.geometry, GeomRef::Mount { feature, name }
                if *feature == part && m.info.is_side(name))
            && let Some(id) = j.position.get(&Motion::Tx)
            && let Some(slide) = params.value(*id)
        {
            return plus(params.name(*id), z - slide);
        }
        if let Some((id, length)) = m.info.length
            && z > length / 2.0
        {
            return plus(params.name(id), z - length);
        }
        ValueInput::Number((z * 1e6).round() / 1e6)
    }

    /// `build_connect` with `joint`: the members cut; returns the cuts.
    pub(crate) fn build_joinery(
        &mut self,
        input: &super::ConnectInput,
    ) -> Result<super::Placed, ModelError> {
        let joint = input.joint.clone().unwrap_or_default();
        let (cuts, warnings) = self.plan_joinery(&input.junction, &joint)?;
        let label = format!("Connect {}", joint.kind);
        self.apply(|state| {
            let mut features = Vec::new();
            for c in &cuts {
                let cut = c.cut.clone();
                edit_part(state, c.part, |def| def.cuts.push(cut))?;
                if !features.contains(&c.part) {
                    features.push(c.part);
                }
            }
            Ok((
                label,
                super::Placed {
                    features,
                    warnings: warnings.clone(),
                    place: "junction",
                    ..super::Placed::default()
                },
            ))
        })
    }

    /// `build_edit`'s `cuts: "refit"`: a member's joinery cut anew (in it
    /// and in the other members of its junctions) from where they are now,
    /// at the depths they were cut to.
    pub(crate) fn refit_plan(&self, component: ComponentUid) -> Result<RefitPlan, ModelError> {
        let info = self
            .part_info(component)
            .ok_or_else(|| refuse("no such BUILD part"))?;
        let def = info.def.clone().ok_or_else(|| refuse("no derived part"))?;
        let mut joints: Vec<(String, String, Option<f64>, bool)> = Vec::new();
        for cut in &def.cuts {
            let Some(junction) = &cut.junction else {
                continue;
            };
            if joints
                .iter()
                .any(|(j, k, ..)| j == junction && *k == cut.kind)
            {
                continue;
            }
            joints.push((junction.clone(), cut.kind.clone(), cut.depth, false));
        }
        // The members touched: their cuts of those junctions go.
        let mut touched: Vec<(FeatureUid, Vec<PartCut<ValueInput>>)> = Vec::new();
        for part in self.build_parts_info() {
            let (Some(p), Some(d)) = (part.part, &part.def) else {
                continue;
            };
            if d.cuts.iter().any(|c| {
                c.junction
                    .as_ref()
                    .is_some_and(|j| joints.iter().any(|(jj, ..)| jj == j))
            }) {
                let kept = match as_input_cuts(self, p) {
                    Some(all) => all
                        .into_iter()
                        .filter(|c| {
                            !c.junction
                                .as_ref()
                                .is_some_and(|j| joints.iter().any(|(jj, ..)| jj == j))
                        })
                        .collect(),
                    None => Vec::new(),
                };
                touched.push((p, kept));
            }
        }
        for (junction, kind, depth, _) in &joints {
            let joint = JointInput {
                kind: kind.clone(),
                depth: depth.map(ValueInput::Number),
                on: None,
            };
            let Ok((cuts, _)) = self.plan_joinery(junction, &joint) else {
                continue;
            };
            for c in cuts {
                match touched.iter_mut().find(|(p, _)| *p == c.part) {
                    Some((_, list)) => list.push(c.cut),
                    None => touched.push((c.part, vec![c.cut])),
                }
            }
        }
        Ok(touched)
    }
}

impl HostBox {
    fn new(e1: V3, e2: V3, r1: [f64; 2], r2: [f64; 2], e3: V3, r3: [f64; 2]) -> Self {
        let (e3, r3) = right_handed(e1, e2, e3, r3);
        HostBox {
            axes: [e1, e2, e3],
            ranges: [r1, r2, r3],
        }
    }
}

/// A derived part's cuts in command form.
fn as_input_cuts<K: Kernel>(
    doc: &Document<K>,
    part: FeatureUid,
) -> Option<Vec<PartCut<ValueInput>>> {
    let entry = doc.feature(part)?;
    match super::parts::as_input(&entry.def, doc.parameters()) {
        FeatureDef::DerivedPart(d) => Some(d.cuts),
        _ => None,
    }
}
