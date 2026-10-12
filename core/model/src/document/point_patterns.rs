// SPDX-License-Identifier: MIT
//! Definition-state occurrence allocation for Pattern at Points. Recompute
//! controls availability; edits reuse IDs by source path and target reference.

use super::{DocState, ModelError};
use crate::features::point_pattern::PatternInstance;
use crate::features::{FeatureDef, PatternObjects};
use crate::ids::{ComponentUid, FeatureUid};
use crate::transform::Transform;

impl DocState {
    pub(crate) fn sync_point_pattern(&mut self, uid: FeatureUid) -> Result<(), ModelError> {
        let entry = self.entry(uid).expect("inserted feature").clone();
        let FeatureDef::PatternAtPoints(def) = &entry.def else {
            return Ok(());
        };
        let mut desired = Vec::new();
        if let PatternObjects::Components { occurrences } = &def.objects {
            for source in occurrences {
                let Ok(component) =
                    crate::joints::path_component(&self.assembly, ComponentUid::ROOT, &source.0)
                else {
                    continue;
                };
                for target in &def.targets {
                    let key = PatternInstance {
                        feature: uid,
                        source: source.clone(),
                        target: target.clone(),
                    };
                    let existing = self
                        .assembly
                        .occurrences
                        .iter()
                        .find(|o| o.pattern.as_ref() == Some(&key))
                        .map(|o| o.uid);
                    let instance = existing.unwrap_or_else(|| {
                        self.assembly
                            .place(component, entry.component, Transform::IDENTITY)
                    });
                    let occurrence = self.assembly.occurrence_mut(instance).expect("placed");
                    occurrence.component = component;
                    occurrence.parent = entry.component;
                    occurrence.pattern = Some(key);
                    desired.push(instance);
                }
            }
        }
        self.assembly.occurrences.retain(|o| {
            o.pattern
                .as_ref()
                .is_none_or(|p| p.feature != uid || desired.contains(&o.uid))
        });
        Ok(())
    }
}
