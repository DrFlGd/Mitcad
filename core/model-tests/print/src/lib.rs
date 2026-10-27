// SPDX-License-Identifier: MIT
//! Tests of the Mitcad model through its JSON api (mitcad#777): 3D
//! printing: slicers' print settings files, purge volumes, printed fits,
//! the print library and profiles, Print Analysis and the print bed face.
//!
//! A test crate of its own, so that a test added here recompiles only
//! this crate (core/model-tests/README.md).
#![cfg(test)]

mod print_bed_tests;
mod print_files_corpus;
mod print_files_tests;
mod print_fits_tests;
mod print_library_tests;
mod print_profile_tests;
mod print_tests;
mod purge_tests;
