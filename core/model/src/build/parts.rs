// SPDX-License-Identifier: MIT
//! What the BUILD rules read of a placed part: its record, derived part,
//! source, metadata, mount frames and length at the timeline marker, and
//! the designation, supplier and part number a list gives it.

use std::collections::{BTreeMap, BTreeSet};

use super::{BuildMeta, BuildRecord, LibrarySourceDef, SourceId};
use crate::document::{DocState, Document};
use crate::features::derived_part::{DerivedPartDef, Ends};
use crate::features::{FeatureDef, ValueInput};
use crate::ids::{ComponentUid, FeatureUid, OccurrenceUid};
use crate::kernel::{BoundingBox, Kernel};
use crate::parameters::{ParamId, Parameters};
use crate::topo::FaceName;
use crate::transform::Transform;

/// A BUILD part as the rules see it.
#[derive(Debug, Clone)]
pub(crate) struct PartInfo {
    pub component: ComponentUid,
    pub name: String,
    pub record: BuildRecord,
    pub part: Option<FeatureUid>,
    pub def: Option<DerivedPartDef>,
    pub source: Option<LibrarySourceDef>,
    pub meta: BuildMeta,
    /// The mount frames at the marker, in the part's coordinates.
    pub mounts: BTreeMap<String, Transform>,
    /// The design's parameter that sets its length, and its value.
    pub length: Option<(ParamId, f64)>,
}

impl PartInfo {
    pub fn mount(&self, name: &str) -> Option<&Transform> {
        self.mounts.get(name)
    }

    /// The slot frames, by name.
    pub fn slots(&self) -> Vec<(&String, &Transform)> {
        self.mounts
            .iter()
            .filter(|(name, _)| self.frame_kind(name) == "slot")
            .collect()
    }

    /// The frames a part goes on a side at (mitcad#478): the slot frames,
    /// and a member's `face` frames on the sides that have no slot frame.
    pub fn sides(&self) -> Vec<(&String, &Transform)> {
        let mut out = self.slots();
        if self.meta.role != super::BuildRole::Profile {
            return out;
        }
        let faces: Vec<(&String, &Transform)> = self
            .mounts
            .iter()
            .filter(|(name, _)| self.frame_kind(name) == "face")
            .collect();
        for (name, t) in faces {
            let z = super::z_axis(t);
            if !out
                .iter()
                .any(|(_, s)| super::dot(super::z_axis(s), z) > 1.0 - 1e-6)
            {
                out.push((name, t));
            }
        }
        out
    }

    /// Whether a frame is a side frame (a slot, or a member's face).
    pub fn is_side(&self, name: &str) -> bool {
        self.sides().iter().any(|(n, _)| *n == name)
    }

    /// Whether the part is a timber member (mitcad#478).
    pub fn is_timber(&self) -> bool {
        self.meta.is_timber()
    }

    /// A frame's kind: the metadata's, else guessed from its name
    /// (`end_a`, `slot_x+_1`).
    pub fn frame_kind(&self, name: &str) -> String {
        if let Some(frame) = self.meta.frame(name).filter(|f| !f.kind.is_empty()) {
            return frame.kind.clone();
        }
        if name == "end_a" || name == "end_b" {
            "end".to_owned()
        } else if name.starts_with("slot") {
            "slot".to_owned()
        } else {
            String::new()
        }
    }

    pub fn is_cut_to_length(&self) -> bool {
        self.meta.length.is_some()
    }

    /// The configuration row it is made in.
    pub fn config(&self) -> Option<String> {
        self.def.as_ref().and_then(|d| d.config.clone())
    }

    /// The derived part's end cuts.
    pub fn ends(&self) -> Option<&Ends> {
        self.def.as_ref().map(|d| &d.ends)
    }
}

impl<K: Kernel> Document<K> {
    /// The BUILD part of a component, if it is one.
    pub(crate) fn part_info(&self, component: ComponentUid) -> Option<PartInfo> {
        let state = self.state();
        let def = state.assembly.component(component)?;
        let record = def.build.clone()?;
        let part_def = record.part.and_then(|uid| match &state.entry(uid)?.def {
            FeatureDef::DerivedPart(d) => Some(d.clone()),
            _ => None,
        });
        let source = record
            .source
            .and_then(|id| state.library_sources.get(id).cloned());
        let meta = source
            .as_ref()
            .and_then(|s| s.build.as_ref())
            .and_then(|b| {
                let row = part_def.as_ref().and_then(|d| d.config.as_deref());
                BuildMeta::read_row(b, row).ok()
            })
            .unwrap_or_else(|| BuildMeta {
                role: record.role,
                ..BuildMeta::default()
            });
        let mounts = record
            .part
            .and_then(|uid| self.last_result().result(uid))
            .filter(|r| r.status.is_ok())
            .and_then(|r| r.output.as_ref())
            .and_then(|o| o.mounts.as_ref())
            .map(|m| (**m).clone())
            .unwrap_or_default();
        let length = meta.length.as_ref().and_then(|l| {
            let id = *part_def.as_ref()?.parameters.get(&l.parameter)?;
            Some((id, self.parameters().value(id)?))
        });
        let part = record.part.filter(|_| part_def.is_some());
        Some(PartInfo {
            component,
            name: def.name.clone(),
            record,
            part,
            def: part_def,
            source,
            meta,
            mounts,
            length,
        })
    }

    /// Every BUILD part, in the order the components were made.
    pub(crate) fn build_parts_info(&self) -> Vec<PartInfo> {
        self.state()
            .assembly
            .components
            .iter()
            .filter_map(|c| self.part_info(c.uid))
            .collect()
    }

    /// The box of a part's bodies at the marker, in its coordinates.
    pub(crate) fn part_bounds(&self, component: ComponentUid) -> Option<BoundingBox> {
        let mut all: Option<BoundingBox> = None;
        for body in self.component_bodies(component) {
            let shape = self.body_shape(body)?;
            let Ok(Some(b)) = self.kernel().bounding_box(shape) else {
                continue;
            };
            all = Some(match all {
                Some(a) => a.union(&b),
                None => b,
            });
        }
        all
    }

    /// The component BUILD places parts in (mitcad#394): the active one
    /// when its policy takes components, else the nearest one up from it
    /// that takes them and can be edited, as the design policy puts a new
    /// component beside a part ("goes beside it", mitcad#292). In a new
    /// design (Part1 active) that is the root. The active component when
    /// none up takes components.
    pub(crate) fn build_host(&self) -> ComponentUid {
        // A timber frame's own component while it drives its fasteners
        // (mitcad#522).
        if let Some(host) = self
            .build_host_override()
            .filter(|c| self.assembly().exists(*c))
        {
            return host;
        }
        let a = self.assembly();
        let active = a.active;
        if a.policy(active).components {
            return active;
        }
        let Ok(path) = crate::links::first_path(a, active, None) else {
            return active;
        };
        (0..path.len())
            .rev()
            .find_map(|i| {
                let parent = a.occurrence(path[i])?.parent;
                (a.policy(parent).components && self.state().editable(parent).is_ok())
                    .then_some(parent)
            })
            .unwrap_or(active)
    }

    /// The path of a component from the BUILD host
    /// ([`Document::build_host`]): its placement there is the path's.
    pub(crate) fn placed_in_host(&self, component: ComponentUid) -> Option<Vec<OccurrenceUid>> {
        let a = self.assembly();
        let host = self.build_host();
        let mut found = None;
        let mut todo: Vec<(ComponentUid, Vec<OccurrenceUid>)> = vec![(host, Vec::new())];
        while let Some((c, path)) = todo.pop() {
            if c == component && !path.is_empty() {
                found = Some(path);
                break;
            }
            if path.len() > 32 {
                continue;
            }
            for o in a.children(c) {
                let mut next = path.clone();
                next.push(o.uid);
                todo.push((o.component, next));
            }
        }
        found
    }

    /// The BUILD host's placement in the design.
    pub(crate) fn host_placement(&self) -> Transform {
        let a = self.assembly();
        let host = self.build_host();
        if host.is_root() {
            return Transform::IDENTITY;
        }
        a.paths_to(host)
            .first()
            .map_or(Transform::IDENTITY, |path| self.path_transform(path))
    }
}

/// The designation of a part made from a source: the row's own, else the
/// library's pattern with `{standard}`, `{name}`, `{config}`, the row's
/// selectors and the part's parameters (`{L}`, in millimetres) replaced;
/// without a pattern the name and the row.
pub(crate) fn designation(
    source: &LibrarySourceDef,
    config: Option<&str>,
    values: &BTreeMap<String, f64>,
) -> String {
    let state = source.design.state().ok();
    let row = state.as_ref().and_then(|s| {
        let table = &s.configurations;
        match config {
            Some(name) => table.row(name).cloned(),
            None => table.default_row().cloned(),
        }
    });
    if let Some(own) = row.as_ref().and_then(|r| r.designation.clone()) {
        return own;
    }
    let pattern = source.library.designation.trim();
    let pattern = if pattern.is_empty() {
        "{name} {config}"
    } else {
        pattern
    };
    let mut text = pattern
        .replace("{standard}", &source.standard)
        .replace("{name}", &source.label())
        .replace("{config}", row.as_ref().map_or("", |r| r.name.as_str()));
    if let Some(row) = &row {
        for (selector, value) in &row.select {
            text = text.replace(&format!("{{{selector}}}"), value);
        }
    }
    for (name, value) in values {
        text = text.replace(&format!("{{{name}}}"), &super::round(*value));
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The part number of a part made in a row: the row's own, else the
/// source's.
pub(crate) fn part_number(source: &LibrarySourceDef, config: Option<&str>) -> String {
    let own = source.design.state().ok().and_then(|s| {
        let table = &s.configurations;
        let row = match config {
            Some(name) => table.row(name),
            None => table.default_row(),
        };
        row.and_then(|r| r.part_number.clone())
    });
    own.unwrap_or_else(|| source.part_number.clone())
}

/// A derived part's parameter values in millimetres (radians), by name.
pub(crate) fn part_values(def: &DerivedPartDef, params: &Parameters) -> BTreeMap<String, f64> {
    def.parameters
        .iter()
        .filter_map(|(name, id)| Some((name.clone(), params.value(*id)?)))
        .collect()
}

/// A definition in command form, its values by their parameters' names.
pub(crate) fn as_input(def: &FeatureDef, params: &Parameters) -> FeatureDef<ValueInput> {
    def.map_params(&mut |_, id: &ParamId| Ok::<_, ()>(ValueInput::Name(params.name(*id))))
        .expect("names never fail")
}

/// The source's state in a row with values set, as the derived part makes
/// it (for frames and sizes before the part exists).
pub(crate) fn source_state(
    source: &LibrarySourceDef,
    config: Option<&str>,
    values: &[(String, f64)],
) -> Result<DocState, String> {
    crate::features::derived_part::configured(source, config, values)
}

/// The source's default value of a parameter, in its row.
pub(crate) fn default_value(
    source: &LibrarySourceDef,
    config: Option<&str>,
    name: &str,
) -> Option<f64> {
    let state = source_state(source, config, &[]).ok()?;
    let id = state.parameters.find(name)?;
    state.parameters.value(id)
}

/// The source of an id, or why there is none.
pub(crate) fn source_of(state: &DocState, id: SourceId) -> Result<LibrarySourceDef, String> {
    state
        .library_sources
        .get(id)
        .cloned()
        .ok_or_else(|| format!("library source {id} is not in the design"))
}

/// What a list says of a BUILD part: its source's library record, its
/// designation and its article number.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListEntry {
    pub library: crate::library::LibraryRef,
    pub designation: String,
    pub part_number: String,
}

impl<K: Kernel> Document<K> {
    /// A BUILD part's entry in the parts list, if the component is one made
    /// from a source.
    pub(crate) fn build_list_entry(&self, component: ComponentUid) -> Option<ListEntry> {
        let info = self.part_info(component)?;
        let source = info.source.as_ref()?;
        let def = info.def.as_ref()?;
        let config = def.config.as_deref();
        let values = part_values(def, self.parameters());
        let mut library = source.library.clone();
        library.config = def.config.clone();
        let designation = designation(source, config, &values);
        library.designation = designation.clone();
        let part_number = part_number(source, config);
        library.part_number = part_number.clone();
        Some(ListEntry {
            library,
            designation,
            part_number,
        })
    }

    /// Moves library sources to other versions (`update_library_parts` with
    /// `source`), as one undo step: each takes that version's design and
    /// `build`, and every derived part made from it recomputes. Returns one
    /// line per source.
    pub fn update_library_sources(
        &mut self,
        changes: &[(SourceId, Option<String>)],
    ) -> Result<Vec<String>, crate::document::ModelError> {
        use super::place::refuse;
        if changes.is_empty() {
            return Err(refuse("no library sources to change"));
        }
        let resolver = self
            .link_resolver()
            .ok_or_else(|| refuse("no component libraries are set up here"))?;
        let mut planned = Vec::new();
        for (id, rev) in changes {
            let old = source_of(self.state(), *id).map_err(refuse)?;
            let read = resolver
                .library_source(&crate::library::LibraryRequest {
                    library: old.library.library.clone(),
                    url: old.library.url.clone(),
                    rev: rev.clone().unwrap_or_else(|| old.library.rev.clone()),
                    component: old.library.component.clone(),
                })
                .map_err(|e| refuse(format!("{}: {e}", old.label())))?;
            let new = LibrarySourceDef::from_library(*id, &read).map_err(refuse)?;
            let line = if new.library.rev == old.library.rev && new.design == old.design {
                format!("{}: unchanged", old.label())
            } else {
                format!(
                    "{}: version {} -> {}",
                    old.label(),
                    old.library.version_text(),
                    new.library.version_text()
                )
            };
            planned.push((new, line));
        }
        let label = match planned.as_slice() {
            [(new, _)] => {
                let version = if new.library.label.is_empty() {
                    new.library.rev.chars().take(7).collect()
                } else {
                    new.library.label.clone()
                };
                format!("Update {} to {version}", new.label())
            }
            _ => "Update Library Parts".to_owned(),
        };
        let lines: Vec<String> = planned.iter().map(|(_, l)| l.clone()).collect();
        self.apply(|state| {
            for (new, _) in planned {
                if let Some(source) = state.library_sources.get_mut(new.id) {
                    *source = new;
                }
            }
            Ok((label, lines))
        })
    }
}

/// Gives a derived part's bodies their source's appearance and material
/// (mitcad#478: a timber member shows its wood), for a part whose library
/// block names a `material`: when the source's bodies
/// that set one all set the same, every body the part makes takes it (the
/// first `count`, at least one), and a document appearance of the source
/// is copied into the design when it has none of that id.
pub(crate) fn inherit_appearance(
    state: &mut DocState,
    part: FeatureUid,
    source: &LibrarySourceDef,
) {
    // Only parts whose library block names a material (timber, sheets,
    // their metal parts): slot profiles look as they did.
    if source
        .build
        .as_ref()
        .is_none_or(|b| b.get("material").is_none())
    {
        return;
    }
    let Ok(design) = source.design.state() else {
        return;
    };
    let shown: Vec<&crate::document::BodyAttributes> = design
        .body_attributes
        .values()
        .filter(|a| a.visible)
        .collect();
    let same = |pick: &dyn Fn(&crate::document::BodyAttributes) -> Option<String>| {
        let values: Vec<String> = shown.iter().filter_map(|a| pick(a)).collect();
        match values.first() {
            Some(first) if values.iter().all(|v| v == first) => Some(first.clone()),
            _ => None,
        }
    };
    let appearance = same(&|a| a.appearance.clone());
    let material = same(&|a| a.material.clone());
    // The ends' look (mitcad#484: a timber member's end grain): the
    // appearance the source's bodies give the caps of their extrusion
    // (`<F>:start(...)`, `<F>:end(...)`, the faces at z = 0 and L), when
    // they give one, goes to the part's end faces, cut or not.
    let caps: BTreeSet<&String> = shown
        .iter()
        .flat_map(|a| a.face_appearances.iter())
        .filter(|(face, _)| {
            face.parse::<FaceName>()
                .is_ok_and(|name| name.role == "start" || name.role == "end")
        })
        .map(|(_, id)| id)
        .collect();
    let ends = match (caps.len(), source.build.as_ref()) {
        (1, Some(build)) if build.get("length").is_some() => caps.first().map(|id| (*id).clone()),
        _ => None,
    };
    if appearance.is_none() && material.is_none() && ends.is_none() {
        return;
    }
    for id in appearance.iter().chain(ends.iter()) {
        if crate::appearance::library_appearance(id).is_none()
            && !state.appearances.iter().any(|a| &a.id == id)
            && let Some(own) = design.appearances.iter().find(|a| &a.id == id)
        {
            state.appearances.push(own.clone());
        }
    }
    // A material of the library design's own comes with it (mitcad#640).
    let material = match material {
        Some(id) if design.materials.iter().any(|m| m.id == id) => {
            let mapped = state.copy_materials(&design, &BTreeSet::from([id.clone()]));
            Some(mapped.get(&id).cloned().unwrap_or(id))
        }
        other => other,
    };
    let count = shown.len().max(1);
    for i in 0..count {
        let entry = state
            .body_attributes
            .entry(crate::ids::BodyUid::new(part, i as u32))
            .or_default();
        if entry.appearance.is_none() {
            entry.appearance = appearance.clone();
        }
        if entry.material.is_none() {
            entry.material = material.clone();
        }
        if let Some(id) = &ends {
            for end in ["end_a", "end_b"] {
                entry
                    .face_appearances
                    .entry(FaceName::new(part, end, None).to_string())
                    .or_insert_with(|| id.clone());
            }
        }
    }
}
