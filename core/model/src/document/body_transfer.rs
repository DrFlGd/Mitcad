// SPDX-License-Identifier: MIT
//! Moving bodies into an existing component on the timeline (mitcad#465;
//! the feature in `features/body_transfer.rs`): the command that adds a
//! Body Transfer at the marker, with the occurrence paths the bodies and the
//! destination are seen at, and the destinations it can move them into.

use std::collections::BTreeSet;

use super::{Added, DocState, Document, ModelError, invalid};
use crate::features::OccurrencePath;
use crate::features::{BodyTransferDef, FeatureDef, ValueInput};
use crate::ids::{BodyUid, ComponentUid, FeatureUid, OccurrenceUid};
use crate::joints::path_component;
use crate::kernel::Kernel;
use crate::links::{OccurrenceLink, first_path};

/// A component bodies can be moved into, seen at one occurrence path.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferDestination {
    pub component: ComponentUid,
    /// The occurrence path from the root (empty for the root).
    pub occurrence: Vec<OccurrenceUid>,
    /// Why the bodies cannot go there, if they cannot.
    pub reason: Option<String>,
}

impl DocState {
    /// Whether features of `component` take in bodies of other components
    /// by body transfers: copies of it then take its bodies as they are.
    pub(crate) fn transfers_into(&self, component: ComponentUid) -> bool {
        self.features
            .iter()
            .any(|f| f.component == component && matches!(f.def, FeatureDef::BodyTransfer(_)))
    }
}

impl<K: Kernel> Document<K> {
    /// The component the bodies are in at the marker: all of one.
    fn transfer_source(&self, bodies: &[BodyUid]) -> Result<ComponentUid, ModelError> {
        if bodies.is_empty() {
            return Err(invalid("no bodies selected"));
        }
        let mut source = None;
        for (i, body) in bodies.iter().enumerate() {
            if bodies[..i].contains(body) {
                return Err(invalid(format!("body {body} is listed more than once")));
            }
            let owner = self.body_component(*body).ok_or_else(|| {
                invalid(format!("body {body} does not exist at the timeline marker"))
            })?;
            match source {
                None => source = Some(owner),
                Some(s) if s != owner => {
                    return Err(invalid(format!(
                        "{} is in {} and {} in {}: the bodies of one transfer come from one \
                         component",
                        self.body_name(bodies[0]),
                        self.state.assembly.name(s),
                        self.body_name(*body),
                        self.state.assembly.name(owner)
                    )));
                }
                Some(_) => {}
            }
        }
        Ok(source.expect("one body at least"))
    }

    /// The path `given` when it leads to `component`, else its first path.
    fn transfer_path(
        &self,
        component: ComponentUid,
        given: Option<&[OccurrenceUid]>,
        what: &str,
    ) -> Result<Vec<OccurrenceUid>, ModelError> {
        let a = &self.state.assembly;
        match given {
            Some(path) => {
                let at = path_component(a, ComponentUid::ROOT, path)
                    .map_err(|e| invalid(format!("{what}: {e}")))?;
                if at != component {
                    return Err(invalid(format!(
                        "{what} {} leads to {}, not to {}",
                        OccurrencePath(path.to_vec()),
                        a.name(at),
                        a.name(component)
                    )));
                }
                Ok(path.to_vec())
            }
            None => first_path(a, component, None).map_err(invalid),
        }
    }

    /// Why the bodies of `source` cannot go into `destination`, if they
    /// cannot (what the definition alone shows; the command also computes).
    fn transfer_refusal(&self, source: ComponentUid, destination: ComponentUid) -> Option<String> {
        let a = &self.state.assembly;
        if destination == source {
            return Some(format!("the bodies are in {} already", a.name(destination)));
        }
        if let Err(e) = self.state.editable(destination) {
            return Some(e.to_string());
        }
        if a.component(destination)
            .is_some_and(|c| c.library.is_some())
        {
            return Some(format!(
                "{} is a library part, made by its own feature",
                a.name(destination)
            ));
        }
        None
    }

    /// The placed components (each occurrence path, the root first) and
    /// whether the bodies can be moved into each.
    pub fn transfer_destinations(
        &self,
        bodies: &[BodyUid],
    ) -> Result<Vec<TransferDestination>, ModelError> {
        let source = self.transfer_source(bodies)?;
        let a = &self.state.assembly;
        let mut out = Vec::new();
        for component in
            std::iter::once(ComponentUid::ROOT).chain(a.components.iter().map(|c| c.uid))
        {
            let reason = self.transfer_refusal(source, component);
            for path in a.paths_to(component) {
                out.push(TransferDestination {
                    component,
                    occurrence: path,
                    reason: reason.clone(),
                });
            }
        }
        Ok(out)
    }

    /// Moves bodies into an existing component by a Body Transfer at the
    /// timeline marker, in the destination (one undo step `Move Body1 into
    /// Bracket`). The bodies are seen at `source_occurrence` and the
    /// destination at `destination_occurrence` (the first paths to their
    /// components when None), and keep their place in the design. Refused
    /// when the transfer fails or a feature that worked fails after it (a
    /// feature of the source that works on a body later on).
    pub fn transfer_bodies(
        &mut self,
        bodies: &[BodyUid],
        source_occurrence: Option<&[OccurrenceUid]>,
        destination: ComponentUid,
        destination_occurrence: Option<&[OccurrenceUid]>,
        name: Option<&str>,
    ) -> Result<Added, ModelError> {
        let source = self.transfer_source(bodies)?;
        if let Some(reason) = self.transfer_refusal(source, destination) {
            return Err(invalid(reason));
        }
        let link = OccurrenceLink {
            source: OccurrencePath(self.transfer_path(
                source,
                source_occurrence,
                "the bodies' occurrence",
            )?),
            target: OccurrencePath(self.transfer_path(
                destination,
                destination_occurrence,
                "the destination's occurrence",
            )?),
        };
        let what = match bodies {
            [one] => self.body_name(*one),
            _ => format!("{} bodies", bodies.len()),
        };
        let label = format!("Move {what} into {}", self.state.assembly.name(destination));
        let worked: BTreeSet<FeatureUid> = self
            .result
            .results
            .iter()
            .filter(|r| r.status.is_ok())
            .map(|r| r.uid)
            .collect();
        let def = FeatureDef::<ValueInput>::BodyTransfer(BodyTransferDef::new(
            bodies.to_vec(),
            link,
            None,
        ));
        self.apply_checked(
            |state| {
                let added = state.add_feature_in(&def, name, destination)?;
                Ok((label, added))
            },
            |state, result| {
                for r in &result.results {
                    let crate::recompute::FeatureStatus::Failed(message) = &r.status else {
                        continue;
                    };
                    let entry = state.entry(r.uid);
                    let added = entry.is_some_and(|e| matches!(e.def, FeatureDef::BodyTransfer(_)))
                        && !worked.contains(&r.uid)
                        && self_is_new(state, r.uid);
                    if added || worked.contains(&r.uid) {
                        let name = entry.map_or_else(|| r.uid.to_string(), |f| f.name.clone());
                        return Err(invalid(format!("{name} would fail: {message}")));
                    }
                }
                Ok(())
            },
        )
    }
}

/// The feature just before the marker: the one the command added.
fn self_is_new(state: &DocState, uid: FeatureUid) -> bool {
    state
        .marker
        .checked_sub(1)
        .and_then(|i| state.features.get(i))
        .is_some_and(|f| f.uid == uid)
}
