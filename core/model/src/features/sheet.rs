// SPDX-License-Identifier: MIT
//! Fabrication rules and a base sheet built from named bounded profiles.

use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureDef, FeatureInfo, FeatureOutput,
    ProfileRef, References,
};
use crate::ids::{BodyUid, FeatureUid};
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::sheet::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Allowance<P> {
    KFactor { k: P },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliefKind {
    Rectangular,
    Round,
    Tear,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relief<P> {
    #[serde(rename = "type")]
    pub kind: ReliefKind,
    pub width: P,
    pub depth: P,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetRulesDef<P = ParamId> {
    pub thickness: P,
    pub bend_radius: P,
    pub allowance: Allowance<P>,
    pub reference_side: SheetReferenceSide,
    pub bend_relief: Relief<P>,
    pub corner_relief: Relief<P>,
}

impl<P> SheetRulesDef<P> {
    pub const TYPE: &'static str = "sheet_rules";
    pub const BASE_NAME: &'static str = "SheetRules";
    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetRulesDef<Q>, E> {
        let Allowance::KFactor { k } = &self.allowance;
        Ok(SheetRulesDef {
            thickness: f("thickness", &self.thickness)?,
            bend_radius: f("bend_radius", &self.bend_radius)?,
            allowance: Allowance::KFactor {
                k: f("allowance.k", k)?,
            },
            reference_side: self.reference_side,
            bend_relief: Relief {
                kind: self.bend_relief.kind,
                width: f("bend_relief.width", &self.bend_relief.width)?,
                depth: f("bend_relief.depth", &self.bend_relief.depth)?,
            },
            corner_relief: Relief {
                kind: self.corner_relief.kind,
                width: f("corner_relief.width", &self.corner_relief.width)?,
                depth: f("corner_relief.depth", &self.corner_relief.depth)?,
            },
        })
    }
}

impl SheetRulesDef<f64> {
    fn validate(&self) -> Result<(), String> {
        self.map_params(&mut |slot, value| {
            let good = value.is_finite()
                && if slot == "allowance.k" {
                    (0.0..=1.0).contains(value)
                } else {
                    *value > TOLERANCE
                };
            if good {
                Ok(*value)
            } else {
                Err(format!("invalid sheet rule {slot}: {value}"))
            }
        })
        .map(|_| ())
    }
}

impl FeatureInfo for SheetRulesDef {
    fn references(&self) -> References {
        References::default()
    }
    fn check(&self, _ctx: &CheckContext<'_>) -> Result<(), String> {
        Ok(())
    }
    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        self.map_params(&mut |_, id| Ok::<_, String>(value(*id)))?
            .validate()
    }
}

impl<K: Kernel> Evaluate<K> for SheetRulesDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let rules = self.map_params(&mut |_, id| ctx.param(*id))?;
        rules.validate()?;
        Ok(FeatureOutput {
            sheet_rules: Some(rules),
            ..FeatureOutput::default()
        })
    }
}

/// Settled semantic allocation. Removed references remain as tombstones so
/// that adding another region never reuses a previous panel's identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelSlot {
    pub profile: ProfileRef,
    pub slot: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetFaceDef<P = ParamId> {
    pub rules: FeatureUid,
    #[serde(deserialize_with = "super::extrude::profile_set")]
    pub profiles: Vec<ProfileRef>,
    /// A face joined to this sheet body (mitcad#242): the regions lie on
    /// the front or back surface of one of its planar panels and are added
    /// to that panel's material, through its thickness. Without it the
    /// face makes new bodies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<BodyUid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panel_slots: Vec<PanelSlot>,
    #[serde(skip)]
    pub marker: PhantomData<P>,
}

impl<P> SheetFaceDef<P> {
    pub const TYPE: &'static str = "sheet_face";
    pub const BASE_NAME: &'static str = "SheetFace";
    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetFaceDef<Q>, E> {
        Ok(SheetFaceDef {
            rules: self.rules,
            profiles: self.profiles.clone(),
            body: self.body,
            panel_slots: self.panel_slots.clone(),
            marker: PhantomData,
        })
    }

    pub(crate) fn settle(&mut self, old: Option<&Self>) {
        if let Some(old) = old {
            self.panel_slots = old.panel_slots.clone();
        }
        let mut missing: Vec<_> = self
            .profiles
            .iter()
            .filter(|p| !self.panel_slots.iter().any(|s| &s.profile == *p))
            .cloned()
            .collect();
        missing.sort_by_key(|p| (p.sketch, p.region.to_string()));
        let mut next = self
            .panel_slots
            .iter()
            .map(|s| s.slot)
            .max()
            .map_or(0, |s| s.saturating_add(1));
        for profile in missing {
            self.panel_slots.push(PanelSlot {
                profile,
                slot: next,
            });
            next = next.saturating_add(1);
        }
    }
}

impl FeatureInfo for SheetFaceDef {
    fn references(&self) -> References {
        let mut refs = References {
            features: self
                .profiles
                .iter()
                .map(|p| p.sketch)
                .chain([self.rules])
                .collect(),
            ..References::default()
        };
        if let Some(body) = self.body {
            refs.body(body);
        }
        refs
    }
    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        super::extrude::check_profiles(ctx, &self.profiles)?;
        if let Some(body) = self.body {
            ctx.body(body)?;
        }
        if !matches!(ctx.feature(self.rules)?.def, FeatureDef::SheetRules(_)) {
            return Err(format!("{} is not a sheet rules feature", self.rules));
        }
        let mut slots = BTreeSet::new();
        for (i, slot) in self.panel_slots.iter().enumerate() {
            if !slots.insert(slot.slot)
                || self.panel_slots[..i]
                    .iter()
                    .any(|s| s.profile == slot.profile)
            {
                return Err("sheet panel allocation contains duplicate identities".into());
            }
        }
        Ok(())
    }
    fn creates_bodies(&self) -> bool {
        true
    }
    fn profiles(&self) -> &[ProfileRef] {
        &self.profiles
    }
}

impl<K: Kernel> Evaluate<K> for SheetFaceDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        if let Some(body) = self.body {
            return crate::sheet::folded::join_face(self, body, ctx);
        }
        let rules = ctx.sheet_rules(self.rules)?;
        let sketch_uid = self.profiles.first().ok_or("no profiles selected")?.sketch;
        let sketch = ctx.sketch(sketch_uid)?;
        let mut settled = self.clone();
        settled.settle(None);
        let mut panels = Vec::new();
        for profile in &self.profiles {
            // Semantic identity must not silently follow a nearest region.
            let region = sketch
                .regions
                .iter()
                .find(|r| r.key == profile.region)
                .ok_or_else(|| format!("sheet profile {} is unavailable", profile.region))?;
            let slot = settled
                .panel_slots
                .iter()
                .find(|s| s.profile == *profile)
                .expect("settled")
                .slot;
            panels.push(SheetPlanarPatch {
                id: SheetDomainId {
                    feature: ctx.uid,
                    slot,
                },
                frame: sketch.frame,
                region: SheetRegion {
                    loops: region
                        .loops
                        .iter()
                        .map(|l| SheetLoop {
                            segments: l
                                .segments
                                .iter()
                                .map(|s| SheetSegment {
                                    id: SheetCurveId::Sketch {
                                        sketch: sketch_uid,
                                        segment: s.key.clone(),
                                    },
                                    geometry: s.geometry.clone(),
                                })
                                .collect(),
                        })
                        .collect(),
                },
            });
        }
        panels.sort_by_key(|p| p.id);
        let charts: Vec<_> = panels
            .iter()
            .map(|p| SheetPatch::from_legacy(p, rules.value.thickness, rules.value.reference_side))
            .collect::<Result<_, _>>()?;
        let joins = crate::sheet::planar_joins(&charts)?;
        let spec = SheetShapeSpec {
            thickness: rules.value.thickness,
            patches: &charts,
            ports: &[],
            tolerance: TOLERANCE,
        };
        let geometry = ctx
            .kernel
            .sheet_shape(&spec, &ctx.uid.to_string())
            .map_err(|e| e.to_string())?;
        if geometry.solids.is_empty() {
            return Err("sheet Face produced no solid".into());
        }
        let owner = ctx
            .env
            .features
            .iter()
            .find(|f| f.uid == ctx.uid)
            .expect("current feature")
            .component;
        let mut output = FeatureOutput::default();
        let mut assigned = BTreeSet::new();
        for solid in geometry.solids {
            if solid.membership.is_empty()
                || solid.membership.iter().any(|member| {
                    member.region != 0
                        || !charts.iter().any(|p| p.domain == member.domain)
                        || !assigned.insert(*member)
                })
            {
                return Err("sheet kernel returned ambiguous domain membership".into());
            }
            let slot = solid
                .membership
                .iter()
                .map(|member| member.domain.slot())
                .min()
                .expect("not empty");
            let body = BodyUid::new(ctx.uid, slot);
            output.changes.push(BodyChange::Set(body, solid.shape));
            output.sheets.insert(
                body,
                SheetChange::Set(SheetState {
                    schema: 2,
                    algorithm: ALGORITHM_VERSION,
                    body,
                    owner,
                    shape_generation: String::new(),
                    rules: self.rules,
                    rules_version: format!("{:032x}", rules.version),
                    panels: panels
                        .iter()
                        .filter(|p| {
                            solid
                                .membership
                                .iter()
                                .any(|m| m.domain == SheetDomainRef::Panel(p.id))
                        })
                        .cloned()
                        .collect(),
                    charts: charts
                        .iter()
                        .filter(|p| solid.membership.iter().any(|m| m.domain == p.domain))
                        .cloned()
                        .collect(),
                    bends: Vec::new(),
                    joins: joins
                        .iter()
                        .filter(|j| {
                            [j.first.boundary.domain, j.second.boundary.domain]
                                .iter()
                                .all(|domain| solid.membership.iter().any(|m| m.domain == *domain))
                        })
                        .cloned()
                        .collect(),
                    opened: Vec::new(),
                    lineage: Vec::new(),
                    overlays: Vec::new(),
                    invalidated: None,
                    blockers: Vec::new(),
                }),
            );
        }
        if assigned.len() != panels.len() {
            return Err("sheet kernel omitted a material domain".into());
        }
        for message in geometry.diagnostics {
            ctx.warn(message);
        }
        Ok(output)
    }
}
