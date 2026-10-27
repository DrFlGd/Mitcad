// SPDX-License-Identifier: MIT
//! Tests of the Mitcad model through its JSON api (mitcad#777): Commands
//! and queries in general: the document query, the browser, named views and
//! DXF insert, small UI gaps and metric defaults, failed features,
//! inserting at a timeline position, autoselected profiles, faces and edges
//! with geometry, text parameters, parameter limits, the caches of results,
//! cancelling recomputes, the result store and preview placement.
//!
//! A test crate of its own, so that a test added here recompiles only
//! this crate (core/model-tests/README.md).
#![cfg(test)]

mod api_tests;
mod autoselect_tests;
mod browser_tests;
mod cache_tests;
mod face_lists_tests;
mod failed_features_tests;
mod insert_feature_tests;
mod metric_defaults_tests;
mod monitor_tests;
mod parameter_limits_tests;
mod preview_placement_tests;
mod store_tests;
mod text_parameter_tests;
mod ui_gaps_tests;
mod view_file_tests;
