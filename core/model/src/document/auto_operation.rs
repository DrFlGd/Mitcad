// SPDX-License-Identifier: MIT
//! The operation a new Extrude takes until the user picks one (mitcad#289):
//! Join when its tool touches or overlaps a visible body of the feature's
//! component, Cut when it goes into one, New Body when it touches none.
//! The app asks for it with the preview (`auto_operation`, commands.md,
//! "Extrude's automatic operation") and keeps the user's own choice.

use super::{Document, ModelError, PreviewReport};
use crate::features::extrude::Operation;
use crate::features::{FeatureDef, ValueInput};
use crate::ids::{BodyUid, FeatureUid};
use crate::kernel::{BoundingBox, Kernel, KernelError, ToolContact};

/// Boxes this far apart still meet (mm): Join's contact distance with
/// room for rounding.
const BOX_SLACK: f64 = 1.0e-5;

/// The operation a new extrusion's tool suggests, and the bodies it works
/// on: those it goes into for Cut, those it touches for Join, none for New
/// Body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoOperation {
    pub operation: Operation,
    pub objects: Vec<BodyUid>,
}

impl AutoOperation {
    fn new_body() -> Self {
        Self {
            operation: Operation::NewBody,
            objects: Vec::new(),
        }
    }
}

impl<K: Kernel> Document<K> {
    /// Previews an added feature as [`Document::preview_add`] does and,
    /// for an extrusion, chooses its operation from where its tool lies:
    /// the extrusion as a new body is evaluated first (the tool does not
    /// depend on the operation, and a Cut that reaches nothing fails
    /// without one), then `def` as it is, whose report is returned. None
    /// for other features and for an extrusion that makes no tool.
    pub fn preview_add_auto(
        &mut self,
        def: &FeatureDef<ValueInput>,
        name: Option<&str>,
    ) -> Result<(PreviewReport, Option<AutoOperation>), ModelError> {
        let FeatureDef::Extrude(extrude) = def else {
            return Ok((self.preview_add(def, name)?, None));
        };
        let mut plain = extrude.clone();
        plain.operation = Operation::NewBody;
        plain.participants.clear();
        plain.cut_scope = None;
        let plain = FeatureDef::Extrude(plain);
        let report = self.preview_add(&plain, name)?;
        let choice = self.tool_operation(report.uid);
        if plain == *def {
            return Ok((report, choice));
        }
        Ok((self.preview_add(def, name)?, choice))
    }

    /// The operation the previewed feature's tool suggests against the
    /// visible bodies of its component at the marker (what a feature added
    /// there starts from).
    fn tool_operation(&self, uid: FeatureUid) -> Option<AutoOperation> {
        let tool = self.preview_tool()?;
        let component = self.preview.as_ref()?.state.entry(uid)?.component;
        let bodies: Vec<(BodyUid, &K::Shape)> = self
            .result
            .bodies
            .iter()
            .filter(|(body, _)| self.result.owners.get(*body) == Some(&component))
            .filter(|(body, _)| {
                self.state
                    .body_attributes
                    .get(*body)
                    .is_none_or(|a| a.visible)
            })
            .map(|(body, shape)| (*body, &shape.value))
            .collect();
        // A kernel that cannot tell suggests nothing: the default stays.
        choose_operation(&self.kernel, tool, &bodies).ok()
    }
}

/// Whether two boxes meet, `slack` apart at most.
fn boxes_meet(a: &BoundingBox, b: &BoundingBox, slack: f64) -> bool {
    (0..3).all(|i| a.min[i] <= b.max[i] + slack && b.min[i] <= a.max[i] + slack)
}

/// The operation for a tool against bodies: bounding boxes first, so that
/// only the bodies the tool's box reaches get the kernel's contact test.
fn choose_operation<K: Kernel>(
    kernel: &K,
    tool: &K::Shape,
    bodies: &[(BodyUid, &K::Shape)],
) -> Result<AutoOperation, KernelError> {
    let tool_box = match kernel.enclosing_box(tool) {
        // An empty tool touches nothing.
        Ok(None) => return Ok(AutoOperation::new_body()),
        Ok(Some(tool_box)) => Some(tool_box),
        // A kernel without boxes tests every body.
        Err(_) => None,
    };
    let mut near = Vec::new();
    for (uid, shape) in bodies {
        let meets = match (&tool_box, kernel.enclosing_box(shape)) {
            (_, Ok(None)) => false,
            (Some(tool_box), Ok(Some(body_box))) => boxes_meet(tool_box, &body_box, BOX_SLACK),
            _ => true,
        };
        if meets {
            near.push((*uid, *shape));
        }
    }
    if near.is_empty() {
        return Ok(AutoOperation::new_body());
    }
    let shapes: Vec<&K::Shape> = near.iter().map(|(_, shape)| *shape).collect();
    let contacts = kernel.tool_contacts(tool, &shapes)?;
    let with = |wanted: ToolContact| -> Vec<BodyUid> {
        near.iter()
            .zip(&contacts)
            .filter(|(_, contact)| **contact == wanted)
            .map(|((uid, _), _)| *uid)
            .collect()
    };
    let inside = with(ToolContact::Inside);
    if !inside.is_empty() {
        return Ok(AutoOperation {
            operation: Operation::Cut,
            objects: inside,
        });
    }
    let touched = with(ToolContact::Touches);
    if !touched.is_empty() {
        return Ok(AutoOperation {
            operation: Operation::Join,
            objects: touched,
        });
    }
    Ok(AutoOperation::new_body())
}
