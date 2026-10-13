// SPDX-License-Identifier: MIT
//! Moving an occurrence to another parent (mitcad#219): Move to... on a
//! component occurrence in the browser. The occurrence keeps its identity,
//! its component (with that component's features, bodies and the
//! occurrences placed in it), its number, light bulb and grounding; only
//! its parent and its own placement change, the placement so that it
//! stays where it is in the design at the timeline marker. Occurrence
//! paths that lead through it follow it in the same undo step: links of
//! sketch planes, projections and modelling features to other components,
//! cut scopes, joints and assembly constraints, section analyses and
//! Isolate.
//!
//! [`Document::plan_reparent`] is the one place that decides whether a
//! move is allowed: the destination list, the preview and the command use
//! it (design policies, mitcad#220, add their checks there). It refuses,
//! naming the blocker, what the shared-definition model cannot represent
//! or what would change meaning:
//!
//! - the root, an occurrence made by Pattern at Points, an occurrence a
//!   timeline feature places (Move, Capture Position, a joint or
//!   constraint that moves it, a rigid group);
//! - a source parent or a destination placed more than once: the move
//!   would change every placement of that shared component;
//! - linked (read-only) parents and destinations;
//! - the current parent, and the occurrence's own component or anything
//!   placed in it (a cycle);
//! - joints and constraints whose moved occurrence would change, or whose
//!   component the occurrence would leave;
//! - Pattern at Points features and instances that refer to it;
//! - references across the occurrence while a parent on the way from the
//!   old parent or to the new one is placed by a timeline feature: the
//!   relative placement could then differ at the reference's point of the
//!   timeline.
//!
//! The command computes the new state before taking it and refuses it
//! when a feature that built before would fail.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::components::check_transform;
use super::{DocState, Document, ModelError, invalid, same_placement};
use crate::assembly::{Assembly, inverse};
use crate::document::AnalysisDef;
use crate::features::extrude_scope::{CutScope, TargetScope};
use crate::features::{FeatureDef, OccurrencePath};
use crate::ids::{ComponentUid, FeatureUid, OccurrenceUid};
use crate::kernel::Kernel;
use crate::links::OccurrenceLink;
use crate::recompute::FeatureStatus;
use crate::transform::Transform;

/// A validated move of one occurrence to another parent.
#[derive(Debug, Clone)]
pub(crate) struct ReparentPlan {
    pub(crate) occurrence: OccurrenceUid,
    /// The component it places.
    pub(crate) component: ComponentUid,
    /// Its path from the root before the move.
    pub(crate) source: Vec<OccurrenceUid>,
    /// The component it is placed in before the move.
    pub(crate) parent: ComponentUid,
    /// The component it goes into, and the path to that component.
    pub(crate) destination: ComponentUid,
    pub(crate) destination_path: Vec<OccurrenceUid>,
    /// Its placement in the destination: the same place in the design.
    pub(crate) transform: Transform,
    /// What follows it: the features, analyses and Isolate whose
    /// occurrence paths lead through it.
    pub(crate) updates: Vec<String>,
    changes: Rewritten,
}

impl ReparentPlan {
    /// Its path from the root after the move.
    pub(crate) fn path(&self) -> Vec<OccurrenceUid> {
        let mut path = self.destination_path.clone();
        path.push(self.occurrence);
        path
    }
}

/// The new references, kept apart until the move is taken.
#[derive(Debug, Clone, Default)]
struct Rewritten {
    features: Vec<(usize, FeatureDef)>,
    analyses: Vec<(usize, OccurrencePath)>,
    isolated: Vec<(usize, String)>,
}

/// How occurrence paths through the moved occurrence follow it.
struct Rewrite<'a> {
    assembly: &'a Assembly,
    occurrence: OccurrenceUid,
    /// Its path from the root before the move.
    old: &'a [OccurrenceUid],
    /// And after.
    new: Vec<OccurrenceUid>,
    /// Why references across it cannot follow it, when they cannot.
    varying: Option<String>,
}

impl Rewrite<'_> {
    fn name(&self) -> String {
        self.assembly.occurrence_name(self.occurrence)
    }

    /// Whether a path from the root leads through the occurrence.
    fn through(&self, path: &[OccurrenceUid]) -> bool {
        path.len() >= self.old.len() && path[..self.old.len()] == *self.old
    }

    /// A path from the root through the occurrence goes its new way.
    fn absolute(&self, path: &mut Vec<OccurrenceUid>) -> bool {
        if !self.through(path) {
            return false;
        }
        let rest = path.split_off(self.old.len());
        path.clone_from(&self.new);
        path.extend(rest);
        true
    }

    /// Both paths of a link to another component's geometry.
    fn link(&self, link: &mut OccurrenceLink) -> Result<bool, String> {
        let source = self.through(&link.source.0);
        let target = self.through(&link.target.0);
        if source != target
            && let Some(why) = &self.varying
        {
            return Err(why.clone());
        }
        let moved = self.absolute(&mut link.source.0);
        Ok(self.absolute(&mut link.target.0) || moved)
    }

    /// A joint's or constraint's path, relative to its feature's component:
    /// its first occurrence is the one the joint moves, which must stay.
    fn unit(&self, path: &mut Vec<OccurrenceUid>) -> Result<bool, String> {
        let Some(k) = path.iter().position(|o| *o == self.occurrence) else {
            return Ok(false);
        };
        let n = self.old.len();
        if k >= n || path[..k] != self.old[n - 1 - k..n - 1] {
            // Not a path to it: recompute reports such a path.
            return Ok(false);
        }
        if k == 0 {
            return Err(format!(
                "it moves {} itself, which would leave its component",
                self.name()
            ));
        }
        // The feature's component is placed at old[..j]; old[j] is what it
        // moves.
        let j = n - 1 - k;
        let parents = &self.new[..self.new.len() - 1];
        if parents.len() <= j || parents[..=j] != self.old[..=j] {
            return Err(format!(
                "{} would no longer be in {}, the occurrence it moves",
                self.name(),
                self.assembly.occurrence_name(self.old[j])
            ));
        }
        if let Some(why) = &self.varying {
            return Err(why.clone());
        }
        let mut moved = parents[j..].to_vec();
        moved.push(self.occurrence);
        moved.extend_from_slice(&path[k + 1..]);
        *path = moved;
        Ok(true)
    }

    /// A cut scope's paths (from the root).
    fn scope(&self, scope: &mut CutScope) -> Result<bool, String> {
        if !scope.paths().iter().any(|p| self.through(&p.0)) {
            return Ok(false);
        }
        if let Some(why) = &self.varying {
            return Err(why.clone());
        }
        let mut paths: Vec<&mut Vec<OccurrenceUid>> = vec![&mut scope.context.occurrence.0];
        match &mut scope.targets {
            TargetScope::Automatic {
                excluded,
                excluded_bodies,
            } => {
                paths.extend(excluded.iter_mut().map(|t| &mut t.occurrence.0));
                paths.extend(excluded_bodies.iter_mut().map(|t| &mut t.occurrence.0));
            }
            TargetScope::Explicit {
                included,
                excluded_bodies,
            } => {
                paths.extend(included.iter_mut().map(|t| &mut t.occurrence.0));
                paths.extend(excluded_bodies.iter_mut().map(|t| &mut t.occurrence.0));
            }
        }
        for path in paths {
            self.absolute(path);
        }
        Ok(true)
    }

    /// A feature's references; Ok(true) when they changed, Err with the
    /// blocker when they cannot follow.
    fn feature(&self, def: &mut FeatureDef) -> Result<bool, String> {
        let o = self.occurrence;
        let other = crate::joints::referenced_occurrences(def).contains(&o);
        let mut changed = false;
        match def {
            FeatureDef::Sketch(s) => {
                for link in s
                    .plane_link
                    .iter_mut()
                    .chain(s.projections.iter_mut().filter_map(|p| p.link.as_mut()))
                {
                    changed |= self.link(link)?;
                }
            }
            FeatureDef::Combine(c) => {
                for link in c.tool_links.values_mut() {
                    changed |= self.link(link)?;
                }
            }
            FeatureDef::Sweep(s) => {
                for link in s.body_links.values_mut() {
                    changed |= self.link(link)?;
                }
            }
            FeatureDef::SplitBody(s) => {
                for link in s.tool_links.values_mut() {
                    changed |= self.link(link)?;
                }
            }
            FeatureDef::ConstructionPlane(p) => {
                for link in p.links.values_mut() {
                    changed |= self.link(link)?;
                }
            }
            FeatureDef::Extrude(x) => {
                for link in x.object_links.values_mut() {
                    changed |= self.link(link)?;
                }
                if let Some(scope) = &mut x.cut_scope {
                    changed |= self.scope(scope)?;
                }
            }
            FeatureDef::Joint(j) => {
                changed |= self.unit(&mut j.a.occurrence.0)?;
                changed |= self.unit(&mut j.b.occurrence.0)?;
            }
            FeatureDef::AsBuiltJoint(j) => {
                changed |= self.unit(&mut j.a.0)?;
                changed |= self.unit(&mut j.b.0)?;
                if let Some(origin) = &mut j.origin {
                    changed |= self.unit(&mut origin.occurrence.0)?;
                }
            }
            FeatureDef::ConstraintSet(set) => {
                for row in &mut set.rows {
                    for origin in [&mut row.a, &mut row.b]
                        .into_iter()
                        .chain(row.a2.as_mut())
                        .chain(row.b2.as_mut())
                    {
                        changed |= self.unit(&mut origin.occurrence.0)?;
                    }
                }
            }
            FeatureDef::RigidGroup(g) if g.occurrences.contains(&o) => {
                return Err(format!(
                    "it holds {} rigidly to other occurrences of its component",
                    self.name()
                ));
            }
            FeatureDef::MoveOccurrence(m) if m.occurrences.contains(&o) => {
                return Err(format!("it moves {}", self.name()));
            }
            FeatureDef::CapturePosition(c) if c.positions.iter().any(|p| p.occurrence == o) => {
                return Err(format!("it captures the position of {}", self.name()));
            }
            FeatureDef::PatternAtPoints(p) if p.paths().iter().any(|path| path.0.contains(&o)) => {
                return Err(format!(
                    "Pattern at Points refers to {}; moving its occurrences is not supported",
                    self.name()
                ));
            }
            _ if other => {
                return Err(format!(
                    "it refers to {} in a way the move cannot update",
                    self.name()
                ));
            }
            _ => {}
        }
        Ok(changed)
    }
}

/// The paths of the document's state that lead through the occurrence, as
/// they become, with what follows and what blocks the move.
fn rewrite_references(
    state: &DocState,
    rewrite: &Rewrite<'_>,
) -> (Rewritten, Vec<String>, Vec<String>) {
    let o = rewrite.occurrence;
    let mut out = Rewritten::default();
    let mut updates = Vec::new();
    let mut blockers = Vec::new();
    for (i, entry) in state.features.iter().enumerate() {
        if !crate::joints::referenced_occurrences(&entry.def).contains(&o) {
            continue;
        }
        let mut def = entry.def.clone();
        match rewrite.feature(&mut def) {
            Ok(true) => {
                updates.push(entry.name.clone());
                out.features.push((i, def));
            }
            Ok(false) => {}
            Err(why) => blockers.push(format!("{}: {why}", entry.name)),
        }
    }
    for (i, analysis) in state.analyses.iter().enumerate() {
        match &analysis.def {
            AnalysisDef::Section(section) => {
                if let Some(path) = &section.occurrence {
                    let mut moved = path.0.clone();
                    if rewrite.absolute(&mut moved) {
                        updates.push(analysis.name.clone());
                        out.analyses.push((i, OccurrencePath(moved)));
                    }
                }
            }
        }
    }
    for (i, item) in state.display.isolated.iter().enumerate() {
        let uids: Result<Vec<OccurrenceUid>, _> = item
            .occurrence
            .split('/')
            .filter(|part| !part.is_empty())
            .map(str::parse::<OccurrenceUid>)
            .collect();
        if let Ok(mut path) = uids
            && rewrite.absolute(&mut path)
        {
            out.isolated.push((i, OccurrencePath(path).to_string()));
        }
    }
    if !out.isolated.is_empty() {
        updates.push("Isolate".to_owned());
    }
    // Occurrences made by Pattern at Points keep the paths they were made
    // from.
    for made in &state.assembly.occurrences {
        if let Some(pattern) = &made.pattern
            && (pattern.source.0.contains(&o)
                || pattern
                    .target
                    .occurrence
                    .as_ref()
                    .is_some_and(|p| p.0.contains(&o)))
        {
            blockers.push(format!(
                "{} is made by Pattern at Points from {}; moving it is not supported",
                state.assembly.occurrence_name(made.uid),
                rewrite.name()
            ));
        }
    }
    (out, updates, blockers)
}

/// The first timeline feature that places an occurrence: moves or sets it,
/// moves it as the occurrence a joint, constraint or rigid group joins, or
/// made it (Pattern at Points). The whole timeline counts, also after the
/// marker and suppressed features.
fn placing_feature(state: &DocState, uid: OccurrenceUid) -> Option<String> {
    let first = |path: &OccurrencePath| path.0.first() == Some(&uid);
    let made_by: Option<FeatureUid> = state
        .assembly
        .occurrence(uid)
        .and_then(|o| o.pattern.as_ref())
        .map(|p| p.feature);
    state
        .features
        .iter()
        .find(|f| match &f.def {
            FeatureDef::MoveOccurrence(m) => m.occurrences.contains(&uid),
            FeatureDef::CapturePosition(c) => c.positions.iter().any(|p| p.occurrence == uid),
            FeatureDef::Joint(j) => first(&j.a.occurrence) || first(&j.b.occurrence),
            FeatureDef::AsBuiltJoint(j) => first(&j.a) || first(&j.b),
            FeatureDef::RigidGroup(g) => g.occurrences.contains(&uid),
            FeatureDef::ConstraintSet(set) => set
                .rows
                .iter()
                .any(|row| row.origins().any(|(_, origin)| first(&origin.occurrence))),
            _ => made_by == Some(f.uid),
        })
        .map(|f| f.name.clone())
}

/// Takes a planned move into a state.
fn take(state: &mut DocState, plan: &ReparentPlan) {
    if let Some(o) = state.assembly.occurrence_mut(plan.occurrence) {
        o.parent = plan.destination;
        o.transform = plan.transform;
    }
    for (i, def) in &plan.changes.features {
        Arc::make_mut(&mut state.features[*i]).def = def.clone();
    }
    for (i, path) in &plan.changes.analyses {
        match &mut state.analyses[*i].def {
            AnalysisDef::Section(section) => section.occurrence = Some(path.clone()),
        }
    }
    for (i, text) in &plan.changes.isolated {
        state.display.isolated[*i].occurrence.clone_from(text);
    }
}

impl<K: Kernel> Document<K> {
    /// What places an occurrence other than its own transform: a timeline
    /// feature, or the joints where the solved placement differs.
    fn placed_by(&self, uid: OccurrenceUid) -> Option<String> {
        if let Some(name) = placing_feature(&self.state, uid) {
            return Some(name);
        }
        let own = self.state.assembly.occurrence(uid)?;
        let solved = self.result.placements.get(&uid)?;
        (!same_placement(solved, &own.transform)).then(|| "the joints".to_owned())
    }

    /// The checks of the occurrence to move, whatever the destination:
    /// its component and its parent.
    pub(crate) fn reparent_source(
        &self,
        source: &[OccurrenceUid],
    ) -> Result<(OccurrenceUid, ComponentUid, ComponentUid), ModelError> {
        let a = &self.state.assembly;
        let Some((&uid, parents)) = source.split_last() else {
            return Err(invalid("the root component cannot be moved"));
        };
        let component =
            crate::joints::path_component(a, ComponentUid::ROOT, source).map_err(invalid)?;
        let occurrence = self.state.require_occurrence(uid)?;
        let name = a.occurrence_name(uid);
        let parent = occurrence.parent;
        if occurrence.pattern.is_some() {
            return Err(invalid(format!(
                "{name} is made by Pattern at Points; edit that feature instead"
            )));
        }
        // Each placement of a shared parent has the occurrence: moving it
        // out of one would move it out of all.
        let placements = a.paths_to(parent);
        if placements.len() != 1 {
            return Err(invalid(format!(
                "{} is placed {} times and {name} is in each of them; moving it would change \
                 every placement of {}",
                a.name(parent),
                placements.len(),
                a.name(parent)
            )));
        }
        if placements[0] != parents {
            return Err(invalid(format!(
                "{} is not the placement of {name}",
                OccurrencePath(source.to_vec())
            )));
        }
        self.state.editable(parent)?;
        if let Some(by) = self.placed_by(uid) {
            return Err(invalid(format!(
                "{name} is placed by {by}; edit or delete it before moving {name}"
            )));
        }
        Ok((uid, component, parent))
    }

    /// Whether the occurrence at `source` (its path from the root) can go
    /// into the component placed at `destination` (empty: the root), and
    /// what the move does: the one validation of hierarchy moves (design
    /// policies, mitcad#220, belong here too). Err lists the blockers.
    pub(crate) fn plan_reparent(
        &self,
        source: &[OccurrenceUid],
        destination: &[OccurrenceUid],
    ) -> Result<ReparentPlan, Vec<String>> {
        let one = |e: ModelError| vec![e.to_string()];
        let (uid, component, parent) = self.reparent_source(source).map_err(one)?;
        let a = &self.state.assembly;
        let name = a.occurrence_name(uid);
        let target = crate::joints::path_component(a, ComponentUid::ROOT, destination)
            .map_err(|e| vec![e])?;
        if target == parent {
            return Err(vec![format!("{name} is already in {}", a.name(target))]);
        }
        if a.contains(component, target) {
            return Err(vec![format!(
                "{name} cannot go into {}, which is {}",
                a.name(target),
                if target == component {
                    "its own component"
                } else {
                    "placed in it"
                }
            )]);
        }
        let placements = a.paths_to(target);
        if placements.len() != 1 {
            return Err(vec![format!(
                "{} is placed {} times; {name} would appear in each of them. Choose a \
                 component placed once",
                a.name(target),
                placements.len()
            )]);
        }
        self.state.editable(target).map_err(one)?;
        // The placement that keeps it where it is: through the parents
        // the old and the new way do not share.
        let parents = &source[..source.len() - 1];
        let common = parents
            .iter()
            .zip(destination)
            .take_while(|(x, y)| x == y)
            .count();
        let own = a
            .occurrence(uid)
            .map_or(Transform::IDENTITY, |o| o.transform);
        let transform = inverse(&self.path_transform(&destination[common..]))
            .after(&self.path_transform(&parents[common..]))
            .after(&own);
        check_transform(&transform).map_err(one)?;
        let varying = parents[common..]
            .iter()
            .chain(&destination[common..])
            .find_map(|x| self.placed_by(*x).map(|by| (*x, by)))
            .map(|(x, by)| {
                format!(
                    "{} is placed by {by} in the timeline, so references across {name} could \
                     change where they are evaluated",
                    a.occurrence_name(x)
                )
            });
        let mut new = destination.to_vec();
        new.push(uid);
        let rewrite = Rewrite {
            assembly: a,
            occurrence: uid,
            old: source,
            new,
            varying,
        };
        let (changes, updates, blockers) = rewrite_references(&self.state, &rewrite);
        if !blockers.is_empty() {
            return Err(blockers);
        }
        let plan = ReparentPlan {
            occurrence: uid,
            component,
            source: source.to_vec(),
            parent,
            destination: target,
            destination_path: destination.to_vec(),
            transform,
            updates,
            changes,
        };
        // The design policy (mitcad#220): the destination must take
        // components, and no link may come to reach an owner that refuses
        // it. Shown with the plan, so the picker explains it before the
        // command (which checks it again) runs.
        let mut next = self.state.clone();
        take(&mut next, &plan);
        if let Some(refused) = self.policy_refusal(&next) {
            return Err(vec![refused]);
        }
        Ok(plan)
    }

    /// Moves the occurrence at `source` into the component placed at
    /// `destination` (empty: the root), where it stays in the design, with
    /// the references through it; one undo step. Refused, changing
    /// nothing, when [`Document::plan_reparent`] refuses it or a feature
    /// that built would fail.
    pub(crate) fn reparent_occurrence(
        &mut self,
        source: &[OccurrenceUid],
        destination: &[OccurrenceUid],
    ) -> Result<ReparentPlan, ModelError> {
        let stale = self.result.results.len() != self.state.features.len()
            || self
                .result
                .results
                .iter()
                .zip(&self.state.features)
                .any(|(r, f)| r.uid != f.uid);
        if stale {
            self.try_recompute()?;
        }
        let plan = self
            .plan_reparent(source, destination)
            .map_err(|blockers| invalid(blockers.join("; ")))?;
        let built: BTreeSet<FeatureUid> = self
            .result
            .results
            .iter()
            .filter(|r| r.status.is_ok())
            .map(|r| r.uid)
            .collect();
        self.apply_checked(
            |state| {
                let label = format!(
                    "Move {} to {}",
                    state.assembly.occurrence_name(plan.occurrence),
                    state.assembly.name(plan.destination)
                );
                take(state, &plan);
                Ok((label, ()))
            },
            |state, result| {
                let failed: Vec<String> = result
                    .results
                    .iter()
                    .filter(|r| built.contains(&r.uid))
                    .filter_map(|r| match &r.status {
                        FeatureStatus::Failed(message) => Some(format!(
                            "{}: {message}",
                            state
                                .features
                                .iter()
                                .find(|f| f.uid == r.uid)
                                .map_or_else(|| r.uid.to_string(), |f| f.name.clone())
                        )),
                        _ => None,
                    })
                    .collect();
                if failed.is_empty() {
                    Ok(())
                } else {
                    Err(invalid(format!(
                        "the move would make features fail: {}",
                        failed.join("; ")
                    )))
                }
            },
        )?;
        Ok(plan)
    }
}
