// SPDX-License-Identifier: MIT
//! Body Transfer (mitcad#465): bodies of another component move into the
//! feature's component (the destination, an existing component) at the
//! feature's point of the timeline. They keep their ids, names, attributes
//! and topological names; their geometry is converted once from the source
//! component's coordinates into the destination's, so they stay where they
//! are in the design. From then on they are the destination's bodies: the
//! features after it in the destination work on them, those of the source
//! no longer see them.
//!
//! `link` says where both components are seen (as a derive's links:
//! the occurrence path to the source component, `source`, and the one to
//! the destination, `target`), and the conversion is
//! `placement(target)⁻¹ · placement(source)` at the feature's point of the
//! timeline, so the bodies stay in place when the placements before it
//! change. A `transform` given instead is a conversion the feature keeps
//! as it is (the rows of a rigid motion), such as the one a file records
//! for its own transfer (the `.solidcraft` import).

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::links::OccurrenceLink;
use crate::parameters::ParamId;
use crate::transform::Transform;

/// Has no values; `P` only matches the other feature types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyTransferDef<P = ParamId> {
    /// The bodies moved, all of the source component.
    pub bodies: Vec<BodyUid>,
    /// Where the source component (`source`) and the destination, the
    /// feature's own component (`target`), are seen; empty paths are the
    /// root.
    #[serde(default)]
    pub link: OccurrenceLink,
    /// A fixed conversion from the source's coordinates into the
    /// destination's, instead of the placements' (rows of a rigid motion).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_rows"
    )]
    pub transform: Option<Transform>,
    #[serde(skip)]
    pub values: PhantomData<P>,
}

impl<P> BodyTransferDef<P> {
    pub const TYPE: &'static str = "body_transfer";
    pub const BASE_NAME: &'static str = "BodyTransfer";

    pub fn new(bodies: Vec<BodyUid>, link: OccurrenceLink, transform: Option<Transform>) -> Self {
        Self {
            bodies,
            link,
            transform,
            values: PhantomData,
        }
    }

    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<BodyTransferDef<Q>, E> {
        Ok(BodyTransferDef::new(
            self.bodies.clone(),
            self.link.clone(),
            self.transform,
        ))
    }
}

/// An optional transform as rows of numbers.
mod optional_rows {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::transform::Transform;

    pub fn serialize<S: Serializer>(t: &Option<Transform>, s: S) -> Result<S::Ok, S::Error> {
        match t {
            Some(t) => crate::features::occurrence::rows::serialize(t, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Transform>, D::Error> {
        let rows = Option::<Vec<Vec<f64>>>::deserialize(d)?;
        rows.map(|rows| crate::assembly::from_rows(&rows).map_err(serde::de::Error::custom))
            .transpose()
    }
}

impl FeatureInfo for BodyTransferDef {
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
        if let Some(t) = &self.transform {
            check_transform(t)?;
        }
        Ok(())
    }
}

fn check_transform(t: &Transform) -> Result<(), String> {
    if !t.is_finite() || !t.is_rigid() {
        return Err("a body transfer converts by a rotation and a translation only".to_owned());
    }
    Ok(())
}

impl<K: Kernel> Evaluate<K> for BodyTransferDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let assembly = ctx.env.assembly;
        let owners = ctx.env.owners;
        // Where the occurrences lead depends on the assembly's structure,
        // which the placements and bodies read do not show.
        ctx.reads.push(crate::recompute::Read::Assembly(
            crate::features::extrude_scope::assembly_key(&ctx.env),
        ));
        let source = self.link.source_component(assembly)?;
        let destination = ctx.component();
        if source == destination {
            return Err(format!(
                "the bodies are to come from {}, the component of this feature: a body \
                 transfer moves bodies of another component into its own",
                assembly.name(destination)
            ));
        }
        let transform = match &self.transform {
            Some(t) => *t,
            None => ctx.linked(&self.link)?.1,
        };
        check_transform(&transform)?;
        let mut output = FeatureOutput::default();
        for body in &self.bodies {
            let shape = match owners.get(body) {
                Some(owner) if *owner == source => ctx.body_in(source, *body)?,
                Some(owner) => {
                    // Read all the same, so that the cache sees it arrive.
                    let _ = ctx.body_in(source, *body);
                    return Err(format!(
                        "body {body} (from {}) belongs to {}, not to {} where the transfer \
                         takes it from",
                        ctx.feature_name(body.feature),
                        assembly.name(*owner),
                        assembly.name(source)
                    ));
                }
                None => ctx.body_in(source, *body)?,
            };
            let moved = if transform.is_identity() {
                shape
            } else {
                ctx.kernel
                    .transform_shape(&shape, &transform, None)
                    .map_err(|e| format!("body {body}: {e}"))?
            };
            output.changes.push(BodyChange::Set(*body, moved));
            output.owners.insert(*body, destination);
            output
                .sheets
                .insert(*body, crate::sheet::SheetChange::Transform(transform));
        }
        Ok(output)
    }
}
