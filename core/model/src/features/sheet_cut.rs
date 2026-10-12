// SPDX-License-Identifier: MIT
//! Thickness-normal sheet Cut across bends (mitcad#208, mitcad#209): profiles
//! of one or several sketches on the fixed panel cut every chart developed into
//! its plane; material the cut disconnects becomes several bodies.

use super::extrude::ProfileRef;
use super::{CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::sheet::{BendId, PanelId};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetCutDef<P = ParamId> {
    pub body: BodyUid,
    /// The panel whose developed plane the sketch lies in; it stays fixed.
    pub stationary: PanelId,
    /// Bends developed to interpret the sketch; the input configuration is
    /// restored afterwards. Bends already open stay open.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_bends: Vec<BendId>,
    #[serde(deserialize_with = "super::extrude::profile_set")]
    pub profiles: Vec<ProfileRef>,
    #[serde(skip)]
    pub marker: PhantomData<P>,
}

impl<P> SheetCutDef<P> {
    pub const TYPE: &'static str = "sheet_cut";
    pub const BASE_NAME: &'static str = "SheetCut";
    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SheetCutDef<Q>, E> {
        Ok(SheetCutDef {
            body: self.body,
            stationary: self.stationary,
            open_bends: self.open_bends.clone(),
            profiles: self.profiles.clone(),
            marker: PhantomData,
        })
    }
}

impl FeatureInfo for SheetCutDef {
    fn references(&self) -> References {
        let mut r = References::default();
        r.body(self.body);
        r.features.insert(self.stationary.feature);
        r.features.extend(self.open_bends.iter().map(|b| b.feature));
        r.features.extend(self.profiles.iter().map(|p| p.sketch));
        r
    }
    fn check(&self, c: &CheckContext<'_>) -> Result<(), String> {
        if self.profiles.is_empty() {
            return Err("no profiles selected".into());
        }
        // Several sketches may contribute; each must lie on the fixed panel.
        for profile in &self.profiles {
            c.sketch(profile.sketch)?;
        }
        for (i, bend) in self.open_bends.iter().enumerate() {
            if self.open_bends[..i].contains(bend) {
                return Err(format!(
                    "bend {} slot {} is selected more than once",
                    bend.feature, bend.slot
                ));
            }
        }
        c.body(self.body)?;
        for f in self.references().features {
            c.feature(f)?;
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

impl<K: Kernel> Evaluate<K> for SheetCutDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        crate::sheet::folded::cut(self, ctx)
    }
}
