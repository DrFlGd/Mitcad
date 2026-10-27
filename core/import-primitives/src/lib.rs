// SPDX-License-Identifier: MIT
//! The primitives family of the `.f3d` dump replay (mitcad#25): boxes,
//! cylinders, spheres, tori and coils placed on a plane as Mitcad
//! features (`primitives`).
//!
//! A crate above the replay's core (`mitcad-import-replay`, mitcad#785):
//! its methods are the extension trait [`Primitives`] on the core's
//! `Importer`, and the core reaches them only through its `Families`
//! trait, which `mitcad-import` fills, so that an edit here does not
//! rebuild the core.

// Box, cylinder, sphere, torus and coil primitives (mitcad#25).
mod primitives;

pub use primitives::Primitives;
