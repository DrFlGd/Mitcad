// SPDX-License-Identifier: MIT
//! Components of other documents filled from those documents (mitcad#196).
//!
//! The replay places each occurrence of a component of another document as
//! an occurrence of an empty component standing for it
//! ([`crate::components`]). When the caller has imported the other
//! document from its own file (`mitcad-ffi`: a local reference folder, a
//! file the user chose), [`link_references`] copies it with its history
//! into that component once the replay is done, so that every occurrence
//! shows it where the file places it. The replay itself does not see the
//! copied bodies: it matches the file's own history, which keeps none of
//! them.
//!
//! The captured positions of occurrences inside those components (left
//! out of the replay: the components were empty) then place the copied
//! occurrences, as `capture_position` features of those components at the
//! end of the timeline: each path's levels inside the other document are
//! named by the GUIDs its occurrences carry. A copied component is shared
//! by all occurrences of the other document's component, so an occurrence
//! inside it takes its last captured placement, where the paths that place
//! it then agree.

use std::collections::{BTreeMap, HashMap};

use mitcad_model::assembly::{inverse, matrix_rows};
use mitcad_model::{
    ComponentUid, Document, FeatureDef, FeatureUid, Kernel, OccurrenceUid, Transform, ValueInput,
};
use serde_json::json;

use crate::captures::INSIDE;
use crate::report::{DesignReport, ItemReport, Outcome};
use crate::timeline_notes::partly;

/// Another document, imported on its own, to fill the components standing
/// for its components.
pub struct Referenced<'a, K: Kernel> {
    pub document: &'a Document<K>,
    /// Its occurrences by the GUIDs the file gives them (its import's
    /// [`crate::components::ComponentReport::occurrence_guids`], which this function also
    /// extends with those of the documents it filled in it).
    pub occurrence_guids: &'a HashMap<String, OccurrenceUid>,
    /// The name its component takes (made unique).
    pub name: String,
}

/// A component filled from another document.
#[derive(Debug, Clone, PartialEq)]
pub struct Linked {
    pub component: ComponentUid,
    /// The other document's URN.
    pub document: String,
    /// The component's name in this design.
    pub name: String,
    pub occurrences: usize,
}

/// What [`link_references`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LinkOutcome {
    pub linked: Vec<Linked>,
    /// Components that could not be filled: the document's URN and why.
    pub failed: Vec<(String, String)>,
    /// Captured positions inside the filled components that placed their
    /// occurrences.
    pub positions: usize,
    /// Those left out, by why.
    pub positions_left_out: BTreeMap<String, usize>,
}

/// Fills the components standing for components of other documents
/// (`report.components.inserted`) from `sources` (by the documents' URNs),
/// then places the occurrences inside them where the design's captured
/// positions put them ([`Linker`]). Components of documents not in
/// `sources` stay empty.
pub fn link_references<K: Kernel>(
    doc: &mut Document<K>,
    report: &mut DesignReport,
    sources: &HashMap<String, Referenced<'_, K>>,
) -> LinkOutcome {
    let mut linker = Linker::default();
    let mut documents: Vec<String> = Vec::new();
    for i in &report.components.inserted {
        if !documents.contains(&i.document) {
            documents.push(i.document.clone());
        }
    }
    for document in documents {
        if let Some(source) = sources.get(&document) {
            linker.fill(doc, report, &document, source);
        }
    }
    linker.finish(doc, report)
}

/// [`link_references`] one document at a time, so that the caller can drop
/// each document once it is copied.
#[derive(Debug, Default)]
pub struct Linker {
    /// The occurrences inside each filled component, by their GUIDs.
    inside: HashMap<ComponentUid, HashMap<String, OccurrenceUid>>,
    out: LinkOutcome,
}

impl Linker {
    /// Fills the components standing for components of the document
    /// `document` (its URN) with copies of `source`.
    pub fn fill<K: Kernel>(
        &mut self,
        doc: &mut Document<K>,
        report: &mut DesignReport,
        document: &str,
        source: &Referenced<'_, K>,
    ) {
        let components = &mut report.components;
        for inserted in components.inserted.clone() {
            if inserted.document != document {
                continue;
            }
            match doc.fill_component_copy(source.document, inserted.component, Some(&source.name)) {
                Ok(copies) => {
                    let guids: HashMap<String, OccurrenceUid> = source
                        .occurrence_guids
                        .iter()
                        .filter_map(|(g, o)| Some((g.clone(), *copies.get(o)?)))
                        .collect();
                    for (g, o) in &guids {
                        components.occurrence_guids.entry(g.clone()).or_insert(*o);
                    }
                    self.inside.insert(inserted.component, guids);
                    components.linked += inserted.occurrences;
                    self.out.linked.push(Linked {
                        component: inserted.component,
                        document: inserted.document.clone(),
                        name: doc.assembly().name(inserted.component),
                        occurrences: inserted.occurrences,
                    });
                }
                Err(e) => self
                    .out
                    .failed
                    .push((inserted.document.clone(), e.to_string())),
            }
        }
    }

    /// Places the occurrences inside the filled components where the
    /// design's captured positions put them: a `capture_position` feature
    /// in each component per captured position, at the end of the
    /// timeline; the captured position's report entry and timeline note no
    /// longer say they were left out.
    pub fn finish<K: Kernel>(
        mut self,
        doc: &mut Document<K>,
        report: &mut DesignReport,
    ) -> LinkOutcome {
        place_inside(doc, report, &self.inside, &mut self.out);
        report.components.placed_inside += self.out.positions;
        self.out
    }
}

/// What became of the positions inside components of other documents of
/// one captured position.
#[derive(Default)]
struct ItemPositions {
    /// Their occurrences, by the component they are placed in, with their
    /// placements there.
    placed: BTreeMap<ComponentUid, Vec<(OccurrenceUid, Transform)>>,
    /// Why others are left out.
    left_out: Vec<String>,
}

/// The captured positions inside filled components (see the module
/// documentation).
fn place_inside<K: Kernel>(
    doc: &mut Document<K>,
    report: &mut DesignReport,
    inside: &HashMap<ComponentUid, HashMap<String, OccurrenceUid>>,
    out: &mut LinkOutcome,
) {
    if report.components.positions_inside.is_empty() {
        return;
    }
    let mut items: BTreeMap<i64, ItemPositions> = BTreeMap::new();
    // Each occurrence's placement in its parent, by the positions in
    // timeline order: the last one, unless two paths disagree.
    let mut placed: BTreeMap<OccurrenceUid, (i64, Option<Transform>)> = BTreeMap::new();
    let mut positions: Vec<_> = report.components.positions_inside.iter().collect();
    positions.sort_by_key(|p| p.index);
    for p in positions {
        let a = doc.assembly();
        let item = items.entry(p.index).or_default();
        let mut skip = |why: &str| item.left_out.push(why.to_owned());
        let Some(start) = p
            .outer
            .last()
            .and_then(|o| a.occurrence(*o))
            .map(|o| o.component)
        else {
            skip("a position of an occurrence not in the occurrence tree");
            continue;
        };
        let Some(guids) = inside.get(&start) else {
            skip("a position inside a component of a document not found");
            continue;
        };
        let mut path = p.outer.clone();
        let mut component = start;
        for g in &p.inner {
            match guids
                .get(&g.to_lowercase())
                .and_then(|o| a.occurrence(*o))
                .filter(|o| o.parent == component)
            {
                Some(o) => {
                    path.push(o.uid);
                    component = o.component;
                }
                None => {
                    path.clear();
                    break;
                }
            }
        }
        if path.len() <= p.outer.len() {
            skip("a position of an occurrence the other document does not have");
            continue;
        }
        let (&last, parents) = path.split_last().expect("a path");
        if a.occurrence(last).is_some_and(|o| o.grounded) {
            skip("a position of a grounded occurrence");
            continue;
        }
        // Where the occurrences above it are at the end of the timeline.
        let parent = parents.iter().fold(Transform::IDENTITY, |t, o| {
            t.after(&doc.placement(*o).unwrap_or(Transform::IDENTITY))
        });
        let local = inverse(&parent).after(&p.placement);
        // A shared component: another path that puts the occurrence
        // elsewhere at the same point of the timeline leaves it where it is.
        match placed.get_mut(&last) {
            Some((index, at)) if *index == p.index => {
                if at.is_some_and(|t| !same(&t, &local)) {
                    *at = None;
                }
            }
            Some(entry) => *entry = (p.index, Some(local)),
            None => {
                placed.insert(last, (p.index, Some(local)));
            }
        }
    }
    for (occurrence, (index, at)) in placed {
        let item = items.entry(index).or_default();
        let Some(at) = at else {
            item.left_out.push(
                "occurrences of the other document's component put it in different places"
                    .to_owned(),
            );
            continue;
        };
        let Some(parent) = doc.assembly().occurrence(occurrence).map(|o| o.parent) else {
            continue;
        };
        item.placed
            .entry(parent)
            .or_default()
            .push((occurrence, at));
    }
    for (index, positions) in items {
        let at = report.items.iter().position(|i| i.index == index);
        let name = at.map(|i| report.items[i].name.clone());
        let mut left_out = positions.left_out;
        let mut added = Vec::new();
        for (component, list) in positions.placed {
            let def = json!({"type": "capture_position", "positions": list
                .iter()
                .map(|(o, t)| json!({"occurrence": o.to_string(), "transform": matrix_rows(t)}))
                .collect::<Vec<_>>()});
            let result = serde_json::from_value::<FeatureDef<ValueInput>>(def)
                .map_err(|e| e.to_string())
                .and_then(|def| {
                    doc.add_feature_to(&def, name.as_deref(), Some(component))
                        .or_else(|_| doc.add_feature_to(&def, None, Some(component)))
                        .map_err(|e| e.to_string())
                });
            match result {
                Ok(feature) => {
                    out.positions += list.len();
                    added.push(feature.uid);
                }
                Err(e) => left_out.extend(list.iter().map(|_| e.clone())),
            }
        }
        for why in &left_out {
            *out.positions_left_out.entry(why.clone()).or_default() += 1;
        }
        if let Some(at) = at {
            renote(doc, &mut report.items[at], &added, &left_out);
        }
    }
}

/// A captured position's report entry once the positions inside
/// components of other documents came in (`added`, its new features) or
/// not (`left_out`): its note and outcome, and its features' timeline
/// notes.
fn renote<K: Kernel>(
    doc: &mut Document<K>,
    item: &mut ItemReport,
    added: &[FeatureUid],
    left_out: &[String],
) {
    // The note's reasons, without those of the positions inside.
    let note = item.note.clone().unwrap_or_default();
    let mut reasons: Vec<String> = Vec::new();
    for part in note.split("; ") {
        let part = part.strip_prefix("left out: ").unwrap_or(part);
        if !part.is_empty() && !part.ends_with(INSIDE) {
            reasons.push(part.to_owned());
        }
    }
    reasons.extend(left_out.iter().cloned());
    item.features
        .extend(added.iter().map(|uid| uid.to_string()));
    if item.features.is_empty() {
        item.note = (!reasons.is_empty()).then(|| reasons.join("; "));
        return;
    }
    item.outcome = if reasons.is_empty() {
        Outcome::Parametric
    } else {
        Outcome::Partial
    };
    item.note = (!reasons.is_empty()).then(|| format!("left out: {}", reasons.join("; ")));
    let timeline = match item.outcome {
        Outcome::Partial => partly(item.note.as_deref()),
        _ => String::new(),
    };
    let notes: Vec<(FeatureUid, String)> = item
        .features
        .iter()
        .filter_map(|f| f.parse().ok())
        .map(|uid| (uid, timeline.clone()))
        .collect();
    let _ = doc.set_import_notes(&notes);
}

/// Whether two placements are the same within rounding.
fn same(a: &Transform, b: &Transform) -> bool {
    (0..3).all(|r| {
        (0..3).all(|c| (a.linear[r][c] - b.linear[r][c]).abs() <= 1e-9)
            && (a.translation[r] - b.translation[r]).abs() <= 1e-6
    })
}
