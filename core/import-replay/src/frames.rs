// SPDX-License-Identifier: MIT
//! The faces and edges that can give a joint's side its frame, kept per
//! body in the field `frame_entities` of [`Importer`](crate::Importer)
//! for the joints family (`mitcad-import-joints`, mitcad#87); here so that
//! the core's `Importer` can hold them (mitcad#785).

use std::collections::HashMap;

use mitcad_model::datum::{CurveGeometry, SurfaceGeometry};
use mitcad_model::{BodyUid, EdgeName, FaceName};

/// A face or edge that can give a joint's side its frame: a circular
/// edge or a face of the kinds `frame_entities` looks for.
#[derive(Clone)]
pub enum FrameEntity {
    Edge(EdgeName, CurveGeometry),
    Face(FaceName, SurfaceGeometry),
}

/// The [`FrameEntity`]s of each body with the body's version: joints on
/// the same large part find them once (mitcad#87).
pub type FrameEntities = HashMap<BodyUid, (u128, std::sync::Arc<Vec<FrameEntity>>)>;
