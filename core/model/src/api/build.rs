// SPDX-License-Identifier: MIT
//! Commands and queries of the BUILD tab (mitcad#375; `commands.md`,
//! "BUILD: profile assemblies"): placing, editing, connecting, panels and
//! removing, the preview of each, the parts, the junctions and the cut
//! list. Frames in answers are in the design's coordinates; lengths are
//! millimetres and angles radians.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::ApiError;
use crate::build::place::{LengthPlan, Plan, Spot, value_in};
use crate::build::{
    BuildRole, ConnectInput, CutListOptions, EditInput, MitreCut, PanelInput, PartInput,
    PlaceInput, Placed,
};
use crate::document::Document;
use crate::expr::Unit;
use crate::features::bound_length::BoundMode;
use crate::features::derived_part::{EndCut, Ends, evaluate_source};
use crate::features::{FeatureDef, GeomRef, OccurrencePath, ValueInput};
use crate::ids::{ComponentUid, OccurrenceUid};
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::transform::Transform;

/// What a placing's ray meets: the occurrence path, the face and the
/// length to it.
type RayHit = (Vec<OccurrenceUid>, GeomRef, f64);

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
// The variants are named after the commands (`build_place`, ...), read from `cmd`.
#[allow(clippy::enum_variant_names)]
pub(crate) enum BuildCommand {
    BuildPlace(Box<PlaceInput>),
    BuildEdit(Box<EditInput>),
    BuildConnect(Box<ConnectInput>),
    BuildPanel(Box<PanelInput>),
    BuildRemove { component: String },
}

/// The commands' names, to send them here.
pub(crate) const COMMANDS: [&str; 5] = [
    "build_place",
    "build_edit",
    "build_connect",
    "build_panel",
    "build_remove",
];

/// The `build_junctions` query.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JunctionsQuery {
    #[serde(default)]
    components: Option<Vec<String>>,
}

/// The `build_cut_list` query.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CutListQuery {
    #[serde(default)]
    kerf: Option<f64>,
}

/// A frame as answers give it.
fn frame_json(t: &Transform) -> Value {
    let [origin, x_axis, y_axis, z_axis] = crate::build::parts(t);
    json!({"origin": origin, "x_axis": x_axis, "y_axis": y_axis, "z_axis": z_axis})
}

fn path_text(path: &[OccurrenceUid]) -> String {
    OccurrencePath(path.to_vec()).to_string()
}

fn ends_json(ends: &Ends, angle: &dyn Fn(ParamId) -> Option<f64>) -> Value {
    let cut = |c: &Option<EndCut>| match c {
        Some(c) => json!({"angle": angle(c.angle), "tilt": c.tilt.unwrap_or_default().as_str()}),
        None => Value::Null,
    };
    json!({"a": cut(&ends.a), "b": cut(&ends.b)})
}

fn placed_json(placed: &Placed) -> Value {
    let mut value = json!({
        "component": placed.component,
        "occurrence": placed.occurrence,
        "name": placed.name,
        "features": placed.features,
        "joint": placed.joint,
        "bound": placed.bound,
        "adds": placed.adds,
        "warnings": placed.warnings,
        "place": placed.place,
    });
    if let Some((fits, reason)) = &placed.fits {
        value["fits"] = json!(fits);
        value["reason"] = if reason.is_empty() {
            Value::Null
        } else {
            json!(reason)
        };
    }
    value
}

/// What a preview request is: a connector's, a panel's or a placing's.
enum PreviewInput {
    Place(Box<PlaceInput>),
    Panel(Box<PanelInput>),
    Connect(Box<ConnectInput>),
}

impl<K: Kernel> Document<K> {
    pub(crate) fn run_build_command(&mut self, command: BuildCommand) -> Result<Value, ApiError> {
        Ok(match command {
            BuildCommand::BuildPlace(input) => placed_json(&self.build_place(&input)?),
            BuildCommand::BuildEdit(input) => json!({"changed": self.build_edit(&input)?}),
            BuildCommand::BuildConnect(input) => placed_json(&self.build_connect(&input)?),
            BuildCommand::BuildPanel(input) => placed_json(&self.build_panel(&input)?),
            BuildCommand::BuildRemove { component } => {
                let removed = self.build_remove(&component)?;
                json!({"removed": {
                    "components": removed.components,
                    "features": removed.features,
                    "detached": removed.detached,
                }})
            }
        })
    }

    /// Reads a preview request (`build_preview`, `analysis_shape`):
    /// build_place's, build_panel's or build_connect's fields.
    fn preview_input(&self, mut fields: Map<String, Value>) -> Result<PreviewInput, ApiError> {
        fields.remove("query");
        fields.remove("shape");
        let value = Value::Object(fields);
        if value.get("junction").is_some() {
            return super::parse(value, "build_preview")
                .map(|input| PreviewInput::Connect(Box::new(input)));
        }
        let part: PartInput = super::parse(value["part"].clone(), "part")?;
        let panel = self
            .source_use(&part)
            .is_ok_and(|s| s.meta.role == BuildRole::Panel);
        if panel {
            super::parse(value, "build_preview").map(|input| PreviewInput::Panel(Box::new(input)))
        } else {
            super::parse(value, "build_preview").map(|input| PreviewInput::Place(Box::new(input)))
        }
    }

    /// The `build_preview` query.
    pub(crate) fn build_preview_json(&self, fields: Map<String, Value>) -> Result<Value, ApiError> {
        let problems = |e: crate::document::ModelError| json!({"problems": [e.to_string()]});
        Ok(match self.preview_input(fields)? {
            PreviewInput::Place(input) => match self.preview_place(&input) {
                Ok(plan) => self.place_preview_json(&input, &plan),
                Err(e) => problems(e),
            },
            PreviewInput::Panel(input) => match self.plan_panel(&input) {
                Ok(plan) => {
                    let world = self.host_placement().after(&plan.placement);
                    json!({
                        "frame": frame_json(&world),
                        "place": "panel",
                        "fits": plan.fits,
                        "reason": if plan.reason.is_empty() { Value::Null } else { json!(plan.reason) },
                        "width": finite(plan.width),
                        "height": finite(plan.height),
                        "bound": plan.bounds.iter().map(|b| b.parameter.clone()).collect::<Vec<_>>(),
                        "designation": plan.designation,
                        "warnings": plan.warnings,
                        "body": "see analysis_shape",
                        "problems": [],
                    })
                }
                Err(e) => problems(e),
            },
            PreviewInput::Connect(input) => match self.plan_connect(&input) {
                Ok(plan) => {
                    let active = self.host_placement();
                    let machining: Vec<Value> = plan
                        .holes
                        .iter()
                        .map(|h| {
                            json!({"component": h.component,
                                   "at": h.def["placement"]["plane"]["mount"],
                                   "point": h.def["placement"]["points"][0],
                                   "diameter": h.def["diameter"],
                                   "depth": h.def["extent"]["depth"],
                                   "thread": h.def["thread"]["designation"]})
                        })
                        .collect();
                    json!({
                        "frame": frame_json(&active.after(&plan.placement)),
                        "place": "junction",
                        "junction": plan.junction.id,
                        "fasteners": plan.fastener_frames.iter().map(|(name, t)| {
                            json!({"frame": name, "at": frame_json(&active.after(t))})
                        }).collect::<Vec<_>>(),
                        "fastener_parts": plan.fasteners.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
                        "machining": machining,
                        "warnings": plan.warnings,
                        "body": "see analysis_shape",
                        "problems": [],
                    })
                }
                Err(e) => problems(e),
            },
        })
    }

    /// What a placing's ray meets and the new part's length: a length
    /// given is the value; else what the ray gives, else the source's
    /// default. The preview's shape is cut to the same length.
    fn place_preview_length(
        &self,
        input: &PlaceInput,
        plan: &Plan,
    ) -> (Option<RayHit>, Option<f64>) {
        let world = self.host_placement().after(&plan.placement);
        let flip = input.flip;
        let touch = plan
            .mounts
            .get(if flip { "end_b" } else { "end_a" })
            .copied();
        let start = touch.map_or(world.translation, |t| world.apply_point(t.translation));
        let along = world.apply_vector(if flip {
            [0.0, 0.0, -1.0]
        } else {
            crate::build::Z
        });
        let mode = match &plan.length {
            LengthPlan::Bound { mode, .. } => *mode,
            _ => BoundMode::Face,
        };
        let hit = self.ray_target(
            start,
            along,
            plan.target.as_ref().map(|t| t.info.component),
            mode,
        );
        let given = matches!(input.length, Some(crate::build::LengthInput::Value(_)));
        let length_value = match (&plan.length, &hit) {
            (LengthPlan::None, _) => None,
            (LengthPlan::Value(_, v), _) if given => Some(*v),
            (_, Some(hit)) => Some(hit.2),
            (LengthPlan::Value(_, v), None) => Some(*v),
            (LengthPlan::Bound { first, .. }, None) => Some(*first),
        };
        (hit, length_value)
    }

    /// A placing's preview.
    fn place_preview_json(&self, input: &PlaceInput, plan: &Plan) -> Value {
        let active = self.host_placement();
        let world = active.after(&plan.placement);
        let end_a = plan.mounts.get("end_a").map_or([0.0; 3], |t| t.translation);
        let frame = crate::build::frame(
            world.apply_point(end_a),
            world.apply_vector(crate::build::X),
            world.apply_vector(crate::build::Z),
        );
        let params = |c: &Option<EndCut<ValueInput>>| match c {
            Some(c) => json!({
                "angle": value_in(self, &c.angle, Unit::DEG).ok(),
                "tilt": c.tilt.unwrap_or_default().as_str(),
            }),
            None => Value::Null,
        };
        let mut ends = json!({"a": params(&plan.own_ends.a), "b": params(&plan.own_ends.b)});
        if let Some((part, b, cut)) = &plan.target_ends {
            ends["target"] = json!({
                "part": part,
                "end": if *b { "b" } else { "a" },
                "angle": value_in(self, &cut.angle, Unit::DEG).ok(),
                "tilt": cut.tilt.unwrap_or_default().as_str(),
            });
        }
        let bound_to = match &plan.length {
            LengthPlan::Bound { to, .. } => Some((**to).clone()),
            _ => None,
        };
        let (hit, length_value) = self.place_preview_length(input, plan);
        let mode = match &plan.length {
            LengthPlan::Bound { mode, .. } => *mode,
            _ => BoundMode::Face,
        };
        let given = matches!(input.length, Some(crate::build::LengthInput::Value(_)));
        let default = match &plan.length {
            LengthPlan::Value(_, v) if !given => Some(*v),
            LengthPlan::Bound { first, .. } => Some(*first),
            _ => None,
        };
        let target = match (bound_to, &hit) {
            (Some(to), _) => {
                json!({"occurrence": to.occurrence.to_string(), "geometry": to.geometry})
            }
            (None, Some((path, geometry, _))) => {
                json!({"occurrence": path_text(path), "geometry": geometry})
            }
            (None, None) => Value::Null,
        };
        // Snaps along the slot of a side placement (mitcad#394): where the
        // new part's near face may go, and the point on the slot where the
        // part then stands (its middle along the slot).
        let mut snaps = Vec::new();
        let mut snapped = Value::Null;
        if plan.place == "side"
            && let (Some(target), Spot::Joint(joint)) = (&plan.target, &plan.spot)
            && let GeomRef::Mount { name, .. } = &joint.b.geometry
            && let Some(slot) = target.info.mount(name)
            && let Some(side) = &joint.side
        {
            let to_world = active.after(&target.placement).after(slot);
            let middle = (side.near + side.far) / 2.0;
            let at = |offset: f64| to_world.apply_point([offset - side.near + middle, 0.0, 0.0]);
            for (kind, offset) in side.snaps() {
                snaps.push(json!({"kind": kind, "offset": offset, "point": at(offset)}));
            }
            // The reference the placement is bound to and the offset from
            // it, as `at.from` and `at.offset` take them (mitcad#417).
            snapped = json!({"kind": side.snapped, "from": side.from.as_str(),
                             "offset": side.from_offset, "point": at(side.offset),
                             "width": side.far - side.near,
                             "long_point": side.long_point,
                             "foot": active.apply_point(joint.frame_b.translation),
                             "normal": active.apply_vector(crate::build::z_axis(&to_world)),
                             "along": active.apply_vector(crate::build::x_axis(&to_world))});
        }
        // On an end (mitcad#417): straight or turned, the turns the panel
        // offers, a corner's offset and the mitre it gets.
        let mut end = Value::Null;
        if let Spot::Joint(joint) = &plan.spot
            && let Some(e) = &joint.end
        {
            let host = |v: crate::build::V3| active.apply_vector(v);
            let mitre = match &e.mitre {
                Some(m) => {
                    let cut = match m.cut {
                        MitreCut::New => "new",
                        MitreCut::Existing => "existing",
                        MitreCut::Shared => "shared",
                        MitreCut::None => "none",
                    };
                    let own = plan.own_ends.a.as_ref().or(plan.own_ends.b.as_ref());
                    let angle = match m.cut {
                        MitreCut::Existing => plan.target_ends.as_ref().map(|(_, _, c)| &c.angle),
                        _ => own.map(|c| &c.angle),
                    };
                    json!({"cut": cut,
                           "angle": angle.and_then(|a| value_in(self, a, Unit::DEG).ok())})
                }
                None => json!({"cut": "none", "angle": null}),
            };
            end = json!({
                "zone": e.zone,
                "direction": host(e.direction),
                "turns": e.turns.map(host),
                "offset": e.offset,
                "mitre": mitre,
            });
        }
        json!({
            "frame": frame_json(&frame),
            "place": plan.place,
            "length": {
                "value": length_value,
                "default": default,
                "target": target,
                "mode": mode.as_str(),
            },
            "ends": ends,
            "tilt": plan.tilt,
            "snaps": snaps,
            "side": snapped,
            "end": end,
            "host": self.host_json(),
            "designation": plan.designation,
            "box": plan.bounds.as_ref().map(|b| json!({"min": b.min, "max": b.max})),
            "warnings": plan.warnings,
            "body": "see analysis_shape",
            "problems": [],
        })
    }

    /// What a ray from `start` along `along` (design coordinates) meets
    /// first: a frame of another BUILD part facing it, within that part's
    /// box; with the path, the frame and the length the mode gives.
    pub(crate) fn ray_target(
        &self,
        start: crate::build::V3,
        along: crate::build::V3,
        except: Option<ComponentUid>,
        mode: BoundMode,
    ) -> Option<RayHit> {
        use crate::build::{box_along, dot, in_box, moved_box, ray_plane, unit, z_axis};
        let along = unit(along)?;
        let active = self.host_placement();
        let mut best: Option<(f64, Vec<OccurrenceUid>, GeomRef, f64)> = None;
        for info in self.build_parts_info() {
            if Some(info.component) == except {
                continue;
            }
            let (Some(path), Some(part), Some(bounds)) = (
                self.placed_in_host(info.component),
                info.part,
                self.part_bounds(info.component),
            ) else {
                continue;
            };
            let world = active.after(&self.path_transform(&path));
            let world_box = moved_box(&bounds, &world);
            for (name, local) in &info.mounts {
                let kind = info.frame_kind(name);
                if kind != "slot" && kind != "end" {
                    continue;
                }
                let frame = world.after(local);
                let n = z_axis(&frame);
                if dot(n, along) > -1e-6 {
                    continue;
                }
                let Some(t) = ray_plane(start, along, frame.translation, n) else {
                    continue;
                };
                if t <= 1e-6 {
                    continue;
                }
                let point = crate::build::add(start, crate::build::scale(along, t));
                if !in_box(point, &world_box, 1e-3) {
                    continue;
                }
                if best.as_ref().is_none_or(|(d, ..)| t < *d) {
                    let far = box_along(&bounds, &world, start, along).1;
                    let value = match mode {
                        BoundMode::Face => t,
                        BoundMode::Centre => (t + far) / 2.0,
                        BoundMode::Through => far,
                    };
                    best = Some((
                        t,
                        path.clone(),
                        GeomRef::Mount {
                            feature: part,
                            name: name.clone(),
                        },
                        value,
                    ));
                }
            }
        }
        best.map(|(_, path, geometry, value)| (path, geometry, value))
    }

    /// The preview's shapes (`analysis_shape` with `build_preview`): the
    /// new part's bodies in the design's coordinates.
    pub(crate) fn build_preview_shape(
        &self,
        fields: Map<String, Value>,
    ) -> Result<Option<K::Shape>, ApiError> {
        // The end that touches the target (end B when flipped), kept where
        // the plan put it when the length differs from the plan's.
        let mut ends = None;
        let (source, values, mut placement, touch) = match self.preview_input(fields)? {
            PreviewInput::Place(input) => {
                let plan = self.preview_place(&input)?;
                // A bound length (or none given) is the ray's, as the
                // query says, not the source's default.
                let mut values = plan.values.clone();
                let mut touch = None;
                if let (Some(meta), (_, Some(length))) = (
                    &plan.source.meta.length,
                    self.place_preview_length(&input, &plan),
                ) {
                    values.insert(meta.parameter.clone(), ValueInput::Number(length));
                    let end = if input.flip { "end_b" } else { "end_a" };
                    touch = plan.mounts.get(end).map(|t| (end, t.translation));
                    // Its end cuts as the part will have them (mitres, a
                    // tilt's foot, mitcad#443 and #444).
                    ends = Some((plan.own_ends.clone(), length));
                }
                (plan.source, values, plan.placement, touch)
            }
            PreviewInput::Panel(input) => {
                let plan = self.plan_panel(&input)?;
                (plan.panel.source, plan.panel.values, plan.placement, None)
            }
            PreviewInput::Connect(input) => {
                let plan = self.plan_connect(&input)?;
                (plan.connector.source, BTreeMap::new(), plan.placement, None)
            }
        };
        let mut numbers = Vec::new();
        for (name, value) in &values {
            numbers.push((name.clone(), value_in(self, value, Unit::MM)?));
        }
        let state =
            crate::build::parts::source_state(&source.def, source.config.as_deref(), &numbers)
                .map_err(ApiError)?;
        let evaluated = evaluate_source(self.kernel(), &state).map_err(ApiError)?;
        if let Some((end, was)) = touch
            && let Some(now) = evaluated.mounts.get(end)
        {
            let shift = crate::transform::sub(was, now.translation);
            placement = placement.after(&Transform::translation(shift));
        }
        let world = self.host_placement().after(&placement);
        let kernel = self.kernel();
        let failed = |e: crate::kernel::KernelError| ApiError(e.to_string());
        let section = crate::build::place::bounds_of(kernel, &evaluated.bodies);
        let mut shapes = Vec::new();
        for body in &evaluated.bodies {
            let mut body = body.clone();
            if let Some((cuts, length)) = &ends {
                for b in [false, true] {
                    let Some(cut) = cuts.end(b) else { continue };
                    let angle = value_in(self, &cut.angle, Unit::DEG)?;
                    if angle.abs() <= 1e-12 {
                        continue;
                    }
                    let Some(plane) = crate::features::derived_part::end_plane(
                        section.as_ref(),
                        b,
                        *length,
                        angle,
                        cut.tilt.unwrap_or_default(),
                    ) else {
                        continue;
                    };
                    let tool = crate::features::derived_part::end_tool(
                        kernel,
                        crate::ids::FeatureUid(0),
                        b,
                        *length,
                        plane,
                    )
                    .map_err(failed)?;
                    let output = kernel
                        .boolean(crate::kernel::BooleanOp::Cut, &[&body], &tool)
                        .map_err(failed)?;
                    if let Some(piece) = output.pieces.into_iter().find(|p| p.sources.contains(&0))
                    {
                        body = piece.shape;
                    }
                }
            }
            shapes.push(
                kernel
                    .transform_shape(&body, &world, None)
                    .map_err(failed)?,
            );
        }
        Ok(match shapes.len() {
            0 => None,
            1 => shapes.pop(),
            _ => Some(
                kernel
                    .compound(&shapes)
                    .map_err(|e| ApiError(e.to_string()))?,
            ),
        })
    }

    /// The `build_host` query (mitcad#394): the component BUILD places
    /// parts in, its name, its path from the root (uids, "" for the root)
    /// and the active component; paths and points of the BUILD commands
    /// are from and in it.
    pub(crate) fn host_json(&self) -> Value {
        let a = self.assembly();
        let host = self.build_host();
        let path = if host.is_root() {
            String::new()
        } else {
            a.paths_to(host)
                .first()
                .map(|p| path_text(p))
                .unwrap_or_default()
        };
        json!({
            "component": host,
            "name": a.name(host),
            "path": path,
            "active": a.active,
        })
    }

    /// The `build_parts` query.
    pub(crate) fn build_parts_json(&self) -> Value {
        let params = self.parameters();
        let angle = |id: ParamId| params.value(id);
        let parts: Vec<Value> = self
            .build_parts_info()
            .into_iter()
            .map(|info| {
                let values = info
                    .def
                    .as_ref()
                    .map(|d| crate::build::parts::part_values(d, params))
                    .unwrap_or_default();
                let config = info.config();
                let designation = info
                    .source
                    .as_ref()
                    .map(|s| crate::build::parts::designation(s, config.as_deref(), &values));
                let part_number = info
                    .source
                    .as_ref()
                    .map(|s| crate::build::parts::part_number(s, config.as_deref()));
                let bound = info.record.bound.iter().find_map(|uid| {
                    let entry = self.feature(*uid)?;
                    let FeatureDef::BoundLength(b) = &entry.def else {
                        return None;
                    };
                    Some(json!({
                        "feature": uid,
                        "parameter": params.name(b.parameter),
                        "to": {"occurrence": b.to.occurrence.to_string(), "geometry": b.to.geometry},
                        "mode": b.mode.as_str(),
                    }))
                });
                let (flip, rotate) = match info.record.joint.and_then(|j| self.feature(j)) {
                    Some(entry) => match &entry.def {
                        FeatureDef::Joint(j) => (
                            matches!(&j.a.geometry, GeomRef::Mount { name, .. } if name == "end_b"),
                            j.angle.and_then(|id| params.value(id)).unwrap_or(0.0),
                        ),
                        _ => (false, 0.0),
                    },
                    None => (false, 0.0),
                };
                let machining: Vec<Value> = self
                    .part_machining(&info)
                    .into_iter()
                    .filter(|(_, m)| m.connector.is_none())
                    .map(|(_, m)| {
                        json!({
                            "end": if m.at == "end_b" { "b" } else { "a" },
                            "hole": {"diameter": m.diameter, "depth": m.depth, "thread": m.thread},
                        })
                    })
                    .collect();
                json!({
                    "component": info.component,
                    "name": info.name,
                    "role": info.record.role.as_str(),
                    "source": info.record.source,
                    "part": info.part,
                    "joint": info.record.joint,
                    "designation": designation,
                    "supplier": info.source.as_ref().map(|s| s.supplier.clone()),
                    "part_number": part_number,
                    "config": config,
                    "length": info.length.map(|(_, l)| l),
                    "ends": info.ends().map(|e| ends_json(e, &angle)),
                    "bound": bound,
                    "flip": flip,
                    "rotate": rotate,
                    "tilt": info.record.tilt.unwrap_or(0.0),
                    "added_by": info.record.added_by,
                    "adds": info.record.adds,
                    "junction": info.record.junction,
                    "from": info.record.from.map(|f| f.as_str()),
                    "machining": machining,
                })
            })
            .collect();
        json!({"parts": parts})
    }

    /// The `build_junctions` query.
    pub(crate) fn build_junctions_json(&self, query: &JunctionsQuery) -> Result<Value, ApiError> {
        let only = match &query.components {
            Some(list) => Some(
                list.iter()
                    .map(|c| self.component_ref(c))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            None => None,
        };
        let active = self.host_placement();
        let side = |s: &crate::build::JunctionSide| {
            // The part's own axes at end A.
            let world = active.after(&self.path_transform(&s.occurrence));
            let end_a = self
                .part_info(s.component)
                .and_then(|i| i.mount("end_a").copied())
                .map_or([0.0; 3], |t| t.translation);
            let own = crate::build::frame(
                world.apply_point(end_a),
                world.apply_vector(crate::build::X),
                world.apply_vector(crate::build::Z),
            );
            json!({
                "component": s.component,
                "occurrence": path_text(&s.occurrence),
                "at": s.at,
                "frame": frame_json(&own),
                "at_frame": frame_json(&s.frame),
            })
        };
        let junctions: Vec<Value> = self
            .build_junctions(only.as_deref())
            .iter()
            .map(|j| {
                json!({
                    "id": j.id,
                    "kind": j.kind,
                    "a": side(&j.a),
                    "b": side(&j.b),
                    "angle": j.angle,
                    "systems": j.systems.iter().map(|s| s.as_ref().map(|s| s.label())).collect::<Vec<_>>(),
                    "space": {"x": j.space[0], "y": j.space[1], "z": j.space[2]},
                    "connectors": j.connectors,
                })
            })
            .collect();
        Ok(json!({"junctions": junctions}))
    }

    /// The `build_cut_list` query.
    pub(crate) fn build_cut_list_json(&self, query: &CutListQuery) -> Result<Value, ApiError> {
        let kerf = query.kerf.unwrap_or(CutListOptions::default().kerf);
        if !(kerf.is_finite() && kerf >= 0.0) {
            return Err(ApiError(format!(
                "the kerf must be zero or more, got {kerf}"
            )));
        }
        let list = self.build_cut_list(CutListOptions { kerf });
        let end = |e: &Option<(f64, crate::features::derived_part::Tilt)>| match e {
            Some((angle, tilt)) => json!({"angle": angle, "tilt": tilt.as_str()}),
            None => Value::Null,
        };
        let rows: Vec<Value> = list
            .rows
            .iter()
            .map(|r| {
                json!({
                    "source": r.source,
                    "designation": r.designation,
                    "supplier": r.supplier,
                    "part_number": r.part_number,
                    "config": r.config,
                    "length": r.length,
                    "ends": {"a": end(&r.ends[0]), "b": end(&r.ends[1])},
                    "machining": r.machining.iter().map(|m| json!({
                        "at": m.at, "diameter": m.diameter, "depth": m.depth,
                        "thread": m.thread, "connector": m.connector,
                    })).collect::<Vec<_>>(),
                    "features": r.features,
                    "quantity": r.quantity,
                    "mass": r.mass,
                    "components": r.components,
                })
            })
            .collect();
        let stock: Vec<Value> = list
            .stock
            .iter()
            .map(|s| {
                json!({
                    "source": s.source,
                    "designation": s.designation,
                    "stock_length": s.stock_length,
                    "pieces": s.pieces,
                    "bars": s.bars,
                    "needed": s.needed,
                    "offcut": s.offcut,
                    "too_long": s.too_long,
                })
            })
            .collect();
        let other: Vec<Value> = self.parts_list_json()["rows"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|row| {
                row["component"]
                    .as_str()
                    .and_then(|c| c.parse::<ComponentUid>().ok())
                    .is_none_or(|c| !list.cut.contains(&c))
            })
            .collect();
        Ok(json!({"rows": rows, "stock": stock, "kerf": kerf, "other": other}))
    }
}

/// A number for JSON, null when it is not finite.
fn finite(v: f64) -> Value {
    if v.is_finite() { json!(v) } else { Value::Null }
}
