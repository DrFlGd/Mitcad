// SPDX-License-Identifier: MIT
//! `placeholder`: where an imported file had a timeline item the import
//! did not bring in because it was suppressed or failed in the file
//! (mitcad#170). The importers add it suppressed at the item's place, named
//! after the item, with the file's state as its import note
//! ([`crate::Document::import_note`]); it builds nothing, also when it is
//! unsuppressed (it then warns that it builds nothing).

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::{CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References};
use crate::kernel::Kernel;
use crate::parameters::ParamId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceholderDef<P = ParamId> {
    /// The item's type in the file it was imported from (`ExtrudeFeature`),
    /// empty when not known.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub item_type: String,
    #[serde(skip)]
    pub marker: PhantomData<P>,
}

impl<P> PlaceholderDef<P> {
    pub const TYPE: &'static str = "placeholder";
    pub const BASE_NAME: &'static str = "Placeholder";

    pub fn new(item_type: impl Into<String>) -> Self {
        Self {
            item_type: item_type.into(),
            marker: PhantomData,
        }
    }

    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<PlaceholderDef<Q>, E> {
        Ok(PlaceholderDef::new(self.item_type.clone()))
    }
}

/// The warning of a placeholder that is not suppressed.
pub const BUILDS_NOTHING: &str =
    "builds nothing: it stands for an item of the imported file that did not come in";

impl FeatureInfo for PlaceholderDef {
    fn references(&self) -> References {
        References::default()
    }

    fn check(&self, _ctx: &CheckContext<'_>) -> Result<(), String> {
        Ok(())
    }
}

impl<K: Kernel> Evaluate<K> for PlaceholderDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        ctx.warn(BUILDS_NOTHING);
        Ok(FeatureOutput::default())
    }
}
