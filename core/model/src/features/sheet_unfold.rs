// SPDX-License-Identifier: MIT
//! Chosen-bend Unfold and Refold timeline features (mitcad#208). They change
//! the actual evaluated sheet between folded and developed configurations
//! while keeping every panel, bend and curve identity.

use super::{
    CheckContext, EvalContext, Evaluate, FeatureDef, FeatureInfo, FeatureOutput, References,
};
use crate::ids::{BodyUid, FeatureUid};
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::sheet::{BendId, PanelId};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetUnfoldDef<P = ParamId> {
    pub body: BodyUid,
    /// The panel held fixed while the selected bends are developed.
    pub stationary: PanelId,
    /// An explicit nonempty set; unselected bends keep their folded shape.
    pub bends: Vec<BendId>,
    #[serde(skip)]
    pub marker: PhantomData<P>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetRefoldDef<P = ParamId> {
    pub body: BodyUid,
    /// The earlier `sheet_unfold` whose open bends are closed again.
    pub unfold: FeatureUid,
    /// The subset to close; empty closes every bend still open by `unfold`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bends: Vec<BendId>,
    #[serde(skip)]
    pub marker: PhantomData<P>,
}

impl<P> SheetUnfoldDef<P> {
    pub const TYPE: &'static str = "sheet_unfold";
    pub const BASE_NAME: &'static str = "SheetUnfold";
    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetUnfoldDef<Q>, E> {
        Ok(SheetUnfoldDef {
            body: self.body,
            stationary: self.stationary,
            bends: self.bends.clone(),
            marker: PhantomData,
        })
    }
}

impl<P> SheetRefoldDef<P> {
    pub const TYPE: &'static str = "sheet_refold";
    pub const BASE_NAME: &'static str = "SheetRefold";
    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetRefoldDef<Q>, E> {
        Ok(SheetRefoldDef {
            body: self.body,
            unfold: self.unfold,
            bends: self.bends.clone(),
            marker: PhantomData,
        })
    }
}

fn distinct(bends: &[BendId]) -> Result<(), String> {
    for (i, bend) in bends.iter().enumerate() {
        if bends[..i].contains(bend) {
            return Err(format!(
                "bend {} slot {} is selected more than once",
                bend.feature, bend.slot
            ));
        }
    }
    Ok(())
}

impl FeatureInfo for SheetUnfoldDef {
    fn references(&self) -> References {
        let mut r = References::default();
        r.body(self.body);
        r.features.insert(self.stationary.feature);
        r.features.extend(self.bends.iter().map(|b| b.feature));
        r
    }
    fn check(&self, c: &CheckContext<'_>) -> Result<(), String> {
        if self.bends.is_empty() {
            return Err("sheet Unfold needs an explicit nonempty set of bends".into());
        }
        distinct(&self.bends)?;
        c.body(self.body)?;
        for f in self.references().features {
            c.feature(f)?;
        }
        Ok(())
    }
    fn creates_bodies(&self) -> bool {
        true
    }
}

impl FeatureInfo for SheetRefoldDef {
    fn references(&self) -> References {
        let mut r = References::default();
        r.body(self.body);
        r.features.insert(self.unfold);
        r.features.extend(self.bends.iter().map(|b| b.feature));
        r
    }
    fn check(&self, c: &CheckContext<'_>) -> Result<(), String> {
        distinct(&self.bends)?;
        c.body(self.body)?;
        for f in self.references().features {
            c.feature(f)?;
        }
        match &c.feature(self.unfold)?.def {
            FeatureDef::SheetUnfold(unfold) if unfold.body == self.body => Ok(()),
            FeatureDef::SheetUnfold(_) => Err(format!(
                "sheet Unfold {} does not develop body {}",
                self.unfold, self.body
            )),
            _ => Err(format!("{} is not a sheet Unfold", self.unfold)),
        }
    }
    fn creates_bodies(&self) -> bool {
        true
    }
}

impl<K: Kernel> Evaluate<K> for SheetUnfoldDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        crate::sheet::folded::unfold(self, ctx)
    }
}

impl<K: Kernel> Evaluate<K> for SheetRefoldDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        crate::sheet::folded::refold(self, ctx)
    }
}
