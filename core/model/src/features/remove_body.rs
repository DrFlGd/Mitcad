// SPDX-License-Identifier: MIT
//! Remove body: takes bodies out of the design. The features before it keep
//! their results; features after it cannot refer to the removed bodies. It
//! is what leaves a split's unwanted piece, or a tool body that is not
//! wanted any more, out of the design without hiding it.

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::moves::check_bodies;
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;

/// Has no values; `P` only matches the other feature types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveBodyDef<P = ParamId> {
    pub bodies: Vec<BodyUid>,
    #[serde(skip)]
    pub values: PhantomData<P>,
}

impl<P> RemoveBodyDef<P> {
    pub const TYPE: &'static str = "remove_body";
    pub const BASE_NAME: &'static str = "RemoveBody";

    pub fn new(bodies: Vec<BodyUid>) -> Self {
        Self {
            bodies,
            values: PhantomData,
        }
    }

    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<RemoveBodyDef<Q>, E> {
        Ok(RemoveBodyDef::new(self.bodies.clone()))
    }
}

impl FeatureInfo for RemoveBodyDef {
    fn references(&self) -> References {
        let mut references = References::default();
        for body in &self.bodies {
            references.body(*body);
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        check_bodies(ctx, &self.bodies)
    }
}

impl<K: Kernel> Evaluate<K> for RemoveBodyDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let mut changes = Vec::with_capacity(self.bodies.len());
        for body in &self.bodies {
            // Read it, so the cache key holds the body it removes.
            match ctx.body(*body) {
                Ok(_) => changes.push(BodyChange::Remove(*body)),
                Err(error) => match ctx.gone_piece(*body) {
                    // A piece its split no longer makes (mitcad#408).
                    Some(gone) => ctx.warn(format!("{gone}: nothing to remove")),
                    None => return Err(error),
                },
            }
        }
        Ok(FeatureOutput {
            changes,
            ..FeatureOutput::default()
        })
    }
}
