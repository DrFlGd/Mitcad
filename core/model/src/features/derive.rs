// SPDX-License-Identifier: MIT
//! Derive (mitcad#324): copies of bodies of other components as new bodies
//! of the feature's component, linked to them. Each body is read in its own
//! component at the derive's point of the timeline and moved into the
//! derive's coordinates by the placements there (`links`, one
//! [`OccurrenceLink`] per body of another component, as a combine's
//! `tool_links`), so the copies follow when the source bodies change or
//! their occurrences move. The source bodies stay where they are. The
//! copies are bodies `<derive>.b<j>` in the order listed, their faces
//! named `<derive>:inst1(<name>)`.
//!
//! The `.f3d` import brings a design's derived bodies in so: the documents
//! they come from are copied into the design as hidden components, each
//! placed where the derive put its bodies, and the derive copies their
//! bodies from there.

use std::collections::BTreeMap;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::mapping::map_params;
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::links::OccurrenceLink;
use crate::parameters::ParamId;
use crate::transform::{Instance, Transform};

/// Has no values; `P` only matches the other feature types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeriveDef<P = ParamId> {
    /// The bodies copied, in order.
    pub bodies: Vec<BodyUid>,
    /// Where each body of another component and the derive's component are
    /// seen; a body without a link is one of the derive's own component.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub links: BTreeMap<BodyUid, OccurrenceLink>,
    #[serde(skip)]
    pub values: PhantomData<P>,
}

impl<P> DeriveDef<P> {
    pub const TYPE: &'static str = "derive";
    pub const BASE_NAME: &'static str = "Derive";

    pub fn new(bodies: Vec<BodyUid>, links: BTreeMap<BodyUid, OccurrenceLink>) -> Self {
        Self {
            bodies,
            links,
            values: PhantomData,
        }
    }

    map_params! {
        DeriveDef {
            bodies: clone,
            links: clone,
            values: phantom,
        }
    }
}

impl FeatureInfo for DeriveDef {
    fn references(&self) -> References {
        let mut references = References::default();
        for body in &self.bodies {
            references.body(*body);
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        if self.bodies.is_empty() {
            return Err("no bodies selected".to_owned());
        }
        for (i, body) in self.bodies.iter().enumerate() {
            if self.bodies[..i].contains(body) {
                return Err(format!("body {body} is listed more than once"));
            }
            ctx.body(*body)?;
        }
        if let Some(other) = self.links.keys().find(|b| !self.bodies.contains(b)) {
            return Err(format!("body {other} is linked but not derived"));
        }
        Ok(())
    }

    fn creates_bodies(&self) -> bool {
        true
    }
}

impl<K: Kernel> Evaluate<K> for DeriveDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let mut changes = Vec::with_capacity(self.bodies.len());
        for (j, body) in self.bodies.iter().enumerate() {
            let (shape, transform) = match self.links.get(body) {
                Some(link) => {
                    let (component, transform) =
                        ctx.linked(link).map_err(|e| format!("body {body}: {e}"))?;
                    (ctx.body_in(component, *body)?, transform)
                }
                None => (ctx.body(*body)?, Transform::IDENTITY),
            };
            let instance = Instance {
                feature: ctx.uid,
                index: 1,
            };
            let copy = ctx
                .kernel
                .transform_shape(&shape, &transform, Some(instance))
                .map_err(|e| format!("body {body}: {e}"))?;
            changes.push(BodyChange::Set(BodyUid::new(ctx.uid, j as u32), copy));
        }
        Ok(FeatureOutput {
            changes,
            ..FeatureOutput::default()
        })
    }
}
