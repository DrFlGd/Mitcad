// SPDX-License-Identifier: MIT
//! Tests of the Mitcad model through its JSON api (mitcad#777): Drawings:
//! the sheet, stored drawings, templates and revisions, tables and the .dwg
//! export.
//!
//! A test crate of its own, so that a test added here recompiles only
//! this crate (core/model-tests/README.md).
#![cfg(test)]

mod drawing_tests;
mod dwg_tests;
mod stored_tests;
mod table_tests;
mod template_tests;
