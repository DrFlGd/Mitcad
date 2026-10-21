// SPDX-License-Identifier: MIT
//! Wood fasteners and outer connectors at junctions of timber members
//! (mitcad#478; `commands.md`, "BUILD: timber construction", *Wood
//! fasteners* and *Brackets, hangers and post bases*): screws and nails
//! driven through one member into the other, sized from the members when
//! their row is not given, with warnings for a short embedment and edge
//! and end distances; joist hangers, angle brackets and post bases fitted by
//! the members' sections and screwed on through their hole frames.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::connect::{Piece, make_piece};
use super::geometry::{V3, add, cross, dot, inverse, scale, sub, unit, z_axis};
use super::junctions::Junction;
use super::parts::{PartInfo, source_state};
use super::place::{Placed, SourceUse, bounds_of, refuse};
use super::{BuildItem, BuildMeta, BuildRecord, BuildRole};
use crate::document::{Document, ModelError};
use crate::features::derived_part::evaluate_source;
use crate::features::{FrameOverride, GeomRef, JointOrigin, OccurrencePath};
use crate::ids::{ComponentUid, FeatureUid, OccurrenceUid};
use crate::joints::{JointKind, SlideAxis};
use crate::kernel::{BoundingBox, Kernel};
use crate::transform::Transform;

/// A member's free end a post base goes on.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndInput {
    pub component: String,
    #[serde(default)]
    pub end: super::place::EndFrom,
}

/// A member as the fastening rules see it: where it is in the BUILD host
/// and its box there.
#[derive(Debug, Clone)]
pub(crate) struct Member {
    pub info: PartInfo,
    pub path: Vec<OccurrenceUid>,
    /// Its placement in the BUILD host.
    pub placement: Transform,
    pub bounds: BoundingBox,
}

impl Member {
    fn corners(&self) -> Vec<V3> {
        self.bounds
            .corners()
            .map(|c| self.placement.apply_point(c))
            .collect()
    }

    /// The least and the most of its box along a direction (host).
    pub fn extent(&self, along: V3) -> (f64, f64) {
        self.corners()
            .iter()
            .map(|c| dot(*c, along))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(v), b.max(v))
            })
    }

    pub fn axis(&self) -> V3 {
        self.placement.apply_vector([0.0, 0.0, 1.0])
    }

    /// The section's smaller dimension.
    pub fn thinnest(&self) -> f64 {
        let b = &self.bounds;
        (b.max[0] - b.min[0]).min(b.max[1] - b.min[1])
    }
}

/// How fasteners go through a junction: through one member into the
/// other along `axis` (host coordinates), entering at `head` along the
/// axis, the contact a rectangle about `centre` (`u`, `v`, half sizes).
#[derive(Debug, Clone)]
pub(crate) struct Fastening {
    pub through: Member,
    pub into: Member,
    pub axis: V3,
    pub head: f64,
    pub thickness: f64,
    pub depth: f64,
    pub centre: V3,
    pub u: V3,
    pub v: V3,
    pub half: [f64; 2],
}

impl Fastening {
    /// The largest diameter the sizing rule takes: the thinner member's
    /// smaller section dimension / 7.
    pub fn max_diameter(&self) -> f64 {
        self.through.thinnest().min(self.into.thinnest()) / 7.0
    }

    /// Where `count` fasteners go on the contact plane (host): in a row
    /// along its longer side, centred across it.
    pub fn points(&self, count: usize) -> Vec<V3> {
        let n = count.max(1);
        let (long, across, h) = if self.half[0] >= self.half[1] {
            (self.u, self.v, self.half[0])
        } else {
            (self.v, self.u, self.half[1])
        };
        let _ = across;
        (0..n)
            .map(|i| {
                let s = -h + (i as f64 + 0.5) * 2.0 * h / n as f64;
                add(self.centre, scale(long, s))
            })
            .collect()
    }
}

/// A wood fastener's sizes and row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WoodSize {
    pub config: Option<String>,
    pub diameter: f64,
    pub length: f64,
}

/// A wood fastener planned at a junction: the pieces (the first leads),
/// their frames and the warnings.
#[derive(Debug, Clone)]
pub(crate) struct WoodPlan {
    pub lead: Option<Piece>,
    pub pieces: Vec<Piece>,
    /// Pieces on the lead (a connector's fasteners): their hole frames.
    pub on_lead: Vec<Piece>,
    pub frames: Vec<(String, Transform)>,
    pub size: Option<WoodSize>,
    pub warnings: Vec<String>,
    pub designation: String,
    pub junction: Option<String>,
    pub placement: Option<Transform>,
}

/// An override frame on a member's `end_a` frame: at `origin` with axes
/// `x` and `z`, all in the host, turned into the member's coordinates.
fn origin_on(member: &Member, origin: V3, x: V3, z: V3) -> Result<JointOrigin, ModelError> {
    let part = member
        .info
        .part
        .ok_or_else(|| refuse(format!("{} has no derived part", member.info.name)))?;
    let to_local = inverse(&member.placement);
    Ok(JointOrigin {
        occurrence: OccurrencePath(member.path.clone()),
        geometry: GeomRef::Mount {
            feature: part,
            name: "end_a".to_owned(),
        },
        frame_override: Some(FrameOverride {
            origin: Some(to_local.apply_point(origin)),
            z_axis: Some(to_local.apply_vector(z)),
            x_axis: Some(to_local.apply_vector(x)),
            shift: None,
        }),
    })
}

/// A unit vector square to `z`.
fn square_to(z: V3) -> V3 {
    super::geometry::perpendicular(z)
}

/// The range of points along a direction.
fn range(points: &[V3], along: V3) -> (f64, f64) {
    points
        .iter()
        .map(|p| dot(*p, along))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        })
}

impl<K: Kernel> Document<K> {
    /// A BUILD part as a member: its path, placement and box in the host.
    pub(crate) fn member(&self, component: ComponentUid) -> Option<Member> {
        let info = self.part_info(component)?;
        let path = self.placed_in_host(component)?;
        let placement = self.path_transform(&path);
        let bounds = self.part_bounds(component)?;
        Some(Member {
            info,
            path,
            placement,
            bounds,
        })
    }

    /// How fasteners go through a junction of two members: through the
    /// side member into the end that meets it, or through the newer of two
    /// crossing members into the other; None for one end to end on one
    /// axis.
    pub(crate) fn fastening(&self, junction: &Junction) -> Option<Fastening> {
        if junction.kind == "inline" {
            return None;
        }
        let a = self.member(junction.a.component)?;
        let b = self.member(junction.b.component)?;
        let host = inverse(&self.host_placement());
        let end_of_a = matches!(junction.a.at.as_str(), "end_a" | "end_b");
        if end_of_a {
            let end = host.after(&junction.a.frame);
            // Into a along its axis from its end; through b from its far
            // face.
            let axis = scale(z_axis(&end), -1.0);
            let contact = dot(end.translation, axis);
            let (b_lo, _) = b.extent(axis);
            let (_, a_hi) = a.extent(axis);
            let thickness = (contact - b_lo).max(0.0);
            // The end's section on the contact plane, within b.
            let u = a.placement.apply_vector([1.0, 0.0, 0.0]);
            let v = a.placement.apply_vector([0.0, 1.0, 0.0]);
            let a_corners = a.corners();
            let b_corners = b.corners();
            let (au, bu) = (range(&a_corners, u), range(&b_corners, u));
            let (av, bv) = (range(&a_corners, v), range(&b_corners, v));
            let lo_u = au.0.max(bu.0);
            let hi_u = au.1.min(bu.1);
            let lo_v = av.0.max(bv.0);
            let hi_v = av.1.min(bv.1);
            let p = end.translation;
            let centre = add(
                add(p, scale(u, (lo_u + hi_u) / 2.0 - dot(p, u))),
                scale(v, (lo_v + hi_v) / 2.0 - dot(p, v)),
            );
            return Some(Fastening {
                axis,
                head: b_lo,
                thickness,
                depth: a_hi - contact,
                centre,
                u,
                v,
                half: [
                    ((hi_u - lo_u) / 2.0).max(0.0),
                    ((hi_v - lo_v) / 2.0).max(0.0),
                ],
                through: b,
                into: a,
            });
        }
        // Crossing (or lying on each other): through a into b, square to
        // both axes.
        let n = unit(cross(a.axis(), b.axis()))?;
        let towards = sub(box_centre(&b), box_centre(&a));
        let axis = if dot(n, towards) >= 0.0 {
            n
        } else {
            scale(n, -1.0)
        };
        let (a_lo, a_hi) = a.extent(axis);
        let (b_lo, b_hi) = b.extent(axis);
        let mut contact = b_lo.clamp(a_lo, a_hi);
        if contact - a_lo <= 1e-9 {
            contact = (a_lo + a_hi) / 2.0;
        }
        let u = a.axis();
        let v = unit(cross(axis, u))?;
        let (au, bu) = (range(&a.corners(), u), range(&b.corners(), u));
        let (av, bv) = (range(&a.corners(), v), range(&b.corners(), v));
        let lo_u = au.0.max(bu.0);
        let hi_u = au.1.min(bu.1);
        let lo_v = av.0.max(bv.0);
        let hi_v = av.1.min(bv.1);
        let centre = add(
            add(scale(axis, contact), scale(u, (lo_u + hi_u) / 2.0)),
            scale(v, (lo_v + hi_v) / 2.0),
        );
        Some(Fastening {
            axis,
            head: a_lo,
            thickness: contact - a_lo,
            depth: b_hi - contact,
            centre,
            u,
            v,
            half: [
                ((hi_u - lo_u) / 2.0).max(0.0),
                ((hi_v - lo_v) / 2.0).max(0.0),
            ],
            through: a,
            into: b,
        })
    }

    /// A wood fastener's size: its row's, or by the sizing rule when the
    /// source has rows and none is named.
    pub(crate) fn wood_size(
        &self,
        source: &SourceUse,
        fastening: Option<&Fastening>,
        warnings: &mut Vec<String>,
    ) -> Result<WoodSize, ModelError> {
        let label = source.def.label();
        let build = source
            .def
            .build
            .clone()
            .ok_or_else(|| refuse(format!("{label} has no build")))?;
        let size_of = |row: Option<&str>| -> Option<(f64, f64)> {
            let meta = BuildMeta::read_row(&build, row).ok()?;
            let wood = meta.wood?;
            Some((wood.diameter_mm()?, wood.length_mm()?))
        };
        if source.config.is_some() || fastening.is_none() {
            let (diameter, length) = size_of(source.config.as_deref())
                .ok_or_else(|| refuse(format!("{label} has no wood diameter and length")))?;
            return Ok(WoodSize {
                config: source.config.clone(),
                diameter,
                length,
            });
        }
        let fastening = fastening.expect("checked");
        let design = source.def.design.state().map_err(refuse)?;
        let rows: Vec<(String, f64, f64)> = design
            .configurations
            .rows
            .iter()
            .filter_map(|r| {
                let (d, l) = size_of(Some(&r.name))?;
                Some((r.name.clone(), d, l))
            })
            .collect();
        if rows.is_empty() {
            let (diameter, length) = size_of(None)
                .ok_or_else(|| refuse(format!("{label} has no wood diameter and length")))?;
            return Ok(WoodSize {
                config: None,
                diameter,
                length,
            });
        }
        let most = fastening.max_diameter();
        let diameters: Vec<f64> = rows.iter().map(|r| r.1).collect();
        let d = diameters
            .iter()
            .copied()
            .filter(|d| *d <= most + 1e-9)
            .fold(None, |m: Option<f64>, d| Some(m.map_or(d, |m| m.max(d))))
            .unwrap_or_else(|| diameters.iter().copied().fold(f64::INFINITY, f64::min));
        let wanted = fastening.thickness + 8.0 * d;
        let mut same: Vec<&(String, f64, f64)> =
            rows.iter().filter(|r| (r.1 - d).abs() < 1e-9).collect();
        same.sort_by(|x, y| x.2.total_cmp(&y.2));
        let pick = same
            .iter()
            .find(|r| r.2 >= wanted - 1e-9)
            .or(same.last())
            .copied()
            .expect("a row of that diameter");
        if pick.2 < wanted - 1e-9 {
            warnings.push(format!(
                "the longest {} mm {} is {} mm: {} mm would embed 8 diameters",
                super::round(d),
                label,
                super::round(pick.2),
                super::round(wanted)
            ));
        }
        Ok(WoodSize {
            config: Some(pick.0.clone()),
            diameter: pick.1,
            length: pick.2,
        })
    }

    /// The warnings of a fastener `i` (from 1) at `point` on the contact
    /// plane: its embedment, its distance to the contact's edges and to an
    /// end of the member it goes through.
    pub(crate) fn fastener_warnings(
        &self,
        fastening: &Fastening,
        size: &WoodSize,
        i: usize,
        point: V3,
        kind: &str,
        out: &mut Vec<String>,
    ) {
        let d = size.diameter;
        let embed = size.length - fastening.thickness;
        let into = &fastening.into.info.name;
        let through = &fastening.through.info.name;
        if embed < 6.0 * d - 1e-9 {
            out.push(format!(
                "{kind} {i} embeds {} mm in {into}: at least {} mm",
                super::round(embed.max(0.0)),
                super::round(6.0 * d)
            ));
        }
        if size.length > fastening.thickness + fastening.depth + 1e-9 {
            out.push(format!("{kind} {i} goes through {into}"));
        }
        let q = sub(point, fastening.centre);
        let edge = (fastening.half[0] - dot(q, fastening.u).abs())
            .min(fastening.half[1] - dot(q, fastening.v).abs());
        if edge < 3.0 * d - 1e-9 {
            out.push(format!(
                "{kind} {i} is {} mm from an edge: at least {} mm",
                super::round(edge.max(0.0)),
                super::round(3.0 * d)
            ));
        }
        let t_axis = fastening.through.axis();
        if dot(t_axis, fastening.axis).abs() < 1.0 - 1e-6 {
            let (lo, hi) = fastening.through.extent(t_axis);
            let at = dot(point, t_axis);
            let end = (at - lo).min(hi - at);
            if end < 10.0 * d - 1e-9 {
                out.push(format!(
                    "{kind} {i} is {} mm from the end of {through}: at least {} mm",
                    super::round(end.max(0.0)),
                    super::round(10.0 * d)
                ));
            }
        }
    }

    /// Whether a `build_connect` goes the timber way: a wood fastener, an
    /// outer connector, or a post base on an end.
    pub(crate) fn is_wood_connect(&self, input: &super::ConnectInput) -> bool {
        if input.end.is_some() {
            return true;
        }
        self.source_use(&input.part).is_ok_and(|s| {
            s.meta.wood.is_some() || s.meta.connector.as_ref().is_some_and(|c| c.is_outer())
        })
    }

    /// Plans screws or nails, or an outer connector with its fasteners.
    pub(crate) fn plan_wood(&self, input: &super::ConnectInput) -> Result<WoodPlan, ModelError> {
        let source = self.source_use(&input.part)?;
        let label = source.def.label();
        if source.meta.wood.is_some() {
            return self.plan_screws(input, source);
        }
        let connector = source.meta.connector.clone().unwrap_or_default();
        if !connector.is_outer() {
            return Err(refuse(format!(
                "{label} goes in slots: a member has no slots"
            )));
        }
        self.plan_outer(input, source, &label)
    }

    pub(crate) fn junction_of(&self, id: &str) -> Result<Junction, ModelError> {
        self.build_junctions(None)
            .into_iter()
            .find(|j| j.id == id)
            .ok_or_else(|| refuse(format!("no junction {id}")))
    }

    fn plan_screws(
        &self,
        input: &super::ConnectInput,
        mut source: SourceUse,
    ) -> Result<WoodPlan, ModelError> {
        let label = source.def.label();
        let junction = self.junction_of(&input.junction)?;
        for side in [&junction.a, &junction.b] {
            if self
                .part_info(side.component)
                .is_some_and(|i| i.meta.system.is_some())
            {
                return Err(refuse(format!(
                    "{label} is driven into timber: the junction has a slot profile"
                )));
            }
        }
        let fastening = self
            .fastening(&junction)
            .ok_or_else(|| refuse("an inline joint takes a plate"))?;
        let mut warnings = Vec::new();
        let size = self.wood_size(&source, Some(&fastening), &mut warnings)?;
        source.config = size.config.clone();
        let kind = source
            .meta
            .wood
            .as_ref()
            .map_or("screw", |w| if w.kind == "nail" { "nail" } else { "screw" })
            .to_owned();
        let count = input.count.unwrap_or(2).max(1) as usize;
        let designation =
            super::parts::designation(&source.def, source.config.as_deref(), &BTreeMap::new());
        let name = if designation.is_empty() {
            label.clone()
        } else {
            designation.clone()
        };
        let mut pieces = Vec::new();
        let mut frames = Vec::new();
        let back = scale(fastening.axis, -1.0);
        for (i, p) in fastening.points(count).into_iter().enumerate() {
            self.fastener_warnings(&fastening, &size, i + 1, p, &kind, &mut warnings);
            // At the entry: on the face the fastener goes in from.
            let entry = add(
                p,
                scale(fastening.axis, fastening.head - dot(p, fastening.axis)),
            );
            let x = square_to(back);
            let b = origin_on(&fastening.through, entry, x, back)?;
            frames.push((
                format!("{kind}_{}", i + 1),
                super::geometry::frame(entry, x, fastening.axis),
            ));
            pieces.push(Piece {
                source: source.clone(),
                values: BTreeMap::new(),
                name: name.clone(),
                kind: JointKind::Rigid,
                slide: SlideAxis::Z,
                own: Some(axis_frame(&source)),
                b,
                flip: true,
                angle: 0.0,
                slide_value: None,
                record: BuildRecord {
                    role: BuildRole::Fastener,
                    junction: Some(junction.id.clone()),
                    ..BuildRecord::default()
                },
            });
        }
        Ok(WoodPlan {
            lead: None,
            pieces,
            on_lead: Vec::new(),
            frames,
            size: Some(size),
            warnings,
            designation: name,
            junction: Some(junction.id),
            placement: None,
        })
    }

    fn plan_outer(
        &self,
        input: &super::ConnectInput,
        source: SourceUse,
        label: &str,
    ) -> Result<WoodPlan, ModelError> {
        let connector = source.meta.connector.clone().unwrap_or_default();
        let fit = connector.fit.clone().unwrap_or_default();
        let mount = connector
            .mates
            .first()
            .map(|m| m.frame.clone())
            .unwrap_or_else(|| "mount".to_owned());
        let (mounts, _) = evaluated_frames(self, &source)?;
        let own_mount = *mounts
            .get(&mount)
            .ok_or_else(|| refuse(format!("{label} has no frame {mount}")))?;
        // The carried member (a; the post) and where the mount frame goes.
        let (carried, carrier, origin, x, z_into, junction) = match &input.end {
            Some(end) => {
                if fit.kind != "post_base" && !fit.kind.is_empty() {
                    return Err(refuse(format!(
                        "{label} is no post base: it goes on a junction"
                    )));
                }
                let c = self.component_by_ref(&end.component)?;
                let post = self
                    .member(c)
                    .ok_or_else(|| refuse(format!("{} is no placed member", end.component)))?;
                let frame_name = if end.end == super::place::EndFrom::B {
                    "end_b"
                } else {
                    "end_a"
                };
                let local = post
                    .info
                    .mount(frame_name)
                    .copied()
                    .ok_or_else(|| refuse(format!("{} has no {frame_name}", post.info.name)))?;
                let frame = post.placement.after(&local);
                let into = scale(z_axis(&frame), -1.0);
                let x = post.placement.apply_vector([1.0, 0.0, 0.0]);
                let origin = frame.translation;
                (post.clone(), post, origin, x, into, None)
            }
            None => {
                if fit.kind == "post_base" {
                    return Err(refuse(format!(
                        "{label} is a post base: it goes on a member's end (end)"
                    )));
                }
                let junction = self.junction_of(&input.junction)?;
                if !matches!(junction.a.at.as_str(), "end_a" | "end_b") {
                    return Err(refuse(format!(
                        "{label} goes where an end meets a side ({} is {})",
                        junction.id, junction.kind
                    )));
                }
                let a = self
                    .member(junction.a.component)
                    .ok_or_else(|| refuse("the junction's parts are gone"))?;
                let b = self
                    .member(junction.b.component)
                    .ok_or_else(|| refuse("the junction's parts are gone"))?;
                if a.info.meta.system.is_some() || b.info.meta.system.is_some() {
                    return Err(refuse(format!(
                        "{label} goes on timber: the junction has a slot profile"
                    )));
                }
                let host = inverse(&self.host_placement());
                let end = host.after(&junction.a.frame);
                let into_b = z_axis(&end);
                let (origin, x) = match fit.kind.as_str() {
                    "angle" => {
                        let side = input.side.clone().unwrap_or_else(|| {
                            let b = &a.bounds;
                            if b.max[0] - b.min[0] >= b.max[1] - b.min[1] {
                                "y+".to_owned()
                            } else {
                                "x+".to_owned()
                            }
                        });
                        let (k, sign) = match side.as_str() {
                            "x+" => (0, 1.0),
                            "x-" => (0, -1.0),
                            "y+" => (1, 1.0),
                            "y-" => (1, -1.0),
                            other => {
                                return Err(refuse(format!(
                                    "side is x+, x-, y+ or y-, not {other}"
                                )));
                            }
                        };
                        let mut local = [0.0; 3];
                        local[k] = sign;
                        let normal = a.placement.apply_vector(local);
                        let half = if sign > 0.0 {
                            a.bounds.max[k]
                        } else {
                            -a.bounds.min[k]
                        };
                        (add(end.translation, scale(normal, half)), normal)
                    }
                    _ => {
                        // The direction of a's section most up.
                        let up = super::geometry::Z;
                        let candidates = [
                            a.placement.apply_vector([1.0, 0.0, 0.0]),
                            a.placement.apply_vector([0.0, 1.0, 0.0]),
                        ];
                        let best = candidates
                            .into_iter()
                            .max_by(|p, q| dot(*p, up).abs().total_cmp(&dot(*q, up).abs()))
                            .expect("two");
                        let x = if dot(best, up).abs() < 1e-6 {
                            candidates[0]
                        } else if dot(best, up) < 0.0 {
                            scale(best, -1.0)
                        } else {
                            best
                        };
                        (end.translation, x)
                    }
                };
                (a, b, origin, x, into_b, Some(junction))
            }
        };
        // Fits the carried member's section.
        let size = [
            carried.bounds.max[0] - carried.bounds.min[0],
            carried.bounds.max[1] - carried.bounds.min[1],
        ];
        let size = carried.info.meta.section_size().unwrap_or(size);
        if let Some(section) = fit
            .section
            .as_ref()
            .and_then(|s| Some([s.first()?.millimetres()?, s.get(1)?.millimetres()?]))
        {
            let same = |p: [f64; 2], q: [f64; 2]| {
                ((p[0] - q[0]).abs() <= 1.0 && (p[1] - q[1]).abs() <= 1.0)
                    || ((p[0] - q[1]).abs() <= 1.0 && (p[1] - q[0]).abs() <= 1.0)
            };
            if !same(section, size) {
                return Err(refuse(format!(
                    "the {} fits {} x {}, {} is {} x {}",
                    if fit.kind.is_empty() {
                        "connector"
                    } else {
                        fit.kind.as_str()
                    },
                    super::round(section[0]),
                    super::round(section[1]),
                    carried.info.name,
                    super::round(size[0].min(size[1])),
                    super::round(size[0].max(size[1]))
                )));
            }
        }
        if let Some([lo, hi]) = fit
            .width
            .as_ref()
            .and_then(|w| Some([w.first()?.millimetres()?, w.get(1)?.millimetres()?]))
        {
            let thin = size[0].min(size[1]);
            if thin < lo - 1e-6 || thin > hi + 1e-6 {
                return Err(refuse(format!(
                    "{label} fits members {} to {} mm thick, {} is {} mm",
                    super::round(lo),
                    super::round(hi),
                    carried.info.name,
                    super::round(thin)
                )));
            }
        }
        // The override on the carrier's (the post's) end_a frame: z out of
        // it, towards the connector; flipped, the mount's z points in.
        let out = scale(z_into, -1.0);
        let b = origin_on(&carrier, origin, x, out)?;
        let frame_b = super::geometry::frame(origin, x, out);
        let placement = frame_b
            .after(&crate::joints::alignment(0.0, 0.0, true))
            .after(&inverse(&own_mount));
        let designation =
            super::parts::designation(&source.def, source.config.as_deref(), &BTreeMap::new());
        let name = input.name.clone().unwrap_or_else(|| {
            if designation.is_empty() {
                label.to_owned()
            } else {
                designation.clone()
            }
        });
        let lead = Piece {
            source: source.clone(),
            values: BTreeMap::new(),
            name: name.clone(),
            kind: JointKind::Rigid,
            slide: SlideAxis::Z,
            own: Some(mount.clone()),
            b,
            flip: true,
            angle: 0.0,
            slide_value: None,
            record: BuildRecord {
                role: BuildRole::Connector,
                junction: junction.as_ref().map(|j| j.id.clone()),
                ..BuildRecord::default()
            },
        };
        // Its fasteners through its hole frames.
        let mut warnings = Vec::new();
        let mut on_lead = Vec::new();
        let mut frames = Vec::new();
        if input.fasteners {
            for f in &connector.fasteners {
                let Some(reference) = f.screw.as_ref().or(f.nail.as_ref()) else {
                    continue;
                };
                let part = super::connect::request_of(reference, &source.def);
                let used = match self.fastener_source(&part) {
                    Ok(used) => used,
                    Err(e) => {
                        warnings.push(format!("fastener {} is left out: {e}", reference.component));
                        continue;
                    }
                };
                let kind = if f.nail.is_some() && f.screw.is_none() {
                    "nail"
                } else {
                    "screw"
                };
                let size = self.wood_size(&used, None, &mut warnings).ok();
                let member = if f.on == "b" { &carrier } else { &carried };
                for (i, hole) in f.frames.iter().enumerate() {
                    let Some(local) = mounts.get(hole) else {
                        warnings.push(format!("{label} has no frame {hole}"));
                        continue;
                    };
                    let at = placement.after(local);
                    frames.push((hole.clone(), at));
                    if let Some(size) = &size {
                        // Into the member along the hole's -z.
                        let axis = scale(z_axis(&at), -1.0);
                        let (lo, _) = member.extent(axis);
                        let gone = (lo - dot(at.translation, axis)).max(0.0);
                        let embed = size.length - gone;
                        if embed < 6.0 * size.diameter - 1e-9 {
                            warnings.push(format!(
                                "{kind} {} embeds {} mm in {}: at least {} mm",
                                i + 1,
                                super::round(embed.max(0.0)),
                                member.info.name,
                                super::round(6.0 * size.diameter)
                            ));
                        }
                    }
                    let fname = used.def.label();
                    on_lead.push(Piece {
                        source: used.clone(),
                        values: BTreeMap::new(),
                        name: fname,
                        kind: JointKind::Rigid,
                        slide: SlideAxis::Z,
                        own: Some(axis_frame(&used)),
                        b: JointOrigin {
                            occurrence: OccurrencePath::default(),
                            geometry: GeomRef::Mount {
                                feature: FeatureUid(0),
                                name: hole.clone(),
                            },
                            frame_override: None,
                        },
                        flip: true,
                        angle: 0.0,
                        slide_value: None,
                        record: BuildRecord {
                            role: BuildRole::Fastener,
                            ..BuildRecord::default()
                        },
                    });
                }
            }
        }
        Ok(WoodPlan {
            lead: Some(lead),
            pieces: Vec::new(),
            on_lead,
            frames,
            size: None,
            warnings,
            designation: name,
            junction: junction.map(|j| j.id),
            placement: Some(placement),
        })
    }

    /// `build_connect` with a wood fastener or an outer connector.
    pub(crate) fn build_wood_connect(
        &mut self,
        input: &super::ConnectInput,
    ) -> Result<Placed, ModelError> {
        let plan = self.plan_wood(input)?;
        let active = self.build_host();
        self.state().editable(active)?;
        let label = format!("Connect {}", plan.designation);
        self.apply(|state| {
            let mut made = Vec::new();
            let mut adds = Vec::new();
            let mut lead = None;
            if let Some(piece) = &plan.lead {
                let (c, occurrence, part) = make_piece(state, piece, active)?;
                lead = Some((c, occurrence, part));
                for f in &plan.on_lead {
                    let mut f = f.clone();
                    f.b.occurrence = OccurrencePath(vec![occurrence]);
                    if let GeomRef::Mount { feature, .. } = &mut f.b.geometry {
                        *feature = part;
                    }
                    f.record.added_by = Some(c);
                    let (fc, _, _) = make_piece(state, &f, active)?;
                    adds.push(BuildItem::Component(fc));
                }
            }
            for piece in &plan.pieces {
                let (c, occurrence, part) = make_piece(state, piece, active)?;
                match lead {
                    None => lead = Some((c, occurrence, part)),
                    Some((first, _, _)) => {
                        if let Some(r) = state
                            .assembly
                            .component_mut(c)
                            .and_then(|c| c.build.as_mut())
                        {
                            r.added_by = Some(first);
                        }
                        adds.push(BuildItem::Component(c));
                    }
                }
                made.push(c);
            }
            let (c, occurrence, part) = lead.ok_or_else(|| refuse("nothing to place"))?;
            let record = state
                .assembly
                .component_mut(c)
                .and_then(|c| c.build.as_mut())
                .expect("made");
            record.adds = adds.clone();
            let joint = record.joint;
            let name = state.assembly.name(c);
            Ok((
                label,
                Placed {
                    component: Some(c),
                    occurrence: Some(occurrence),
                    name,
                    features: vec![part],
                    joint,
                    bound: Vec::new(),
                    adds,
                    warnings: plan.warnings.clone(),
                    place: "junction",
                    fits: None,
                    set: Vec::new(),
                },
            ))
        })
    }
}

/// A fastener's frame that goes on the hole: its `axis` frame (kind
/// `shaft`), else its first frame.
pub(crate) fn axis_frame(source: &SourceUse) -> String {
    source
        .meta
        .frames
        .iter()
        .find(|f| f.name == "axis" || f.kind == "shaft")
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "axis".to_owned())
}

/// The middle of a member's box (host).
fn box_centre(m: &Member) -> V3 {
    let b = &m.bounds;
    m.placement
        .apply_point(std::array::from_fn(|i| (b.min[i] + b.max[i]) / 2.0))
}

/// A source's frames and box at its row.
fn evaluated_frames<K: Kernel>(
    doc: &Document<K>,
    source: &SourceUse,
) -> Result<(BTreeMap<String, Transform>, Option<BoundingBox>), ModelError> {
    let label = source.def.label();
    let state = source_state(&source.def, source.config.as_deref(), &[])
        .map_err(|e| refuse(format!("{label}: {e}")))?;
    let evaluated =
        evaluate_source(doc.kernel(), &state).map_err(|e| refuse(format!("{label}: {e}")))?;
    let bounds = bounds_of(doc.kernel(), &evaluated.bodies);
    Ok((evaluated.mounts, bounds))
}

/// What `build_junctions` says of a junction of two timber members: the
/// member a fastener goes through and the one it goes into, the thickness
/// gone through, the sizing rule's largest diameter and the sections.
pub(crate) fn wood_json(f: &Fastening, junction: &Junction) -> serde_json::Value {
    let section = |m: &Member| {
        m.info.meta.section_size().unwrap_or([
            m.bounds.max[0] - m.bounds.min[0],
            m.bounds.max[1] - m.bounds.min[1],
        ])
    };
    let (a, b) = if f.into.info.component == junction.a.component {
        (&f.into, &f.through)
    } else {
        (&f.through, &f.into)
    };
    serde_json::json!({
        "through": f.through.info.component,
        "into": f.into.info.component,
        "thickness": f.thickness,
        "max_diameter": f.max_diameter(),
        "sections": {"a": section(a), "b": section(b)},
    })
}
