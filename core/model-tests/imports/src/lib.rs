// SPDX-License-Identifier: MIT
//! Tests of the imports that need the whole model and its JSON api
//! (mitcad#831): a design replayed into a document and checked through the
//! api's queries and commands.
//!
//! The import crates' own tests check what their readers and imports
//! produce without the api (`mitcad-model-api`), so that a change in the
//! api, or in the model for the readers that do not depend on it, rebuilds
//! this one crate instead of each import's tests (core/model-tests/README.md).
#![cfg(test)]

mod ifc_tests;
mod ipt_design_tests;
mod solidcraft_tests;
