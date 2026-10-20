// SPDX-License-Identifier: MIT
//! Boundary Fill (mitcad#457): solids of the cells that tool bodies
//! enclose. The tools' faces divide space into cells, each inside some of
//! the tools and outside the others; a point inside a cell picks it, all of
//! it (several solids where that space falls apart). The cells become new
//! bodies, one per cell in the order of the points, or join, cut or
//! intersect the participant bodies as one tool, as an extrusion's tool
//! does (`bodies::apply_tool`: without participants, all bodies of the
//! component, the tools that stay among them). The tools stay unless
//! removed; removed tools are gone before the operation. Two points in one
//! cell fill it once, with a warning.
//!
//! A cell's faces are pieces of the tools' faces and keep their names
//! (`F2:side(c1[c4,c2])`, pieces of one face numbered `#k`), as a boolean
//! keeps them ([`Kernel::boundary_cells`]); the new bodies are
//! `<feature>.b<i>`. The tools are read, never changed.

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::bodies::{BodySet, apply_tool, boolean_op, verb};
use super::extrude::check_participants;
use super::{
    CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, Operation, References,
    is_false,
};
use crate::datum::Vec3;
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;

/// Has no values; `P` only matches the other feature types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryFillDef<P = ParamId> {
    /// The bodies whose faces bound the cells.
    pub tools: Vec<BodyUid>,
    /// A point inside each cell to fill, in the coordinates of the
    /// feature's component.
    pub cells: Vec<Vec3>,
    pub operation: Operation,
    /// The bodies a join, cut or intersection works on; empty for all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<BodyUid>,
    /// Takes the tools out of the design (before the operation).
    #[serde(default, skip_serializing_if = "is_false")]
    pub remove_tools: bool,
    #[serde(skip)]
    pub values: PhantomData<P>,
}

impl<P> BoundaryFillDef<P> {
    pub const TYPE: &'static str = "boundary_fill";
    pub const BASE_NAME: &'static str = "BoundaryFill";

    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<BoundaryFillDef<Q>, E> {
        Ok(BoundaryFillDef {
            tools: self.tools.clone(),
            cells: self.cells.clone(),
            operation: self.operation,
            participants: self.participants.clone(),
            remove_tools: self.remove_tools,
            values: PhantomData,
        })
    }
}

impl FeatureInfo for BoundaryFillDef {
    fn references(&self) -> References {
        let mut references = References::default();
        for body in self.tools.iter().chain(&self.participants) {
            references.body(*body);
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        if self.tools.is_empty() {
            return Err("no tool bodies selected".to_owned());
        }
        for (i, tool) in self.tools.iter().enumerate() {
            if self.tools[..i].contains(tool) {
                return Err(format!("body {tool} is listed more than once"));
            }
            ctx.body(*tool)?;
        }
        if self.cells.is_empty() {
            return Err("no cells picked: a point inside each cell to fill".to_owned());
        }
        if self.cells.iter().flatten().any(|v| !v.is_finite()) {
            return Err("the points of the cells must be finite".to_owned());
        }
        check_participants(ctx, self.operation, &self.participants)?;
        if self.remove_tools
            && let Some(body) = self.participants.iter().find(|b| self.tools.contains(b))
        {
            return Err(format!(
                "body {body} is a tool the fill removes; it cannot be a participant"
            ));
        }
        Ok(())
    }

    fn creates_bodies(&self) -> bool {
        // The cells' new bodies, and the pieces of bodies a cut splits.
        true
    }

    fn new_component(&self) -> bool {
        self.operation == Operation::NewComponent
    }
}

impl<K: Kernel> Evaluate<K> for BoundaryFillDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        // Mesh bodies are refused (mitcad#399).
        let tools = self
            .tools
            .iter()
            .map(|uid| ctx.surface_body(*uid))
            .collect::<Result<Vec<_>, _>>()?;
        let refs: Vec<&K::Shape> = tools.iter().collect();
        let cells = ctx
            .kernel
            .boundary_cells(&refs, &self.cells)
            .map_err(|e| e.to_string())?;
        if cells.len() != self.cells.len() {
            return Err(format!(
                "the geometry kernel gave {} cells for {} points",
                cells.len(),
                self.cells.len()
            ));
        }
        // A cell is the same cell as another when the same tools hold its
        // point.
        let mut filled: Vec<K::Shape> = Vec::new();
        for (i, cell) in cells.iter().enumerate() {
            if let Some(j) = cells[..i].iter().position(|c| c.holders == cell.holders) {
                ctx.warn(format!(
                    "points {} and {} pick the same cell; it is filled once",
                    j + 1,
                    i + 1
                ));
                continue;
            }
            filled.push(cell.shape.clone());
        }
        let mut bodies = BodySet::new(if self.operation.makes_bodies() {
            self.tools
                .iter()
                .copied()
                .zip(tools.iter().cloned())
                .collect()
        } else {
            ctx.bodies()
        });
        if self.remove_tools {
            for uid in &self.tools {
                bodies.remove(*uid);
            }
        }
        let mut tool = None;
        if self.operation.makes_bodies() {
            for cell in filled {
                let uid = ctx.new_body();
                bodies.set(uid, cell);
            }
        } else {
            let united = if filled.len() == 1 {
                filled.into_iter().next().expect("one cell")
            } else {
                let refs: Vec<&K::Shape> = filled.iter().collect();
                ctx.kernel.unite(&refs).map_err(|e| e.to_string())?
            };
            if !apply_tool(
                ctx,
                &mut bodies,
                self.operation,
                &self.participants,
                &united,
            )? {
                let op = boolean_op(self.operation).expect("a join, cut or intersection");
                return Err(format!(
                    "the filled cells do not {} any participant body",
                    match verb(op) {
                        "join" => "touch",
                        "cut" => "cut into",
                        other => other,
                    }
                ));
            }
            tool = Some(united);
        }
        Ok(FeatureOutput {
            changes: bodies.changes(),
            tool,
            ..FeatureOutput::default()
        })
    }
}
