// SPDX-License-Identifier: MIT
//! Commands and queries of Move Features into a Component (mitcad#171, see
//! `commands.md`, "Moving features into components").

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use crate::document::{Document, MoveGroup, PlannedGroup};
use crate::features::{FeatureDef, ValueInput};
use crate::ids::{BodyUid, ComponentUid, FeatureUid};
use crate::kernel::Kernel;
use crate::parameters::ParamId;

/// What goes: `bodies` and `features` (one component), or `groups` of
/// bodies (a component each); none of them: every modelled feature.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MoveSelection {
    /// The component the features are in; the root when left out.
    #[serde(default)]
    component: Option<String>,
    #[serde(default)]
    bodies: Vec<BodyUid>,
    #[serde(default)]
    features: Vec<FeatureUid>,
    #[serde(default)]
    groups: Vec<Vec<BodyUid>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum MoveCommand {
    MoveToComponent {
        #[serde(default)]
        component: Option<String>,
        #[serde(default)]
        bodies: Vec<BodyUid>,
        #[serde(default)]
        features: Vec<FeatureUid>,
        #[serde(default)]
        groups: Vec<Vec<BodyUid>>,
        /// The new component's name (one component).
        #[serde(default)]
        name: Option<String>,
        /// The components' names, one per group.
        #[serde(default)]
        names: Vec<String>,
        /// Bodies a feature ties go into one component.
        #[serde(default)]
        together: bool,
    },
    MoveHistoryIntoComponent {
        feature: FeatureUid,
    },
    SetHybridAssembly {
        #[serde(default = "yes")]
        allowed: bool,
    },
}

fn yes() -> bool {
    true
}

/// The commands' names, to send them here.
pub(crate) const COMMANDS: [&str; 3] = [
    "move_to_component",
    "move_history_into_component",
    "set_hybrid_assembly",
];

/// The `feature_references` query: what a definition (a command panel's,
/// before it is added) refers to.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReferencesQuery {
    def: FeatureDef<ValueInput>,
}

impl<K: Kernel> Document<K> {
    fn move_source(&self, selection: &MoveSelection) -> Result<ComponentUid, ApiError> {
        selection
            .component
            .as_deref()
            .map_or(Ok(ComponentUid::ROOT), |c| self.component_ref(c))
    }

    fn move_groups(selection: &MoveSelection) -> Result<Vec<MoveGroup>, ApiError> {
        let one = !(selection.bodies.is_empty() && selection.features.is_empty());
        if one && !selection.groups.is_empty() {
            return Err(ApiError(
                "give either bodies and features, or groups, not both".to_owned(),
            ));
        }
        Ok(if one {
            vec![MoveGroup {
                bodies: selection.bodies.clone(),
                features: selection.features.clone(),
            }]
        } else {
            selection
                .groups
                .iter()
                .map(|bodies| MoveGroup {
                    bodies: bodies.clone(),
                    features: Vec::new(),
                })
                .collect()
        })
    }

    pub(crate) fn run_move_command(&mut self, command: MoveCommand) -> Result<Value, ApiError> {
        Ok(match command {
            MoveCommand::MoveToComponent {
                component,
                bodies,
                features,
                groups,
                name,
                names,
                together,
            } => {
                let selection = MoveSelection {
                    component,
                    bodies,
                    features,
                    groups,
                };
                let source = self.move_source(&selection)?;
                let groups = Self::move_groups(&selection)?;
                let names = match (name, names.is_empty()) {
                    (Some(name), true) => vec![name],
                    (None, _) => names,
                    (Some(_), false) => {
                        return Err(ApiError("give name or names, not both".to_owned()));
                    }
                };
                let moved = self.move_to_components(source, &groups, &names, together)?;
                let a = self.assembly();
                let items: Vec<Value> = moved
                    .iter()
                    .map(|m| {
                        json!({"component": m.component, "occurrence": m.occurrence,
                               "name": a.name(m.component), "bodies": m.bodies,
                               "features": m.features})
                    })
                    .collect();
                json!({"components": items})
            }
            MoveCommand::MoveHistoryIntoComponent { feature } => {
                let (component, features) = self.move_history_into_component(feature)?;
                json!({"component": component, "name": self.assembly().name(component),
                       "features": features})
            }
            MoveCommand::SetHybridAssembly { allowed } => {
                self.set_hybrid_assembly(allowed)?;
                json!({})
            }
        })
    }

    /// The `move_to_component` query: what the command would do.
    pub(crate) fn move_plan_json(&self, selection: &MoveSelection) -> Result<Value, ApiError> {
        let source = self.move_source(selection)?;
        let groups = Self::move_groups(selection)?;
        let plan = self.plan_move_to_components(source, &groups)?;
        let body = |b: &BodyUid| json!({"uid": b, "name": self.body_name(*b)});
        let feature =
            |f: &FeatureUid| json!({"uid": f, "name": self.feature(*f).map(|e| e.name.clone())});
        let group = |g: &PlannedGroup| {
            json!({"bodies": g.bodies.iter().map(body).collect::<Vec<_>>(),
                   "features": g.features.iter().map(feature).collect::<Vec<_>>()})
        };
        Ok(json!({
            "component": source,
            "name": self.assembly().name(source),
            "everything": plan.everything,
            "bodies": self.component_bodies(source).iter().map(body).collect::<Vec<_>>(),
            "hybrid_assembly": self.hybrid_assembly(),
            "groups": plan.groups.iter().map(group).collect::<Vec<_>>(),
            "conflicts": plan.conflicts.iter().map(|c| json!({
                "feature": c.feature,
                "name": self.feature(c.feature).map(|e| e.name.clone()),
                "groups": c.groups,
                "staying": c.staying.iter().map(body).collect::<Vec<_>>(),
                "message": c.message,
            })).collect::<Vec<_>>(),
            "together": plan.together.as_ref().map(|t| t.iter().map(group).collect::<Vec<_>>()),
            "problems": plan.problems,
        }))
    }

    /// The `feature_references` query: the features a definition refers
    /// to and their components.
    pub(crate) fn feature_references_json(&self, query: &ReferencesQuery) -> Value {
        let def = query
            .def
            .map_params(&mut |_, _| Ok::<_, ()>(ParamId::from_raw(0)))
            .expect("never fails");
        let features: Vec<FeatureUid> = def.info().references().features.into_iter().collect();
        let mut components: Vec<ComponentUid> = features
            .iter()
            .filter_map(|f| self.feature(*f).map(|e| e.component))
            .collect();
        components.sort();
        components.dedup();
        json!({"features": features, "components": components})
    }
}
