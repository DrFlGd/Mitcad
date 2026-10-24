// SPDX-License-Identifier: MIT
//! Importer tests on synthetic dumps with the mock kernel.
//!
//! This file only declares the test modules. [`support`] holds what several
//! of them share: the test kernel, the stored-geometry fixture and the
//! builders of the block design and of sketches. Every other module holds
//! the tests of one topic with the helpers only that topic uses. A new test
//! goes into the module of its topic; a helper that two modules need goes
//! into `support` (items `pub(super)`) and into the `use support::{..}`
//! list below, which lets every module reach it through `use super::*`.

#[allow(unused_imports)]
use mitcad_f3d::design::ir::Dump;
#[allow(unused_imports)]
use mitcad_model::testing::MockKernel;
#[allow(unused_imports)]
use mitcad_model::testing::MockShape;
#[allow(unused_imports)]
use mitcad_model::{
    BodyKind, BooleanOp, BooleanOutput, BoundingBox, Chamfer, EdgeName, ExtrudeFeatureSpec,
    ExtrudeSpec, FilletSet, KernelError, MassProperties, Plane,
};
#[allow(unused_imports)]
use mitcad_model::{Document, FeatureDef};
#[allow(unused_imports)]
use serde_json::{Value, json};

#[allow(unused_imports)]
use super::*;

// The shared test kernel, fixtures and builders.
mod support;
#[allow(unused_imports)]
use support::{
    LARGE, SMALL, SlowExtrusions, TestGeometry, TestKernel, add, assembly_dump, assert_parametric,
    at, block_dump, block_history, circle_sketch, decoded_small_rectangle, dump, edge_point,
    entity, extrude, extrusion_of, face_point, fillet_item, gcircle, gfixed, gline, gpoint,
    group_sketch_dump, hash, import, param, point_at, radius, rectangle_sketch, reference, shapes,
    sketch_query, slow_call, slow_call_for, slow_profile_import, thread_item, timeline_dump,
    two_rectangles, without_uids,
};

// The topics.
mod ahead_tests;
mod appearance_tests;
mod body_tests;
mod component_tests;
mod constraint_tests;
mod dressup_tests;
mod fallback_tests;
mod learning_tests;
mod pattern_tests;
mod primitive_tests;
mod profile_tests;
mod replay_tests;
mod sketch_tests;
mod sweep_tests;
mod thread_tests;
mod watchdog_tests;

// Joints, as-built joints, joint origins and ground items (mitcad#55).
mod joint_tests;

// Assembly relationships as constraint sets (mitcad#155).
mod relationship_tests;

// The import's outcomes on the timeline (mitcad#170).
mod timeline_note_tests;

// Items suppressed in the file as suppressed features (mitcad#180).
mod suppressed_tests;

// Components of other documents filled from their documents (mitcad#196).
mod reference_tests;

// Retries on the file's exact state keep the matching bodies (mitcad#283).
mod retry_tests;

// Results that deviate from the file's, kept at the user's choice
// (mitcad#379).
mod deviation_tests;

// Derives: bodies of other documents (mitcad#324).
mod derive_tests;

// Components' part numbers and descriptions as properties (mitcad#496).
mod property_tests;

// History replay into a document that already has bodies (mitcad#624).
mod body_scope_tests;
