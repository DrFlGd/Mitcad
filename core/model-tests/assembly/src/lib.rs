// SPDX-License-Identifier: MIT
//! Tests of the Mitcad model through its JSON api (mitcad#777): Components
//! and occurrences, reparenting, Move Features into a Component, body
//! transfer and the design policy.
//!
//! A test crate of its own, so that a test added here recompiles only
//! this crate (core/model-tests/README.md).
#![cfg(test)]

mod body_transfer_tests;
mod component_tests;
mod move_features_tests;
mod policy_tests;
mod reparent_tests;
