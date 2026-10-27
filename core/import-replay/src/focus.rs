// SPDX-License-Identifier: MIT
//! Which occurrence an entity of another document is in (mitcad#196).
//!
//! A design that places a document more than once names one of its faces,
//! edges or bodies by the document and by the entity's names there: every
//! occurrence has it. What tells them apart is the transform the slot that
//! holds the reference carries, the placement of the occurrence the
//! selection was made in (`_f3d.context`, cm). The import's occurrences
//! inside the components filled from the other documents are placed where
//! the file has them, so the occurrence whose placement in the design at
//! the item is that transform is the one: its component is where the
//! lookups find the entity ([`mitcad_model::Document::set_import_focus`]),
//! and its path is where the links of the features that use it lead
//! (`sweeps.rs`, `links_of`).

use std::collections::HashMap;

use mitcad_dump::Mat4;
use mitcad_model::{BodyUid, ComponentUid, Kernel, OccurrenceUid, Transform};
use serde_json::Value;

use crate::Importer;
use crate::components;
use crate::references::Linker;

/// An occurrence of the design with the component it places and its
/// placement in the design: the path to it from the root.
type Instance = (ComponentUid, Vec<OccurrenceUid>, Transform);

/// An occurrence a reference is in: its component and the path to it.
pub type Hit = (ComponentUid, Vec<OccurrenceUid>);

/// A reference to an entity of another document with the placement of the
/// occurrence it is in.
struct Wanted {
    /// The reference object (`_f3d.object_id`).
    object: u64,
    /// The timeline index of the item that names it.
    index: i64,
    context: Transform,
}

/// How far apart two placements may be and still be the same: the files'
/// transforms carry rounding of that size.
const ROTATION: f64 = 1e-5;
const TRANSLATION: f64 = 5e-3;

/// Most occurrence paths looked at for one item.
const MAX_INSTANCES: usize = 50_000;

/// Whether two rigid placements are the same within the files' rounding.
fn same_pose(a: &Transform, b: &Transform) -> bool {
    (0..3).all(|r| {
        (0..3).all(|c| (a.linear[r][c] - b.linear[r][c]).abs() <= ROTATION)
            && (a.translation[r] - b.translation[r]).abs() <= TRANSLATION
    })
}

/// The references of a value that name an entity of another document and
/// carry a context transform.
fn wanted(value: &Value, index: i64, documents: &[String], out: &mut Vec<Wanted>) {
    match value {
        Value::Object(map) => {
            if let Some(f) = map.get("_f3d").and_then(Value::as_object)
                && f.get("document")
                    .and_then(Value::as_str)
                    .is_some_and(|g| documents.iter().any(|d| d == g))
                && let Some(object) = f.get("object_id").and_then(Value::as_u64)
                && let Some(matrix) = f
                    .get("context")
                    .and_then(|c| serde_json::from_value::<Mat4>(c.clone()).ok())
                && let Some(context) = components::transform(&matrix)
            {
                out.push(Wanted {
                    object,
                    index,
                    context,
                });
            }
            for v in map.values() {
                wanted(v, index, documents, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                wanted(v, index, documents, out);
            }
        }
        _ => {}
    }
}

impl<K: Kernel> Importer<'_, K> {
    /// Finds the occurrences each reference to an entity of another document
    /// is in (the module documentation), once the components are filled
    /// and the scopes set: the document's focus gets their components, and
    /// `focus_paths` their occurrence paths. An occurrence inside another
    /// that sits at the same placement (a seal in a knob) is at the same
    /// pose: all of them are kept, shallowest first. Occurrences that share
    /// a component but are named by different references of one item (a
    /// plane between faces of two occurrences of one part) get a component
    /// each ([`mitcad_model::Document::make_unique`]), so that the bodies
    /// the references are found on tell the occurrences apart.
    pub(crate) fn find_contexts(
        &mut self,
        scopes: &[(String, Vec<ComponentUid>)],
        linker: &mut Linker,
    ) {
        let mut scopes = scopes.to_vec();
        // The GUIDs the design names the other documents by.
        let documents: Vec<String> = self
            .report
            .components
            .inserted
            .iter()
            .filter_map(|i| i.document_guid.as_deref().map(str::to_lowercase))
            .collect();
        let mut all: Vec<Wanted> = Vec::new();
        for (position, item) in self.dump.timeline_items().iter().enumerate() {
            let Some(detail) = item
                .detail
                .as_ref()
                .and_then(|d| serde_json::to_value(d).ok())
            else {
                continue;
            };
            wanted(
                &detail,
                item.index.unwrap_or(position as i64),
                &documents,
                &mut all,
            );
        }
        if all.is_empty() {
            return;
        }
        let mut focus: HashMap<u64, Vec<ComponentUid>> = HashMap::new();
        // The occurrence paths of the design at each item that names
        // references, with their placements: once per item.
        let mut indices: Vec<i64> = all.iter().map(|w| w.index).collect();
        indices.sort_unstable();
        indices.dedup();
        for index in indices {
            let mut instances = self.instances_around(index);
            // The occurrences this item's references are in, by what is
            // named first: two named occurrences of one component that
            // are not the same occurrence.
            let mut seen: Vec<Hit> = Vec::new();
            for w in all.iter().filter(|w| w.index == index) {
                let mut hits = self.hits(&instances, &w.context);
                let Some((component, path)) = hits.first().cloned() else {
                    self.trace_no_hit(&instances, w, index);
                    continue;
                };
                // The later one gets a copy of its own.
                if seen.iter().any(|(c, p)| *c == component && *p != path)
                    && let Some(leaf) = path.last().copied()
                    && let Ok((_, copy)) = self.doc.make_unique(leaf)
                {
                    linker.register_copy(&copy, &mut self.report);
                    // The copy is in the scope of the document it copies
                    // (one placed in a filled component has it already).
                    if let Some(made) = self.doc.assembly().occurrence(leaf).map(|o| o.component) {
                        for (_, roots) in &mut scopes {
                            if roots.contains(&component) {
                                roots.push(made);
                            }
                        }
                    }
                    self.doc.set_import_scopes(scopes.clone());
                    instances = self.instances_around(index);
                    hits = self.hits(&instances, &w.context);
                }
                if let Some(first) = hits.first() {
                    seen.push(first.clone());
                }
                let mut components: Vec<ComponentUid> = Vec::new();
                for (c, _) in &hits {
                    if !components.contains(c) {
                        components.push(*c);
                    }
                }
                if crate::tracing() {
                    eprintln!(
                        "import: reference {} (item {index}) is in {:?}",
                        w.object,
                        hits.iter()
                            .map(|(c, p)| (self.doc.assembly().name(*c), p.len()))
                            .collect::<Vec<_>>()
                    );
                }
                focus.insert(w.object, components);
                self.focus_paths.insert(w.object, hits);
            }
        }
        self.doc.set_import_focus(focus);
        // The copies made are inside the scopes too.
        self.doc.set_import_scopes(scopes);
    }

    /// The instances at an item: where the occurrences are before it, and
    /// after it (an assembly item names them where it puts them).
    fn instances_around(&self, index: i64) -> Vec<Instance> {
        let mut instances = self.instances_at(index);
        instances.extend(self.instances_at(index + 1));
        instances
    }

    /// The occurrences inside the filled components that are at `context`,
    /// shallowest first.
    fn hits(&self, instances: &[Instance], context: &Transform) -> Vec<Hit> {
        instances
            .iter()
            .filter(|(component, _, world)| {
                self.doc.in_import_scope(*component) && same_pose(world, context)
            })
            .map(|(component, path, _)| (*component, path.clone()))
            .collect()
    }

    /// Says (`MITCAD_IMPORT_TRACE`) that no occurrence is where a reference
    /// says, and which is nearest.
    fn trace_no_hit(&self, instances: &[Instance], w: &Wanted, index: i64) {
        if !crate::tracing() {
            return;
        }
        let distance = |t: &Transform| {
            (0..3)
                .map(|i| (t.translation[i] - w.context.translation[i]).powi(2))
                .sum::<f64>()
        };
        let near = instances
            .iter()
            .filter(|(c, _, _)| self.doc.in_import_scope(*c))
            .min_by(|a, b| distance(&a.2).total_cmp(&distance(&b.2)));
        eprintln!(
            "import: no occurrence is at {:?} {:?} where item {index} names an entity of another document (reference {}); the nearest is {:?}",
            w.context.translation,
            w.context.linear,
            w.object,
            near.map(|(c, p, t)| (
                self.doc.assembly().name(*c),
                p.len(),
                t.translation,
                t.linear
            ))
        );
    }

    /// Notes the occurrence path the body found for a reference is in
    /// (`links_of` takes the paths of the features' links from it): of the
    /// occurrences at the reference's placement, the one of the body's
    /// component.
    pub(crate) fn hint_path(&self, fp: &mitcad_dump::Fingerprint, body: BodyUid) {
        let Some(hits) = fp
            .f3d
            .as_ref()
            .and_then(|f| f.object_id)
            .and_then(|o| self.focus_paths.get(&o))
        else {
            return;
        };
        let owner = self.doc.body_component(body);
        let Some((_, path)) = hits
            .iter()
            .find(|(c, _)| Some(*c) == owner)
            .or(hits.first())
        else {
            return;
        };
        if let Ok(mut paths) = self.link_paths.lock() {
            paths.insert(body, path.clone());
        }
    }

    /// The occurrence path a body of another document was found in by a
    /// reference of the file ([`Importer::hint_path`]).
    pub(crate) fn hinted_path(&self, body: BodyUid) -> Option<Vec<OccurrenceUid>> {
        self.link_paths.lock().ok()?.get(&body).cloned()
    }

    /// Every occurrence path of the design with the component it leads to
    /// and its placement in the design at the item `index`: the design's
    /// own occurrences where the file's captured positions have them then,
    /// those inside the filled components where they are.
    fn instances_at(&self, index: i64) -> Vec<Instance> {
        let mut out = Vec::new();
        let mut path = Vec::new();
        self.instances_in(
            ComponentUid::ROOT,
            Transform::IDENTITY,
            index,
            &mut path,
            &mut out,
        );
        out
    }

    fn instances_in(
        &self,
        component: ComponentUid,
        world: Transform,
        index: i64,
        path: &mut Vec<OccurrenceUid>,
        out: &mut Vec<Instance>,
    ) {
        if path.len() > 32 || out.len() >= MAX_INSTANCES {
            return;
        }
        let a = self.doc.assembly();
        for o in a.children(component) {
            // Where the file has it at that item: its captured placements
            // (the design's own occurrences and those inside the copies), else
            // where it is.
            let local = self.captures.placement_at(o.uid, index).unwrap_or_else(|| {
                if self.doc.in_import_scope(o.parent) {
                    self.doc.placement(o.uid).unwrap_or(o.transform)
                } else {
                    o.transform
                }
            });
            let placed = world.after(&local);
            path.push(o.uid);
            out.push((o.component, path.clone(), placed));
            self.instances_in(o.component, placed, index, path, out);
            path.pop();
        }
    }
}
