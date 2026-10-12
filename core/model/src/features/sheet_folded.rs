// SPDX-License-Identifier: MIT
//! Editable folded sheet operations; geometry is shared with material trimming.

use super::sheet::Relief;
use super::{
    CheckContext, EvalContext, Evaluate, FeatureDef, FeatureInfo, FeatureOutput, References,
};
use crate::ids::{BodyUid, EntityUid, FeatureUid};
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::sheet::{BendPortMode, BoundaryRef, PanelId, SheetCurveId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetPosition {
    Inside,
    Outside,
    #[default]
    BendLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetLengthMode {
    Tangent,
    Inner,
    Outer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetLength<P> {
    pub mode: SheetLengthMode,
    pub value: P,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetOffsets<P> {
    pub start: P,
    pub end: P,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "P: Deserialize<'de>"))]
pub struct SheetFlangeDef<P = ParamId> {
    pub body: BodyUid,
    pub boundary: BoundaryRef,
    pub angle: P,
    pub length: SheetLength<P>,
    #[serde(default)]
    pub position: SheetPosition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offsets: Option<SheetOffsets<P>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<P>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relief: Option<Relief<P>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "P: Deserialize<'de>"))]
pub struct SheetBendDef<P = ParamId> {
    pub body: BodyUid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moving_body: Option<BodyUid>,
    pub stationary: BoundaryRef,
    pub moving: BoundaryRef,
    pub angle: P,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_mode: Option<BendPortMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<P>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relief: Option<Relief<P>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationarySide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldLine {
    pub sketch: FeatureUid,
    #[serde(with = "crate::ids::curve_serde")]
    pub curve: EntityUid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "P: Deserialize<'de>"))]
pub struct SheetFoldDef<P = ParamId> {
    pub body: BodyUid,
    pub panel: PanelId,
    pub line: FoldLine,
    pub stationary_side: StationarySide,
    pub angle: P,
    #[serde(default)]
    pub position: SheetPosition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<P>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relief: Option<Relief<P>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetCorner {
    pub panel: PanelId,
    pub incoming: SheetCurveId,
    pub outgoing: SheetCurveId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incoming_piece: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outgoing_piece: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CornerSlot {
    pub corner: SheetCorner,
    pub slot: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "P: Deserialize<'de>"))]
pub struct SheetCornerRoundDef<P = ParamId> {
    pub body: BodyUid,
    pub corners: Vec<SheetCorner>,
    pub radius: P,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corner_slots: Vec<CornerSlot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CornerDistances<P> {
    pub first: P,
    pub second: P,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "P: Deserialize<'de>"))]
pub struct SheetCornerChamferDef<P = ParamId> {
    pub body: BodyUid,
    pub corners: Vec<SheetCorner>,
    pub distances: CornerDistances<P>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corner_slots: Vec<CornerSlot>,
}

fn map_relief<P, Q, E>(
    r: &Option<Relief<P>>,
    f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
) -> Result<Option<Relief<Q>>, E> {
    r.as_ref()
        .map(|r| {
            Ok(Relief {
                kind: r.kind,
                width: f("relief.width", &r.width)?,
                depth: f("relief.depth", &r.depth)?,
            })
        })
        .transpose()
}

impl<P> SheetFlangeDef<P> {
    pub const TYPE: &'static str = "sheet_flange";
    pub const BASE_NAME: &'static str = "SheetFlange";
    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetFlangeDef<Q>, E> {
        Ok(SheetFlangeDef {
            body: self.body,
            boundary: self.boundary.clone(),
            angle: f("angle", &self.angle)?,
            length: SheetLength {
                mode: self.length.mode,
                value: f("length.value", &self.length.value)?,
            },
            position: self.position,
            offsets: self
                .offsets
                .as_ref()
                .map(|o| {
                    Ok(SheetOffsets {
                        start: f("offsets.start", &o.start)?,
                        end: f("offsets.end", &o.end)?,
                    })
                })
                .transpose()?,
            radius: self.radius.as_ref().map(|p| f("radius", p)).transpose()?,
            relief: map_relief(&self.relief, f)?,
        })
    }
}
impl<P> SheetBendDef<P> {
    pub const TYPE: &'static str = "sheet_bend";
    pub const BASE_NAME: &'static str = "SheetBend";
    pub fn resolved_mode(&self) -> BendPortMode {
        self.port_mode
            .unwrap_or(if self.moving_body.is_some_and(|b| b != self.body) {
                BendPortMode::TangentPorts
            } else {
                BendPortMode::VirtualSharp
            })
    }
    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetBendDef<Q>, E> {
        Ok(SheetBendDef {
            body: self.body,
            moving_body: self.moving_body,
            stationary: self.stationary.clone(),
            moving: self.moving.clone(),
            angle: f("angle", &self.angle)?,
            port_mode: Some(self.resolved_mode()),
            radius: self.radius.as_ref().map(|p| f("radius", p)).transpose()?,
            relief: map_relief(&self.relief, f)?,
        })
    }
}
impl<P> SheetFoldDef<P> {
    pub const TYPE: &'static str = "sheet_fold";
    pub const BASE_NAME: &'static str = "SheetFold";
    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetFoldDef<Q>, E> {
        Ok(SheetFoldDef {
            body: self.body,
            panel: self.panel,
            line: self.line.clone(),
            stationary_side: self.stationary_side,
            angle: f("angle", &self.angle)?,
            position: self.position,
            radius: self.radius.as_ref().map(|p| f("radius", p)).transpose()?,
            relief: map_relief(&self.relief, f)?,
        })
    }
}
impl<P> SheetCornerRoundDef<P> {
    pub const TYPE: &'static str = "sheet_corner_round";
    pub const BASE_NAME: &'static str = "SheetCornerRound";
    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetCornerRoundDef<Q>, E> {
        Ok(SheetCornerRoundDef {
            body: self.body,
            corners: self.corners.clone(),
            radius: f("radius", &self.radius)?,
            corner_slots: self.corner_slots.clone(),
        })
    }
}
impl<P> SheetCornerChamferDef<P> {
    pub const TYPE: &'static str = "sheet_corner_chamfer";
    pub const BASE_NAME: &'static str = "SheetCornerChamfer";
    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetCornerChamferDef<Q>, E> {
        Ok(SheetCornerChamferDef {
            body: self.body,
            corners: self.corners.clone(),
            distances: CornerDistances {
                first: f("distances.first", &self.distances.first)?,
                second: f("distances.second", &self.distances.second)?,
            },
            corner_slots: self.corner_slots.clone(),
        })
    }
}

fn curve_ref(refs: &mut References, c: &SheetCurveId) {
    refs.features.insert(match c {
        SheetCurveId::Sketch { sketch, .. } => *sketch,
        SheetCurveId::Generated { feature, .. } => *feature,
    });
}
fn boundary_ref(refs: &mut References, b: &BoundaryRef) {
    refs.features.insert(b.panel.feature);
    curve_ref(refs, &b.curve);
}
fn corner_refs(body: BodyUid, corners: &[SheetCorner]) -> References {
    let mut refs = References::default();
    refs.body(body);
    for c in corners {
        refs.features.insert(c.panel.feature);
        curve_ref(&mut refs, &c.incoming);
        curve_ref(&mut refs, &c.outgoing);
    }
    refs
}
fn checked(ctx: &CheckContext<'_>, body: BodyUid, refs: References) -> Result<(), String> {
    ctx.body(body)?;
    for f in refs.features {
        ctx.feature(f)?;
    }
    Ok(())
}

pub(crate) fn settle(def: &mut FeatureDef, old: Option<&FeatureDef>) {
    let (corners, slots, previous) = match def {
        FeatureDef::SheetCornerRound(d) => (
            &d.corners,
            &mut d.corner_slots,
            match old {
                Some(FeatureDef::SheetCornerRound(o)) => Some(&o.corner_slots),
                _ => None,
            },
        ),
        FeatureDef::SheetCornerChamfer(d) => (
            &d.corners,
            &mut d.corner_slots,
            match old {
                Some(FeatureDef::SheetCornerChamfer(o)) => Some(&o.corner_slots),
                _ => None,
            },
        ),
        _ => return,
    };
    if let Some(previous) = previous {
        *slots = previous.clone();
    }
    let mut missing: Vec<_> = corners
        .iter()
        .filter(|c| !slots.iter().any(|s| &s.corner == *c))
        .cloned()
        .collect();
    missing.sort_by_key(|c| serde_json::to_string(c).expect("corner reference"));
    missing.dedup();
    let mut next = slots
        .iter()
        .map(|s| s.slot)
        .max()
        .map_or(0, |v| v.saturating_add(1));
    for corner in missing {
        slots.push(CornerSlot { corner, slot: next });
        next = next.saturating_add(1);
    }
}

fn check_corners(corners: &[SheetCorner], slots: &[CornerSlot]) -> Result<(), String> {
    if corners.is_empty() {
        return Err("no sheet corners selected".into());
    }
    for (i, c) in corners.iter().enumerate() {
        if corners[..i].contains(c) {
            return Err("sheet corner is selected more than once".into());
        }
    }
    for (i, s) in slots.iter().enumerate() {
        if s.slot > u32::MAX / 4
            || slots[..i]
                .iter()
                .any(|p| p.slot == s.slot || p.corner == s.corner)
        {
            return Err("sheet corner allocation contains duplicate or invalid identities".into());
        }
    }
    Ok(())
}

impl FeatureInfo for SheetFlangeDef {
    fn references(&self) -> References {
        let mut r = References::default();
        r.body(self.body);
        boundary_ref(&mut r, &self.boundary);
        r
    }
    fn check(&self, c: &CheckContext<'_>) -> Result<(), String> {
        checked(c, self.body, self.references())
    }
    fn creates_bodies(&self) -> bool {
        true
    }
}
impl FeatureInfo for SheetBendDef {
    fn references(&self) -> References {
        let mut r = References::default();
        r.body(self.body);
        r.body(self.moving_body.unwrap_or(self.body));
        boundary_ref(&mut r, &self.stationary);
        boundary_ref(&mut r, &self.moving);
        r
    }
    fn check(&self, c: &CheckContext<'_>) -> Result<(), String> {
        c.body(self.moving_body.unwrap_or(self.body))?;
        checked(c, self.body, self.references())
    }
    fn creates_bodies(&self) -> bool {
        true
    }
}
impl FeatureInfo for SheetFoldDef {
    fn references(&self) -> References {
        let mut r = References::default();
        r.body(self.body);
        r.features.extend([self.panel.feature, self.line.sketch]);
        r
    }
    fn check(&self, c: &CheckContext<'_>) -> Result<(), String> {
        c.sketch(self.line.sketch)?;
        checked(c, self.body, self.references())
    }
    fn creates_bodies(&self) -> bool {
        true
    }
}
impl FeatureInfo for SheetCornerRoundDef {
    fn references(&self) -> References {
        corner_refs(self.body, &self.corners)
    }
    fn check(&self, c: &CheckContext<'_>) -> Result<(), String> {
        check_corners(&self.corners, &self.corner_slots)?;
        checked(c, self.body, self.references())
    }
    fn creates_bodies(&self) -> bool {
        true
    }
}
impl FeatureInfo for SheetCornerChamferDef {
    fn references(&self) -> References {
        corner_refs(self.body, &self.corners)
    }
    fn check(&self, c: &CheckContext<'_>) -> Result<(), String> {
        check_corners(&self.corners, &self.corner_slots)?;
        checked(c, self.body, self.references())
    }
    fn creates_bodies(&self) -> bool {
        true
    }
}

impl<K: Kernel> Evaluate<K> for SheetFlangeDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let d = self.map_params(&mut |_, p| ctx.param(*p))?;
        crate::sheet::folded::flange(&d, ctx)
    }
}
impl<K: Kernel> Evaluate<K> for SheetBendDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let d = self.map_params(&mut |_, p| ctx.param(*p))?;
        crate::sheet::folded::bend(&d, ctx)
    }
}
impl<K: Kernel> Evaluate<K> for SheetFoldDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let d = self.map_params(&mut |_, p| ctx.param(*p))?;
        crate::sheet::folded::fold(&d, ctx)
    }
}
impl<K: Kernel> Evaluate<K> for SheetCornerRoundDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let d = self.map_params(&mut |_, p| ctx.param(*p))?;
        crate::sheet::folded::corner_round(&d, ctx)
    }
}
impl<K: Kernel> Evaluate<K> for SheetCornerChamferDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let d = self.map_params(&mut |_, p| ctx.param(*p))?;
        crate::sheet::folded::corner_chamfer(&d, ctx)
    }
}
