// SPDX-License-Identifier: MIT
//! Queries of mount frames and of placing a part by one (mitcad#525;
//! `commands.md`, "Mount frames"): the frames of an occurrence's part, a
//! placement's preview and its shape. Placing is `insert_component` with
//! `mount` (`components.rs`).

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::ApiError;
use super::libraries::LibraryInput;
use mitcad_model::assembly::{matrix_rows, matrix4};
use mitcad_model::features::OccurrencePath;
use mitcad_model::features::joint::MountSpec;
use mitcad_model::internal::joints::{find_path_from, frame_parts, path_component};
use mitcad_model::kernel::Kernel;
use mitcad_model::transform::Transform;
use mitcad_model::{Document, Inserted, MountInput, MountPlan, MountSource};

/// `mount_frames`: the frames of an occurrence's part (a path from the
/// active component; empty: the active component's own).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MountFramesQuery {
    #[serde(default)]
    occurrence: String,
}

/// `mount_preview`: `insert_component`'s fields; `link`, `name` and
/// `transform` are taken and not used.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MountPreviewQuery {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    library: Option<LibraryInput>,
    #[serde(default)]
    base: Option<String>,
    #[serde(default)]
    mount: Option<MountInput>,
    #[serde(default)]
    #[allow(dead_code)]
    link: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    name: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    transform: Option<Value>,
}

fn frame_json(frame: &Transform) -> Value {
    let [origin, x_axis, y_axis, z_axis] = frame_parts(frame);
    json!({"origin": origin, "x_axis": x_axis, "y_axis": y_axis, "z_axis": z_axis})
}

/// A mount as the answers give it: `name`, `kind`, `joint` and a grid's
/// or foot's `pitch`, `cells`, `footprint`.
pub(crate) fn spec_json(spec: &MountSpec) -> Value {
    let mut value = json!({"name": spec.name, "kind": spec.kind.as_str(),
                           "joint": spec.joint.as_str()});
    if let Some(pitch) = spec.pitch {
        value["pitch"] = json!(pitch);
    }
    if let Some(cells) = spec.cells {
        value["cells"] = json!(cells);
    }
    if let Some(footprint) = spec.footprint {
        value["footprint"] = json!(footprint);
    }
    value
}

/// A plan's fields: the part's frame, its kind and joint, the target, where
/// the frame goes (`at`, in the design), the new occurrence's placement
/// (rows, in the active component) and in the design (`world`), and a grid
/// placement's cells.
pub(crate) fn plan_fields<K: Kernel>(
    doc: &Document<K>,
    plan: &MountPlan,
    host: &Transform,
) -> Map<String, Value> {
    let a = doc.assembly();
    let mut out = Map::new();
    out.insert("frame".to_owned(), json!(plan.frame));
    out.insert("kind".to_owned(), json!(plan.kind.as_str()));
    out.insert("joint".to_owned(), json!(plan.joint.as_str()));
    out.insert(
        "target".to_owned(),
        json!({"occurrence": a.path_name(&plan.target.occurrence.0),
               "path": plan.target.occurrence,
               "geometry": plan.target.geometry,
               "frame_override": plan.target.frame_override,
               "name": plan.target_name}),
    );
    out.insert("at".to_owned(), frame_json(&host.after(&plan.at)));
    out.insert("placement".to_owned(), json!(matrix_rows(&plan.placement)));
    out.insert(
        "world".to_owned(),
        json!(matrix4(&host.after(&plan.placement))),
    );
    let grid = match &plan.grid {
        Some(g) => {
            let covers: Vec<[u32; 2]> = (g.cell[0]..g.cell[0] + g.footprint[0])
                .flat_map(|i| (g.cell[1]..g.cell[1] + g.footprint[1]).map(move |j| [i, j]))
                .collect();
            json!({"feature": g.feature, "occurrence": a.path_name(&g.path),
                   "path": OccurrencePath(g.path.clone()), "pitch": g.pitch, "cells": g.cells,
                   "cell": g.cell, "footprint": g.footprint, "turns": g.turns,
                   "covers": covers})
        }
        None => Value::Null,
    };
    out.insert("grid".to_owned(), grid);
    out
}

/// `insert_component`'s answer of what it made: the new component's mount
/// frames and, placed by one, the joint and the plan.
pub(crate) fn inserted_json<K: Kernel>(doc: &Document<K>, inserted: &Inserted) -> Value {
    let a = doc.assembly();
    let frames: Vec<Value> = inserted
        .frames
        .iter()
        .map(|f| {
            let mut path = vec![inserted.occurrence];
            path.extend(f.path.iter().copied());
            let mut value = spec_json(&f.spec);
            value["feature"] = json!(f.feature);
            value["occurrence"] = json!(a.path_name(&path));
            value
        })
        .collect();
    let mut value = json!({
        "component": inserted.component,
        "occurrence": inserted.occurrence,
        "name": a.name(inserted.component),
        "bodies": doc.component_bodies(inserted.component),
        "frames": frames,
        "joint": inserted.joint,
        "joint_name": inserted.joint.and_then(|j| doc.feature(j)).map(|f| f.name.clone()),
    });
    if let Some(plan) = &inserted.plan {
        value["mount"] = Value::Object(plan_fields(doc, plan, &doc.host_world()));
    }
    // Inserted with options (mitcad#801).
    if doc.component_options(inserted.component).is_some() {
        value["moves"] = json!(
            inserted
                .moves
                .iter()
                .map(mitcad_model::part_options::OptionMove::json)
                .collect::<Vec<_>>()
        );
        value["refused"] = inserted
            .broken
            .as_ref()
            .map_or(Value::Null, mitcad_model::part_options::BrokenRule::json);
    }
    value
}

/// What a preview reads: a library part or a file (with the folder of a
/// relative path), and the placement.
struct PreviewParts {
    part: Option<mitcad_model::LibraryPart>,
    path: String,
    base: Option<PathBuf>,
    mount: Option<MountInput>,
}

impl PreviewParts {
    fn source(&self) -> MountSource<'_> {
        match &self.part {
            Some(part) => MountSource::Library(part),
            None => MountSource::File {
                path: &self.path,
                base: self.base.as_deref(),
            },
        }
    }
}

impl MountPreviewQuery {
    fn parts(self) -> Result<PreviewParts, ApiError> {
        let (part, path) = match (self.path, self.library) {
            (Some(path), None) => (None, path),
            (None, Some(library)) => (Some(library.part()), String::new()),
            _ => {
                return Err(ApiError(
                    "a mount preview needs a path or a library part, not both".to_owned(),
                ));
            }
        };
        Ok(PreviewParts {
            part,
            path,
            base: self.base.map(PathBuf::from),
            mount: self.mount,
        })
    }
}

/// The document's methods of this module used across the model, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(crate) trait MountsCrateOps<K: Kernel> {
    /// `mount_frames` (see [`MountFramesQuery`]).
    fn mount_frames_json(&self, query: &MountFramesQuery) -> Result<Value, ApiError>;

    /// `mount_preview` (see [`MountPreviewQuery`]): the part's name and
    /// mount frames (in its own coordinates) and, with `mount`, what the
    /// placement would do, or `problems` with why it cannot be.
    fn mount_preview_json(&self, query: MountPreviewQuery) -> Result<Value, ApiError>;

    /// `analysis_shape` with `{"shape": "mount_preview", ...}`: the part's
    /// bodies where the placement puts them, in the design's coordinates.
    fn mount_preview_shape(&self, fields: Map<String, Value>)
    -> Result<Option<K::Shape>, ApiError>;
}

impl<K: Kernel> MountsCrateOps<K> for Document<K> {
    fn mount_frames_json(&self, query: &MountFramesQuery) -> Result<Value, ApiError> {
        let a = self.assembly();
        let host = a.active;
        let path = find_path_from(a, host, &query.occurrence).ok_or_else(|| {
            ApiError(format!(
                "no occurrence {} in {}",
                query.occurrence,
                a.name(host)
            ))
        })?;
        let component = path_component(a, host, &path).map_err(ApiError)?;
        let world = self.host_world().after(&self.path_transform(&path));
        let frames: Vec<Value> = self
            .frames_under(component)
            .iter()
            .map(|f| {
                let mut full = path.clone();
                full.extend(f.frame.path.iter().copied());
                let mut value = spec_json(&f.frame.spec);
                value["feature"] = json!(f.frame.feature);
                value["name_of_feature"] = json!(self.feature(f.frame.feature).map(|e| &e.name));
                value["occurrence"] = json!(a.path_name(&full));
                value["path"] = json!(OccurrencePath(full));
                value["frame"] = frame_json(&world.after(&f.at));
                value
            })
            .collect();
        Ok(json!({"component": component, "frames": frames}))
    }

    fn mount_preview_json(&self, query: MountPreviewQuery) -> Result<Value, ApiError> {
        let parts = query.parts()?;
        let preview = self.mount_preview(parts.source(), parts.mount.as_ref())?;
        let frames: Vec<Value> = preview
            .frames
            .iter()
            .map(|(spec, at)| {
                let mut value = spec_json(spec);
                value["frame"] = frame_json(at);
                value
            })
            .collect();
        let mut value = json!({"name": preview.name, "frames": frames, "problems": []});
        match &preview.plan {
            None => {}
            Some(Err(problem)) => value["problems"] = json!([problem]),
            Some(Ok(plan)) => {
                if let Value::Object(fields) = &mut value {
                    fields.extend(plan_fields(self, plan, &preview.host));
                }
            }
        }
        Ok(value)
    }

    fn mount_preview_shape(
        &self,
        fields: Map<String, Value>,
    ) -> Result<Option<K::Shape>, ApiError> {
        let query: MountPreviewQuery = super::parse(Value::Object(fields), "mount_preview")?;
        let parts = query.parts()?;
        let Some(mount) = &parts.mount else {
            return Ok(None);
        };
        let mut shapes = self.mount_preview_shapes(parts.source(), mount)?;
        Ok(match shapes.len() {
            0 => None,
            1 => shapes.pop(),
            _ => Some(
                self.kernel()
                    .compound(&shapes)
                    .map_err(|e| ApiError(e.to_string()))?,
            ),
        })
    }
}
