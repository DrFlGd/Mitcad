// SPDX-License-Identifier: MIT
//! The joints family of the `.f3d` dump replay (mitcad#55, mitcad#155):
//! the file's joints, as-built joints, joint origins, captured positions,
//! rigid groups, ground items and assembly relationships as Mitcad's.
//!
//! A crate above the replay's core (`mitcad-import-replay`, mitcad#785):
//! its methods are extension traits on the core's `Importer`
//! ([`Joints`], [`Relationships`]), and the core's timeline walk reaches
//! them only through its `Families` trait, which `mitcad-import` fills, so
//! that an edit here does not rebuild the core.

// Joints, as-built joints, joint origins and ground items (mitcad#55).
mod joints;
// Assembly relationships as constraint sets (mitcad#155).
mod relationships;

pub use joints::Joints;
pub use relationships::Relationships;
