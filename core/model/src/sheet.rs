// SPDX-License-Identifier: MIT
//! Immutable material domains paired with the current named body shape.

use crate::ids::{BodyUid, ComponentUid, FeatureUid};
use crate::profile::{SegmentGeometry, SketchFrame};
use crate::topo::SegmentKey;
use crate::transform::Transform;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

mod adapter;
mod charts;
pub(crate) use adapter::planar_joins;
pub(crate) mod folded;
// The derived whole-body Flat Pattern (mitcad#208).
pub mod flat;
// Its DXF drawing and picks resolved to semantic references (mitcad#210).
pub mod flat_dxf;
pub mod pick;
pub use charts::*;

pub const TOLERANCE: f64 = 1.0e-7;
pub const ALGORITHM_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SheetDomainId {
    pub feature: FeatureUid,
    pub slot: u32,
}

impl fmt::Display for SheetDomainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.p{}", self.feature, self.slot)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SheetCurveId {
    Sketch {
        sketch: FeatureUid,
        segment: SegmentKey,
    },
    Generated {
        feature: FeatureUid,
        slot: u32,
    },
}

impl fmt::Display for SheetCurveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sketch { sketch, segment } => write!(f, "{sketch}/{segment}"),
            Self::Generated { feature, slot } => write!(f, "{feature}/g{slot}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetSegment {
    pub id: SheetCurveId,
    pub geometry: SegmentGeometry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetLoop {
    pub segments: Vec<SheetSegment>,
}

/// First loop is the exterior, subsequent loops are holes. Curves are exact
/// sketch curves, not a sampled silhouette or a bounding rectangle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetRegion {
    pub loops: Vec<SheetLoop>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetPlanarPatch {
    pub id: SheetDomainId,
    pub frame: SketchFrame,
    pub region: SheetRegion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetReferenceSide {
    Front,
    Back,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetDiagnostic {
    pub feature: FeatureUid,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetState {
    pub schema: u32,
    pub algorithm: u32,
    pub body: BodyUid,
    pub owner: ComponentUid,
    /// Hex fingerprint of the exact shape paired with these domains.
    pub shape_generation: String,
    pub rules: FeatureUid,
    pub rules_version: String,
    pub panels: Vec<SheetPlanarPatch>,
    #[serde(default)]
    pub charts: Vec<SheetPatch>,
    #[serde(default)]
    pub bends: Vec<BendDomain>,
    #[serde(default)]
    pub joins: Vec<ShapePort>,
    #[serde(default)]
    pub opened: Vec<OpenedBend>,
    #[serde(default)]
    pub lineage: Vec<CurveLineage>,
    pub invalidated: Option<SheetDiagnostic>,
    pub blockers: Vec<SheetDiagnostic>,
}

impl SheetState {
    pub fn status(&self) -> &'static str {
        if self.invalidated.is_some() {
            "invalid"
        } else if !self.blockers.is_empty() {
            "blocked"
        } else {
            "valid"
        }
    }

    pub(crate) fn transformed(&self, transform: &Transform) -> Self {
        let mut next = self.clone();
        for panel in &mut next.panels {
            panel.frame = transform.apply_frame(&panel.frame);
        }
        for chart in &mut next.charts {
            chart.embedding = chart.embedding.transformed(transform);
        }
        for bend in &mut next.bends {
            if let SheetEmbedding::Cylinder(cylinder) =
                SheetEmbedding::Cylinder(bend.folded_embedding.clone()).transformed(transform)
            {
                bend.folded_embedding = cylinder;
            }
        }
        next
    }
}

/// Every replacement either supplies domains, deliberately transforms them,
/// or invalidates them. Absence also invalidates an existing sheet entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum SheetChange {
    Set(SheetState),
    Transform(Transform),
    Invalidate(String),
    Remove,
}

pub(crate) type SheetStates = BTreeMap<BodyUid, crate::recompute::Versioned<SheetState>>;
pub(crate) type SheetRules =
    BTreeMap<FeatureUid, crate::recompute::Versioned<crate::features::sheet::SheetRulesDef<f64>>>;

fn version(state: &SheetState) -> u128 {
    let mut fp = crate::fingerprint::Fingerprint::new("sheet-state-v1");
    serde_json::to_writer(&mut fp, state).expect("sheet state serializes");
    fp.finish()
}

pub(crate) fn publish(
    states: &mut SheetStates,
    body: BodyUid,
    owner: ComponentUid,
    generation: u128,
    feature: FeatureUid,
    change: Option<&SheetChange>,
) {
    let previous = states.get(&body).map(|v| v.value.clone());
    let next =
        match change {
            Some(SheetChange::Set(state)) => Some(state.clone()),
            Some(SheetChange::Transform(t)) if t.is_rigid() => previous.map(|s| s.transformed(t)),
            Some(SheetChange::Remove) => None,
            _ => {
                previous.map(|mut s| {
                    s.invalidated = Some(SheetDiagnostic { feature, message: match change {
                Some(SheetChange::Invalidate(message)) => message.clone(),
                _ => "ordinary modelling replaced this body without a sheet-domain adapter".into(),
            }});
                    s
                })
            }
        };
    if let Some(mut state) = next {
        state.body = body;
        state.owner = owner;
        state.shape_generation = format!("{generation:032x}");
        states.insert(
            body,
            crate::recompute::Versioned {
                version: version(&state),
                value: state,
            },
        );
    } else {
        states.remove(&body);
    }
}

pub(crate) fn block(
    states: &mut SheetStates,
    bodies: &[BodyUid],
    feature: FeatureUid,
    message: &str,
) {
    for body in bodies {
        if let Some(state) = states.get_mut(body) {
            state.value.blockers.retain(|b| b.feature != feature);
            state.value.blockers.push(SheetDiagnostic {
                feature,
                message: message.into(),
            });
            state.version = version(&state.value);
        }
    }
}

pub(crate) fn state_query<S>(
    result: &crate::recompute::Recomputed<S>,
    body: Option<BodyUid>,
) -> serde_json::Value {
    use serde_json::json;
    let one = |body: BodyUid| {
        let Some(state) = result.sheets.get(&body) else {
            return json!({"body":body,"status":"unavailable"});
        };
        let Some(shape) = result.bodies.get(&body) else {
            return json!({"body":body,"status":"unavailable"});
        };
        if state.value.shape_generation != format!("{:032x}", shape.version)
            || result.owners.get(&body) != Some(&state.value.owner)
        {
            return json!({"body":body,"status":"unavailable","diagnostics":["sheet state does not match current geometry"]});
        }
        let mut value = json!(state.value);
        value["status"] = json!(state.value.status());
        value["version"] = json!(format!("{:032x}", state.version));
        value["diagnostics"] = json!(
            state
                .value
                .invalidated
                .iter()
                .chain(&state.value.blockers)
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
        value
    };
    body.map_or_else(
        || json!(result.sheets.keys().map(|b| one(*b)).collect::<Vec<_>>()),
        one,
    )
}

pub(crate) fn rules_query<S>(
    result: &crate::recompute::Recomputed<S>,
    feature: Option<FeatureUid>,
) -> serde_json::Value {
    use serde_json::json;
    let one = |feature: FeatureUid| match result.sheet_rules.get(&feature) {
        Some(rules) => {
            json!({"feature":feature,"status":"valid","version":format!("{:032x}",rules.version),"rules":rules.value})
        }
        None => json!({"feature":feature,"status":"unavailable"}),
    };
    feature.map_or_else(
        || {
            json!(
                result
                    .sheet_rules
                    .keys()
                    .map(|f| one(*f))
                    .collect::<Vec<_>>()
            )
        },
        one,
    )
}

/// Copies and generic split pieces are ordinary solids until an explicit
/// sheet adapter exists. Keep that invalidity observable on the new body.
pub(crate) fn complete_changes<K: crate::kernel::Kernel>(
    ctx: &crate::features::EvalContext<'_, K>,
    output: &mut crate::features::FeatureOutput<K::Shape>,
) {
    let source = ctx.reads.iter().find_map(|r| match r {
        crate::recompute::Read::Sheet(body, _) => ctx.env.sheets.get(body),
        _ => None,
    });
    for change in &output.changes {
        if let crate::features::BodyChange::Set(body, _) = change
            && !ctx.env.sheets.contains_key(body)
            && !output.sheets.contains_key(body)
            && let Some(source) = source
        {
            let mut state = source.value.clone();
            state.body = *body;
            state.panels.clear();
            state.charts.clear();
            state.bends.clear();
            state.joins.clear();
            state.opened.clear();
            state.lineage.clear();
            state.invalidated = Some(SheetDiagnostic {
                feature: ctx.uid,
                message: "a generic copy or split has no sheet-domain adapter".into(),
            });
            output.sheets.insert(*body, SheetChange::Set(state));
        }
    }
}

/// Declares intended mutation targets before resolving parameters or calling
/// the kernel. Failures therefore still block the affected material and keep
/// dependency reads, including empty automatic target sets.
pub(crate) fn footprint<K: crate::kernel::Kernel>(
    def: &crate::features::FeatureDef,
    ctx: &mut crate::features::EvalContext<'_, K>,
) {
    use crate::features::{FeatureDef as F, PatternObjects};
    let mut bodies = Vec::new();
    let mut sources = Vec::new();
    let mut all = false;
    if let Some(tool) = def.info().tool_use()
        && !tool.operation.makes_bodies()
    {
        all = tool.participants.is_empty();
        bodies.extend(tool.participants);
    }
    let objects = match def {
        F::SheetFlange(d) => {
            bodies.push(d.body);
            None
        }
        F::SheetBend(d) => {
            bodies.extend([d.body, d.moving_body.unwrap_or(d.body)]);
            None
        }
        F::SheetFold(d) => {
            bodies.push(d.body);
            None
        }
        F::SheetCornerRound(d) => {
            bodies.push(d.body);
            None
        }
        F::SheetCornerChamfer(d) => {
            bodies.push(d.body);
            None
        }
        F::SheetUnfold(d) => {
            bodies.push(d.body);
            None
        }
        F::SheetRefold(d) => {
            bodies.push(d.body);
            None
        }
        F::SheetCornerRelief(d) => {
            bodies.push(d.body);
            None
        }
        F::SheetCut(d) => {
            bodies.push(d.body);
            None
        }
        F::Fillet(d) => {
            bodies.push(d.body);
            None
        }
        F::Chamfer(d) => {
            bodies.push(d.body);
            None
        }
        F::Shell(d) => {
            bodies.push(d.body);
            None
        }
        F::Draft(d) => {
            bodies.push(d.body);
            None
        }
        F::OffsetFace(d) => {
            bodies.push(d.body);
            None
        }
        F::DeleteFace(d) => {
            bodies.push(d.body);
            None
        }
        F::ReplaceFace(d) => {
            bodies.push(d.body);
            None
        }
        F::SplitFace(d) => {
            bodies.push(d.body);
            None
        }
        F::SplitBody(d) => {
            bodies.extend(&d.bodies);
            None
        }
        F::Move(d) => {
            if d.copy {
                sources.extend(&d.bodies);
            } else {
                bodies.extend(&d.bodies);
            }
            None
        }
        F::Align(d) => {
            bodies.extend(&d.bodies);
            None
        }
        F::Scale(d) => {
            bodies.extend(&d.bodies);
            None
        }
        F::ComponentFromBodies(d) => {
            bodies.extend(&d.bodies);
            None
        }
        F::RemoveBody(d) => {
            bodies.extend(&d.bodies);
            None
        }
        F::Combine(d) => {
            bodies.push(d.target);
            None
        }
        F::Thread(d) if d.modeled => {
            bodies.extend(d.faces.iter().map(|f| f.body));
            None
        }
        F::Base(d) => {
            bodies.extend(&d.replaces);
            None
        }
        F::Hole(d) => {
            all = d.participants.is_empty();
            bodies.extend(&d.participants);
            None
        }
        F::RectangularPattern(d) => Some(&d.objects),
        F::CircularPattern(d) => Some(&d.objects),
        F::PathPattern(d) => Some(&d.objects),
        F::Mirror(d) => {
            if d.combine
                && let PatternObjects::Bodies { bodies: selected } = &d.objects
            {
                bodies.extend(selected);
            }
            Some(&d.objects)
        }
        F::PatternAtPoints(d) => Some(&d.objects),
        F::Extrude(d) if d.cut_scope.is_some() => {
            all = false;
            let scope = d.cut_scope.as_ref().expect("checked");
            ctx.reads.push(crate::recompute::Read::Assembly(
                crate::features::extrude_scope::assembly_key(&ctx.env),
            ));
            let components: Vec<_> = std::iter::once(ComponentUid::ROOT)
                .chain(ctx.env.assembly.components.iter().map(|c| c.uid))
                .collect();
            for component in components {
                // Discovery reads the set even when empty: a repaired or new
                // participant invalidates the failure cache and its blockers.
                let candidates = ctx.bodies_in(component);
                let selected = match &scope.targets {
                    crate::features::extrude_scope::TargetScope::Automatic { excluded, .. } => {
                        !excluded.iter().any(|t| t.component == component)
                    }
                    crate::features::extrude_scope::TargetScope::Explicit { included, .. } => {
                        included.iter().any(|t| t.component == component)
                    }
                };
                if selected {
                    bodies.extend(
                        candidates
                            .into_iter()
                            .map(|(b, _)| b)
                            .filter(|b| !scope.excluded_bodies().iter().any(|e| e.body == *b)),
                    );
                }
            }
            None
        }
        _ => None,
    };
    if let Some(objects) = objects {
        match objects {
            PatternObjects::Bodies { bodies: selected } => sources.extend(selected),
            PatternObjects::Faces { body, .. } => bodies.push(*body),
            PatternObjects::Features { features } => {
                for uid in features {
                    if let Some(entry) = ctx.env.features.iter().find(|f| f.uid == *uid).cloned() {
                        footprint(&entry.def, ctx);
                    }
                }
            }
            PatternObjects::Components { .. } => {}
        }
    }
    if all {
        bodies.extend(ctx.bodies().into_iter().map(|(b, _)| b));
    }
    for body in sources {
        ctx.sheet(body);
    }
    bodies.sort();
    bodies.dedup();
    for body in bodies {
        if let Some(owner) = ctx.env.owners.get(&body).copied() {
            let version = ctx
                .env
                .components
                .get(&owner)
                .and_then(|b| b.get(&body))
                .map(|b| b.version);
            ctx.reads
                .push(crate::recompute::Read::BodyIn(owner, body, version));
        } else {
            ctx.reads.push(crate::recompute::Read::Body(body, None));
        }
        ctx.sheet(body);
        ctx.sheet_footprint.insert(body);
    }
}
