// SPDX-License-Identifier: MIT
//! Bodies by the item that made them (mitcad#96). The stream decoder gives
//! a body input the timeline item that made the body and its index among
//! that item's bodies (`_f3d.producer`, `_f3d.body_index`, from the body's
//! record: `mitcad_f3d::design::build`). The import keeps its own record of
//! what each item made: the bodies of the features it made for the item,
//! or of the base feature that took the item's state. A body input is
//! found there first, then by its names (`refs::resolve_body`).
//!
//! The file also says which tools a combine consumed. The ASM history keeps
//! such tools as they were (the design does not have them any more), so
//! the combine's state would hold them: they are retired from the history
//! from that state on ([`crate::history::Oracle::retire`]), and the combine
//! is checked without them.

use mitcad_f3d::design::ir::{Fingerprint, Reference, TimelineItem};
use mitcad_model::features::FeatureDef;
use mitcad_model::{BodyUid, ComponentUid, FeatureUid, Kernel};
use serde_json::Value;

use crate::Importer;
use crate::history::{RELATIVE, Sig};
use crate::refs;

impl<K: Kernel> Importer<'_, K> {
    /// The bodies the item `index` made, in the order Mitcad made them,
    /// and whether its own features made them: the bodies of its features
    /// (a split's pieces from the bodies it split, which keep their ids; a
    /// combine's from its target), else those the base features that took
    /// its state brought into its component (new ones, else those they
    /// replaced: such a base feature brings every body that differs from
    /// the state, and a new body can take the id of a body it replaces).
    /// Bodies later items removed are listed too (the positions stay).
    pub(crate) fn made_by(&self, index: i64) -> (Vec<BodyUid>, bool) {
        let Some(report) = self.report.items.iter().find(|r| r.index == index) else {
            return (Vec::new(), false);
        };
        // (The base feature that brought in the operations of the item
        // before its own comes first: a split's first piece, mitcad#181.)
        let features: Vec<FeatureUid> = self
            .start_bases
            .get(&index)
            .copied()
            .into_iter()
            .chain(report.features.iter().filter_map(|f| f.parse().ok()))
            .collect();
        let mut current: Vec<BodyUid> = self.doc.bodies().iter().map(|b| b.uid).collect();
        current.sort();
        let mut out = Vec::new();
        let mut own_features = true;
        for uid in features {
            let Some(f) = self.doc.feature(uid) else {
                continue;
            };
            let own = current.iter().copied().filter(|b| b.feature == uid);
            match &f.def {
                FeatureDef::Base(_) => {
                    own_features = false;
                    out.extend(self.base_bodies(uid, self.components.of_item(index)));
                }
                FeatureDef::SplitBody(def) => {
                    out.extend(def.bodies.iter().copied());
                    out.extend(own);
                }
                FeatureDef::Combine(def) => {
                    let own: Vec<BodyUid> = own.collect();
                    if !own.is_empty() {
                        out.push(def.target);
                        out.extend(own);
                    }
                }
                _ => out.extend(own),
            }
        }
        let mut seen = Vec::new();
        out.retain(|b| {
            let new = !seen.contains(b);
            seen.push(*b);
            new
        });
        (out, own_features)
    }

    /// The bodies a fallback's base features (`first` and those made with
    /// it for other components: the same source) brought into `component`:
    /// the new ones, else those they replaced.
    fn base_bodies(&self, first: FeatureUid, component: ComponentUid) -> Vec<BodyUid> {
        let Some(FeatureDef::Base(def)) = self.doc.feature(first).map(|f| &f.def) else {
            return Vec::new();
        };
        let source = def.source.clone();
        let mut new = Vec::new();
        let mut replaced = Vec::new();
        for f in self.doc.features().skip_while(|f| f.uid != first) {
            let FeatureDef::Base(def) = &f.def else {
                break;
            };
            if f.uid != first && def.source != source {
                break;
            }
            for i in 0..def.bodies.len() {
                let (uid, list) = match def.replaces.get(i) {
                    Some(r) => (*r, &mut replaced),
                    None => (BodyUid::new(f.uid, i as u32), &mut new),
                };
                if self.doc.body_component(uid) == Some(component) {
                    list.push(uid);
                }
            }
        }
        if new.is_empty() { replaced } else { new }
    }

    /// A body input by the item that made it: its only body, else the body
    /// at the decoded index (or the one of its bodies the names give).
    /// Where a base feature took the item's state, the body found by its
    /// names comes first. `None` when the decoder gives no producer, or the
    /// replay does not have that body.
    pub(crate) fn produced_body(&self, fp: &Fingerprint) -> Option<BodyUid> {
        let f3d = fp.f3d.as_ref()?;
        let producer = f3d.producer?;
        let (made, own) = self.made_by(producer);
        if crate::tracing() {
            let kernel = self.doc.kernel();
            let made: Vec<String> = made
                .iter()
                .map(|b| {
                    let volume = self
                        .doc
                        .body_shape(*b)
                        .and_then(|s| Sig::of(kernel, s))
                        .map_or(-1.0, |s| s.volume);
                    format!("{b} ({volume:.1} mm3)")
                })
                .collect();
            eprintln!(
                "import: a body item {producer} made (index {:?}, its own features {own}): {made:?}",
                f3d.body_index
            );
        }
        let live = |b: &BodyUid| self.doc.body_shape(*b).is_some();
        if own && let [only] = made[..] {
            return live(&only).then_some(only);
        }
        if (made.len() > 1 || !own)
            && let Some(named) = refs::resolve_body(self.doc, fp)
            && (made.contains(&named) || !own)
        {
            return Some(named);
        }
        let index = usize::try_from(f3d.body_index.unwrap_or(0)).ok()?;
        // The bodies a split divides keep their ids and are made by the
        // items that made them: the split's own records are its new pieces
        // only, which its index counts.
        let kept = self.split_kept(producer);
        if !kept.is_empty() {
            let pieces: Vec<BodyUid> = made.iter().copied().filter(|b| !kept.contains(b)).collect();
            if !pieces.is_empty() {
                return pieces.get(index).copied().filter(live);
            }
        }
        made.get(index).copied().filter(live)
    }

    /// The bodies the splits of the item `index` divide (they stay among
    /// the bodies [`Importer::made_by`] lists).
    fn split_kept(&self, index: i64) -> Vec<BodyUid> {
        let Some(report) = self.report.items.iter().find(|r| r.index == index) else {
            return Vec::new();
        };
        report
            .features
            .iter()
            .filter_map(|f| f.parse::<FeatureUid>().ok())
            .filter_map(|uid| match &self.doc.feature(uid)?.def {
                FeatureDef::SplitBody(def) => Some(def.bodies.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// The replay's bodies a dressup edge may be on: the live bodies the
    /// item that made the edge's body made (`_f3d.producer`). Empty when
    /// the decoder gives no producer or the replay holds none of its
    /// bodies.
    fn edge_bodies(&self, fp: &Fingerprint) -> Vec<BodyUid> {
        let Some(producer) = fp.f3d.as_ref().and_then(|f| f.producer) else {
            return Vec::new();
        };
        let (made, _) = self.made_by(producer);
        made.into_iter()
            .filter(|b| self.doc.body_shape(*b).is_some())
            .collect()
    }

    /// A fillet's or chamfer's edge given by its fingerprint among the
    /// replay's edges ([`refs::dressup_edge_match`]), on the bodies of the
    /// item that made the edge's body first: bodies that touch have edges
    /// in the same place, and the file's edge is on one of them. Where
    /// those bodies do not hold it, among the edges of all bodies.
    pub(crate) fn match_dressup_edge(
        &self,
        replay: &refs::ReplayEdges,
        fp: &Fingerprint,
    ) -> Result<Vec<refs::FoundEdge>, refs::DressupEdgeFailure> {
        let bodies = self.edge_bodies(fp);
        if !bodies.is_empty()
            && let Ok(found) = refs::dressup_edge_match(&replay.only(&bodies), fp)
        {
            return Ok(found);
        }
        refs::dressup_edge_match(replay, fp)
    }

    /// A body input among the replay's bodies: by the item that made it
    /// first, else by its names or measures.
    pub(crate) fn body_ref(&self, fp: &Fingerprint) -> Option<BodyUid> {
        self.produced_body(fp)
            .or_else(|| refs::resolve_body(self.doc, fp))
    }

    /// The bodies of decoded references, every one found (once each), or
    /// `None`.
    pub(crate) fn bodies_of_refs(&self, refs: &[Reference]) -> Option<Vec<BodyUid>> {
        let mut out = Vec::new();
        for r in refs {
            let Reference::Body(fp) = r else {
                return None;
            };
            let b = self.body_ref(fp)?;
            if !out.contains(&b) {
                out.push(b);
            }
        }
        Some(out)
    }

    /// The bodies a join consumed that the history's state still holds: the
    /// ASM history keeps every body a join merged into another as it was
    /// (the same stored body as in the state before, unchanged), though the
    /// design no longer has it, as for a combine's tools. Of the state
    /// `states.after`, the signatures without the bodies that are the
    /// state before's own, unchanged, and that no body of the candidate's
    /// result `current` is (the candidate merged them), with their stored
    /// ids; None when there are none. The caller checks that the rest is
    /// the candidate's result before it retires them
    /// ([`crate::history::Oracle::retire`]).
    pub(crate) fn merged_by_join(
        &mut self,
        states: crate::geometric::States,
        current: &[Sig],
    ) -> Option<(Vec<Sig>, Vec<u64>)> {
        if !self.oracle.enabled {
            return None;
        }
        let before = states.before?;
        let kernel = self.doc.kernel();
        let earlier: Vec<(u64, Sig)> = self
            .oracle
            .state(kernel, before)
            .ok()?
            .iter()
            .filter_map(|(b, s)| Some((b.id?, *s)))
            .collect();
        let later: Vec<(Option<u64>, Sig)> = self
            .oracle
            .state(kernel, states.after)
            .ok()?
            .iter()
            .map(|(b, s)| (b.id, *s))
            .collect();
        crate::history::without_merged(&earlier, &later, current)
    }

    /// A combine whose tools the file says it consumed
    /// (`isKeepToolBodies` false): where every tool is found by the item
    /// that made it and the item's history state holds each tool unchanged
    /// (the same stored body as in the state before it, with the replay's
    /// measures of the tool), those stored bodies are retired from that
    /// state on, so that the combine, and every item after it, is checked
    /// without them.
    pub(crate) fn retire_consumed_tools(&mut self, index: i64, item: &TimelineItem) {
        if !self.oracle.enabled || item.object_type() != Some("CombineFeature") {
            return;
        }
        let Some(detail) = item
            .detail
            .as_ref()
            .and_then(|d| serde_json::to_value(d).ok())
        else {
            return;
        };
        if detail.get("isKeepToolBodies").and_then(Value::as_bool) != Some(false) {
            return;
        }
        let tools: Vec<Reference> = detail
            .get("toolBodies")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| Some(Reference::from_map(v.as_object()?.clone())))
                    .collect()
            })
            .unwrap_or_default();
        // Only tools found by the items that made them: a body found by its
        // names or measures alone may be another one in the same place,
        // and a body left out by mistake would keep every later item from
        // its state.
        let Some(tools) = tools
            .iter()
            .map(|r| match r {
                Reference::Body(fp) => self.produced_body(fp),
                _ => None,
            })
            .collect::<Option<Vec<BodyUid>>>()
            .filter(|t| !t.is_empty())
        else {
            return;
        };
        let Some(state) = self.oracle.item_state(index).filter(|&s| s > 0) else {
            return;
        };
        let kernel = self.doc.kernel();
        let Some(sigs) = tools
            .iter()
            .map(|t| Sig::of(kernel, self.doc.body_shape(*t)?))
            .collect::<Option<Vec<Sig>>>()
        else {
            return;
        };
        let Ok(before) = self.oracle.state(kernel, state - 1) else {
            return;
        };
        let before: Vec<u64> = before.iter().filter_map(|(b, _)| b.id).collect();
        let Ok(after) = self.oracle.state(kernel, state) else {
            return;
        };
        let mut ids = Vec::new();
        for sig in &sigs {
            let fits: Vec<u64> = after
                .iter()
                .filter(|(b, s)| {
                    s.distance(sig) <= RELATIVE && b.id.is_some_and(|id| before.contains(&id))
                })
                .filter_map(|(b, _)| b.id)
                .filter(|id| !ids.contains(id))
                .collect();
            match fits[..] {
                [id] => ids.push(id),
                _ => return,
            }
        }
        if crate::tracing() {
            eprintln!(
                "import: {}: the tools it consumed stay in the history; left out from state {state}",
                crate::item_name(item)
            );
        }
        self.oracle.retire(state, &ids);
    }
}
