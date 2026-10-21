// SPDX-License-Identifier: MIT
//! Timber members placed by two points (mitcad#478; `commands.md`, "BUILD:
//! timber construction"): a member from a start to an end point, lying on
//! the plane it was picked on (or grounded through the points), turned
//! flat or on edge about its axis.

use serde::{Deserialize, Serialize};

use super::geometry::{V3, Z, add, cross, dot, inverse, scale, sub, unit, z_axis};
use super::place::{JointPlan, RayPlan, SourceUse, Spot, refuse};
use crate::document::{Document, ModelError};
use crate::features::bound_length::RayDirection;
use crate::features::{FrameOverride, JointOrigin, OccurrencePath};
use crate::joints::{JointKind, SlideAxis};
use crate::kernel::{BoundingBox, Kernel};
use crate::transform::Transform;

/// Where a member by two points ends.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointTo {
    /// In the BUILD host's coordinates.
    pub point: V3,
}

/// How a member by two points lies about its axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lie {
    /// The section's smaller dimension across the plane it lies on.
    #[default]
    Flat,
    /// The larger one.
    Edge,
}

/// A member by two points, worked out: where it goes.
pub(crate) struct PointsPlan {
    pub spot: Spot,
}

/// The member's X axis (in the host's coordinates) for an axis `d`, its
/// section's box, the plane's normal `up` and the lie; and how far its axis
/// is lifted off the plane (0 when it stands square to it).
pub(crate) fn lie_axes(
    d: V3,
    section: Option<&BoundingBox>,
    up: V3,
    lie: Lie,
    quarter_turns: f64,
) -> (V3, f64) {
    let up_perp = unit(sub(up, scale(d, dot(d, up))));
    let (w, t) = section.map_or((0.0, 0.0), |b| (b.max[0] - b.min[0], b.max[1] - b.min[1]));
    let (mut x, lift) = match up_perp {
        None => {
            // Standing square to the plane: X along the host's X.
            let hint = [super::geometry::X, super::geometry::Y]
                .into_iter()
                .find(|a| dot(*a, d).abs() < 1.0 - 1e-6)
                .unwrap_or(super::geometry::X);
            (
                unit(sub(hint, scale(d, dot(d, hint)))).unwrap_or(super::geometry::X),
                0.0,
            )
        }
        Some(u) => {
            // The section's Y along `u` puts T across the plane; X along it
            // puts W across.
            let y_up = match lie {
                Lie::Flat => w >= t,
                Lie::Edge => w < t,
            };
            if y_up {
                let lift = section.map_or(0.0, |b| -b.min[1]);
                (cross(u, d), lift)
            } else {
                let lift = section.map_or(0.0, |b| -b.min[0]);
                (u, lift)
            }
        }
    };
    if quarter_turns.abs() > 1e-12 {
        let (s, c) = quarter_turns.sin_cos();
        x = add(scale(x, c), scale(cross(d, x), s));
    }
    (x, lift)
}

impl<K: Kernel> Document<K> {
    /// Plans a member by two points: from `at.point` to `to.point`, lying
    /// on `at.geometry` when given (a rigid joint with its frame
    /// overridden), else grounded through the points.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn plan_points(
        &self,
        source: &SourceUse,
        at: &super::PlaceAt,
        to: &PointTo,
        lie: Lie,
        flip: bool,
        rotate: f64,
        mounts: &std::collections::BTreeMap<String, Transform>,
        bounds: Option<&BoundingBox>,
    ) -> Result<PointsPlan, ModelError> {
        let label = source.def.label();
        if source.meta.length.is_none() {
            return Err(refuse(format!(
                "{label} is not cut to length: it is not placed by two points"
            )));
        }
        let start = at
            .point
            .ok_or_else(|| refuse("a member by two points needs its start (at.point)"))?;
        let (start, end) = if flip {
            (to.point, start)
        } else {
            (start, to.point)
        };
        let d = unit(sub(end, start))
            .ok_or_else(|| refuse("the two points are the same: the member has no length"))?;
        // The plane it lies on, in the host's coordinates.
        let on = match &at.geometry {
            Some(geometry) => {
                let path = self.path_from_host(&at.occurrence)?;
                let local = crate::joints::origin_frame(&mut self.resolver(), geometry)
                    .map_err(|e| refuse(format!("{geometry}: {e}")))?;
                Some((path, geometry.clone(), local))
            }
            None => None,
        };
        let up = match &on {
            Some((path, _, local)) => self.path_transform(path).apply_vector(z_axis(local)),
            None => Z,
        };
        let (x, lift) = lie_axes(d, bounds, up, lie, rotate);
        let lift = if on.is_some() { lift } else { 0.0 };
        let up_perp = unit(sub(up, scale(d, dot(d, up)))).unwrap_or(up);
        let origin = add(start, scale(up_perp, lift));
        let frame = super::geometry::frame(origin, x, d);
        let end_a = mounts.get("end_a").map_or([0.0; 3], |t| t.translation);
        let spot = match on {
            None => Spot::Grounded(frame.after(&Transform::translation(scale(end_a, -1.0)))),
            Some((path, geometry, local)) => {
                let placement = self.path_transform(&path);
                let to_local = inverse(&placement);
                let origin_local = to_local.apply_point(origin);
                let d_local = to_local.apply_vector(d);
                let x_local = to_local.apply_vector(x);
                let frame_inverse = inverse(&local);
                Spot::Joint(Box::new(JointPlan {
                    kind: JointKind::Rigid,
                    slide: SlideAxis::Z,
                    own: "end_a".to_owned(),
                    b: JointOrigin {
                        occurrence: OccurrencePath(path.clone()),
                        geometry: geometry.clone(),
                        frame_override: Some(FrameOverride {
                            origin: Some(origin_local),
                            z_axis: Some(d_local),
                            x_axis: Some(x_local),
                            shift: None,
                        }),
                    },
                    frame_b: frame,
                    flip: true,
                    slide_value: None,
                    ray: Some(RayPlan {
                        from: JointOrigin {
                            occurrence: OccurrencePath(path),
                            geometry,
                            frame_override: None,
                        },
                        shift: frame_inverse.apply_point(origin_local),
                        shift_by_joint: false,
                        slide_extra: 0.0,
                        direction: RayDirection::Vector(frame_inverse.apply_vector(d_local)),
                        start: origin,
                        along: d,
                    }),
                    side: None,
                    end: None,
                    own_override: None,
                    cuts: Vec::new(),
                    from: None,
                    warnings: Vec::new(),
                }))
            }
        };
        Ok(PointsPlan { spot })
    }
}

impl<K: Kernel> Document<K> {
    /// Plans a sheet laid on members' faces (`build_panel` with a sheet,
    /// `panel.mount: "face"`): over the rectangle of the picked faces (of
    /// every member face on the picked one's plane facing its way when no
    /// more are picked), its back on the face.
    pub(crate) fn plan_face_sheet(
        &self,
        input: &super::PanelInput,
        source: SourceUse,
    ) -> Result<super::connect::PanelPlan, ModelError> {
        use super::geometry::parts;
        use crate::expr::Unit;
        use crate::features::ValueInput;
        let label = source.def.label();
        let panel = source.meta.panel.clone().unwrap_or_default();
        let pick = |occurrence: &str,
                    geometry: Option<&crate::features::GeomRef>,
                    point: Option<V3>|
         -> Result<(super::place::Target, String), ModelError> {
            let geometry = geometry.ok_or_else(|| refuse("pick a face of a member"))?;
            let path = self.path_from_host(occurrence)?;
            let target = self
                .target(&path)
                .ok_or_else(|| refuse("pick a face of a member"))?;
            let face = match geometry {
                crate::features::GeomRef::Mount { name, .. } if target.info.is_side(name) => {
                    name.clone()
                }
                crate::features::GeomRef::Face { body, face } => {
                    let local = point.map(|p| inverse(&target.placement).apply_point(p));
                    self.pick_slot(&target, Some(&(*body, face.clone())), local)?
                }
                _ => return Err(refuse("pick a face of a member")),
            };
            Ok((target, face))
        };
        let (target, face) = pick(
            &input.at.occurrence,
            input.at.geometry.as_ref(),
            input.at.point,
        )?;
        let s1 = target
            .frame(&face)
            .ok_or_else(|| refuse(format!("{} has no frame {face}", target.info.name)))?;
        let [o1, x1, y1, z1] = parts(&s1);
        // The faces it covers, as members and their boxes.
        let mut covered: Vec<super::wood::Member> = Vec::new();
        let first = self
            .member(target.info.component)
            .ok_or_else(|| refuse("the member is not placed"))?;
        covered.push(first);
        if input.faces.is_empty() {
            for info in self.build_parts_info() {
                if info.component == target.info.component || !info.is_timber() {
                    continue;
                }
                let Some(m) = self.member(info.component) else {
                    continue;
                };
                let on_plane = info.sides().iter().any(|(_, t)| {
                    let f = m.placement.after(t);
                    dot(z_axis(&f), z1) > 1.0 - 1e-6 && dot(sub(f.translation, o1), z1).abs() < 1e-3
                });
                if on_plane {
                    covered.push(m);
                }
            }
        } else {
            for more in &input.faces {
                let (t, _) = pick(&more.occurrence, Some(&more.geometry), None)?;
                if let Some(m) = self.member(t.info.component) {
                    covered.push(m);
                }
            }
        }
        let range = |along: V3| {
            covered
                .iter()
                .map(|m| m.extent(along))
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), (lo, hi)| {
                    (a.min(lo), b.max(hi))
                })
        };
        let (ul, uh) = range(x1);
        let (vl, vh) = range(y1);
        let width = uh - ul;
        let height = vh - vl;
        // Its values: the sizes where they are parameters, the thickness.
        let design = source
            .def
            .design
            .state()
            .map_err(|e| refuse(format!("{label}: {e}")))?;
        let is_param = |name: &Option<String>| {
            name.clone()
                .filter(|n| design.parameters.find(n.trim()).is_some())
        };
        let mut values = input.parameters.clone();
        let mut warnings = Vec::new();
        if let Some(thickness) = &input.thickness {
            match panel
                .thickness
                .as_ref()
                .map(|t| t.text())
                .filter(|t| design.parameters.find(t.trim()).is_some())
            {
                Some(name) => {
                    values.insert(name.trim().to_owned(), thickness.clone());
                }
                None => warnings.push(format!("{label} has a fixed thickness")),
            }
        }
        if let Some(name) = is_param(&panel.width) {
            values.insert(name, ValueInput::Number(width));
        }
        if let Some(name) = is_param(&panel.height) {
            values.insert(name, ValueInput::Number(height));
        }
        let mut numbers = Vec::new();
        for (name, value) in &values {
            numbers.push((name.clone(), super::place::value_in(self, value, Unit::MM)?));
        }
        let state = super::parts::source_state(&source.def, source.config.as_deref(), &numbers)
            .map_err(|e| refuse(format!("{label}: {e}")))?;
        let evaluated = crate::features::derived_part::evaluate_source(self.kernel(), &state)
            .map_err(|e| refuse(format!("{label}: {e}")))?;
        let b = super::place::bounds_of(self.kernel(), &evaluated.bodies)
            .ok_or_else(|| refuse(format!("{label} has no size")))?;
        let (sw, sh) = (b.max[0] - b.min[0], b.max[1] - b.min[1]);
        let mut problems = Vec::new();
        if sw < width - 1e-6 {
            problems.push(format!(
                "its width of {} mm does not cover the {} mm of the faces",
                super::round(sw),
                super::round(width)
            ));
        }
        if sh < height - 1e-6 {
            problems.push(format!(
                "its height of {} mm does not cover the {} mm of the faces",
                super::round(sh),
                super::round(height)
            ));
        }
        // The middle of its box on the middle of the rectangle, its back
        // on the face.
        let centre = add(
            add(o1, scale(x1, (ul + uh) / 2.0 - dot(o1, x1))),
            scale(y1, (vl + vh) / 2.0 - dot(o1, y1)),
        );
        let middle = [
            -(b.min[0] + b.max[0]) / 2.0,
            -(b.min[1] + b.max[1]) / 2.0,
            -b.min[2],
        ];
        let placement =
            super::geometry::frame(centre, x1, z1).after(&Transform::translation(middle));
        let local = inverse(&target.placement).after(&placement);
        let [lo, lx, _, lz] = parts(&local);
        let b_origin = JointOrigin {
            occurrence: OccurrencePath(target.path.clone()),
            geometry: crate::features::GeomRef::Mount {
                feature: target.part()?,
                name: face.clone(),
            },
            frame_override: Some(FrameOverride {
                origin: Some(lo),
                z_axis: Some(lz),
                x_axis: Some(lx),
                shift: None,
            }),
        };
        let shown: std::collections::BTreeMap<String, f64> = numbers.into_iter().collect();
        let designation = super::parts::designation(&source.def, source.config.as_deref(), &shown);
        let name = input.name.clone().unwrap_or_else(|| {
            if designation.is_empty() {
                label.clone()
            } else {
                designation.clone()
            }
        });
        Ok(super::connect::PanelPlan {
            panel: super::connect::Piece {
                source,
                values,
                name,
                kind: JointKind::Rigid,
                slide: SlideAxis::Z,
                own: None,
                b: b_origin,
                flip: false,
                angle: 0.0,
                slide_value: None,
                record: super::BuildRecord {
                    role: super::BuildRole::Panel,
                    ..super::BuildRecord::default()
                },
            },
            bounds: Vec::new(),
            clamps: Vec::new(),
            fits: problems.is_empty(),
            reason: problems.join("; "),
            width,
            height,
            placement,
            warnings,
            designation,
        })
    }
}

impl<K: Kernel> Document<K> {
    /// `build_edit` of a member by two points with `to`, `at.point` or
    /// `lie`: placed anew from them (its component, derived part and the
    /// connectors on it stay).
    pub(crate) fn edit_points(
        &mut self,
        component: crate::ids::ComponentUid,
        input: &super::EditInput,
    ) -> Result<Vec<String>, ModelError> {
        let info = self
            .part_info(component)
            .ok_or_else(|| refuse("no such BUILD part"))?;
        let mut base: super::PlaceInput = info
            .record
            .points
            .clone()
            .and_then(|v| serde_json::from_value(v).ok())
            .ok_or_else(|| refuse(format!("{} was not placed by two points", info.name)))?;
        if let Some(to) = &input.to {
            base.to = Some(to.clone());
        }
        if let Some(lie) = input.lie {
            base.lie = Some(lie);
        }
        if let Some(point) = input.at.as_ref().and_then(|a| a.point) {
            let mut at = base.at.clone().unwrap_or_default();
            at.point = Some(point);
            base.at = Some(at);
        }
        if let Some(length) = &input.length {
            base.length = Some(length.clone());
        }
        if let Some(config) = &input.config {
            base.part.config = Some(config.clone());
        }
        let plan = self.plan_place(&base)?;
        let active = self.build_host();
        self.state().editable(active)?;
        let label = format!("Edit {}", info.name);
        self.apply(|state| {
            super::spacing::replace_member(state, &plan, active, component)?;
            Ok((label, vec!["points".to_owned(), "length".to_owned()]))
        })
    }
}
