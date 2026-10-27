// SPDX-License-Identifier: MIT
//! The static library the application, `mitcad-cli` and the C++ tests link
//! (the CMake target `mitcad_ffi`): the C++ bridges of `mitcad-ffi-bridges`
//! (`core/ffi`) with every Rust library they use.
//!
//! The code lives in that rlib so that Cargo starts compiling it on its
//! dependencies' metadata, beside their code generation; a static library,
//! like a program, waits for the code of all its dependencies (mitcad#792).
//! Linking the rlib here puts its cxx bridges and `#[no_mangle]` functions
//! into this library.

pub use mitcad_ffi_bridges as bridges;
