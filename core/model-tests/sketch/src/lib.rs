// SPDX-License-Identifier: MIT
//! Tests of the Mitcad model through its JSON api (mitcad#777): The general
//! sketch: entities, constraints, dimensions, patterns, offsets, text, the
//! sketch's axes, inactive constraints, one solve per edit, and copy and paste.
//!
//! A test crate of its own, so that a test added here recompiles only
//! this crate (core/model-tests/README.md).
#![cfg(test)]

mod sketch_axes_tests;
mod sketch_clipboard_tests;
mod sketch_closed_spline_783_tests;
mod sketch_inactive_tests;
mod sketch_p34_tests;
mod sketch_points_tests;
mod sketch_solve_once_tests;
mod sketch_tests;
mod sketch_text_473_tests;
// Redefine Sketch Plane (mitcad#787).
mod sketch_plane_tests;
