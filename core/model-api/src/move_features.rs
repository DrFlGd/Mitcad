// SPDX-License-Identifier: MIT
//! Commands and queries of Move Features into a Component (mitcad#171, see
//! `commands.md`, "Moving features into components").

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use super::dispatch::named_enum;
use super::prelude::*;
use mitcad_model::features::{FeatureDef, OccurrencePath, ValueInput};
use mitcad_model::ids::{BodyUid, ComponentUid, FeatureUid};
use mitcad_model::internal::document::MoveDestination;
use mitcad_model::kernel::Kernel;
use mitcad_model::parameters::ParamId;
use mitcad_model::{Document, MoveGroup, MovePlan, PlannedGroup};

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
    #[serde(default)]
    source_occurrence: Option<OccurrencePath>,
    #[serde(default)]
    destination: Option<MoveDestination>,
    #[serde(default)]
    together: bool,
}

named_enum! {
    // Parsed once per command and dropped: the size difference costs nothing.
    #[allow(clippy::large_enum_variant)]
    #[derive(Debug, Deserialize)]
    #[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
    pub(crate) enum MoveCommand {
        "move_to_component" => MoveToComponent {
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
            #[serde(default)]
            source_occurrence: Option<OccurrencePath>,
            #[serde(default)]
            destination: Option<MoveDestination>,
        },
        "move_history_into_component" => MoveHistoryIntoComponent {
            feature: FeatureUid,
        },
        "set_hybrid_assembly" => SetHybridAssembly {
            #[serde(default = "yes")]
            allowed: bool,
        },
        /// Bodies moved into an existing component on the timeline, by a Body
        /// Transfer (mitcad#465).
        "transfer_bodies" => TransferBodies {
            bodies: Vec<BodyUid>,
            #[serde(default)]
            source_occurrence: Option<OccurrencePath>,
            destination: TransferTarget,
            #[serde(default)]
            name: Option<String>,
        },
    }
}

/// Where a body transfer moves bodies: a component, seen at an occurrence
/// path from the root (its first path when left out).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransferTarget {
    component: String,
    #[serde(default)]
    occurrence: Option<OccurrencePath>,
}

/// The `transfer_bodies` query: the bodies' component and the components
/// they can move into.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransferQuery {
    bodies: Vec<BodyUid>,
    #[serde(default)]
    source_occurrence: Option<OccurrencePath>,
}

fn yes() -> bool {
    true
}

/// The `feature_references` query: what a definition (a command panel's,
/// before it is added) refers to.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReferencesQuery {
    def: FeatureDef<ValueInput>,
}

/// The document's methods of this module used across the model, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(crate) trait MoveFeaturesCrateOps<K: Kernel> {
    fn run_move_command(&mut self, command: MoveCommand) -> Result<Value, ApiError>;

    /// The `transfer_bodies` query: the component the bodies are in and
    /// every placed component (each occurrence path, as the
    /// `move_to_component` query's `destinations`) with whether they can go
    /// there.
    fn transfer_plan_json(&self, query: &TransferQuery) -> Result<Value, ApiError>;

    /// The `move_to_component` query: what the command would do.
    fn move_plan_json(&self, selection: &MoveSelection) -> Result<Value, ApiError>;

    /// The `feature_references` query: the features a definition refers
    /// to and their components.
    fn feature_references_json(&self, query: &ReferencesQuery) -> Value;
}

impl<K: Kernel> MoveFeaturesCrateOps<K> for Document<K> {
    fn run_move_command(&mut self, command: MoveCommand) -> Result<Value, ApiError> {
        Ok(match command {
            MoveCommand::MoveToComponent {
                component,
                bodies,
                features,
                groups,
                name,
                names,
                together,
                source_occurrence,
                destination,
            } => {
                let selection = MoveSelection {
                    component,
                    bodies,
                    features,
                    groups,
                    source_occurrence,
                    destination,
                    together,
                };
                let source = self.move_source(&selection)?;
                let groups = Self::move_groups(&selection)?;
                if let Some(destination) = &selection.destination {
                    if name.is_some() || !names.is_empty() {
                        return Err(ApiError(
                            "give the new name in destination, not name or names".to_owned(),
                        ));
                    }
                    let moved = self.move_to_destination(
                        source,
                        &groups,
                        selection.source_occurrence.as_ref(),
                        destination,
                        together,
                    )?;
                    return Ok(json!({"components": [{
                        "component": moved.component, "occurrence": moved.occurrence,
                        "name": self.assembly().name(moved.component), "created": moved.created,
                        "bodies": moved.bodies, "features": moved.features,
                    }]}));
                }
                if selection.source_occurrence.is_some() {
                    return Err(ApiError(
                        "source_occurrence requires an explicit destination".to_owned(),
                    ));
                }
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
            MoveCommand::TransferBodies {
                bodies,
                source_occurrence,
                destination,
                name,
            } => {
                let component = self.component_ref(&destination.component)?;
                let added = self.transfer_bodies(
                    &bodies,
                    source_occurrence.as_ref().map(|p| p.0.as_slice()),
                    component,
                    destination.occurrence.as_ref().map(|p| p.0.as_slice()),
                    name.as_deref(),
                )?;
                json!({"uid": added.uid, "name": added.name, "component": component,
                       "component_name": self.assembly().name(component),
                       "bodies": bodies})
            }
        })
    }

    fn transfer_plan_json(&self, query: &TransferQuery) -> Result<Value, ApiError> {
        let destinations = self.transfer_destinations(&query.bodies)?;
        let a = self.assembly();
        let source = self
            .body_component(query.bodies[0])
            .unwrap_or(ComponentUid::ROOT);
        let rows: Vec<Value> = destinations
            .iter()
            .map(|d| {
                let path = &d.occurrence;
                let label = if path.is_empty() {
                    a.root_name.clone()
                } else {
                    format!("{}/{}", a.root_name, a.path_name(path))
                };
                json!({
                    "component": d.component, "name": a.name(d.component),
                    "occurrence": OccurrencePath(path.clone()), "path": label,
                    "parent_occurrence": OccurrencePath(path[..path.len().saturating_sub(1)].to_vec()),
                    "eligible": d.reason.is_none(),
                    "reason": d.reason.clone().unwrap_or_default(),
                })
            })
            .collect();
        Ok(json!({
            "component": source,
            "name": a.name(source),
            "source_occurrence": query.source_occurrence,
            "bodies": query.bodies.iter()
                .map(|b| json!({"uid": b, "name": self.body_name(*b)}))
                .collect::<Vec<_>>(),
            "destinations": rows,
        }))
    }

    fn move_plan_json(&self, selection: &MoveSelection) -> Result<Value, ApiError> {
        let source = self.move_source(selection)?;
        let groups = Self::move_groups(selection)?;
        let mut plan = self.plan_move_to_components(source, &groups)?;
        let destinations = self.move_destinations_json(&plan, selection.source_occurrence.as_ref());
        if let Some(destination) = &selection.destination
            && let Err(error) = self.check_move_destination(
                &plan,
                selection.source_occurrence.as_ref(),
                destination,
            )
        {
            plan.problems.push(error.to_string());
        }
        let body = |b: &BodyUid| json!({"uid": b, "name": self.body_name(*b)});
        let feature = |f: &FeatureUid| {
            json!({
                "uid": f, "name": self.feature(*f).map(|e| e.name.clone()),
                "type": self.feature(*f).map(|e| e.def.type_name()),
            })
        };
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
            "destinations": destinations,
            "keep_together": selection.together,
        }))
    }

    fn feature_references_json(&self, query: &ReferencesQuery) -> Value {
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

/// The document's methods used only in this module, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
trait MoveFeaturesLocal<K: Kernel> {
    fn move_source(&self, selection: &MoveSelection) -> Result<ComponentUid, ApiError>;

    fn move_groups(selection: &MoveSelection) -> Result<Vec<MoveGroup>, ApiError>;

    fn move_destinations_json(
        &self,
        plan: &MovePlan,
        source_occurrence: Option<&OccurrencePath>,
    ) -> Vec<Value>;
}

impl<K: Kernel> MoveFeaturesLocal<K> for Document<K> {
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

    fn move_destinations_json(
        &self,
        plan: &MovePlan,
        source_occurrence: Option<&OccurrencePath>,
    ) -> Vec<Value> {
        let a = self.assembly();
        let components =
            std::iter::once(ComponentUid::ROOT).chain(a.components.iter().map(|c| c.uid));
        let mut destinations = Vec::new();
        for component in components {
            for path in a.paths_to(component) {
                let occurrence = OccurrencePath(path.clone());
                let parent_occurrence =
                    OccurrencePath(path[..path.len().saturating_sub(1)].to_vec());
                let existing = MoveDestination::Existing {
                    component,
                    occurrence: occurrence.clone(),
                };
                let new = MoveDestination::New {
                    parent: component,
                    parent_occurrence: occurrence.clone(),
                    name: None,
                };
                let reason = self
                    .check_move_destination(plan, source_occurrence, &existing)
                    .err()
                    .map(|e| e.to_string());
                let create_reason = self
                    .check_move_destination(plan, source_occurrence, &new)
                    .err()
                    .map(|e| e.to_string());
                let label = if path.is_empty() {
                    a.root_name.clone()
                } else {
                    format!("{}/{}", a.root_name, a.path_name(&path))
                };
                destinations.push(json!({
                    "component": component, "name": a.name(component),
                    "occurrence": occurrence, "path": label,
                    "parent_occurrence": parent_occurrence,
                    "eligible": reason.is_none(), "reason": reason.unwrap_or_default(),
                    "can_create": create_reason.is_none(),
                    "create_reason": create_reason.unwrap_or_default(),
                }));
            }
        }
        destinations
    }
}
