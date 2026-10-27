// SPDX-License-Identifier: MIT
//! The sheet-metal family of the `.f3d` dump replay (mitcad#211): the
//! decoded sheet items as Mitcad's native sheet features, and roundings and
//! chamfers tried as sheet corners (`sheet`).
//!
//! A crate above the replay's core (`mitcad-import-replay`, mitcad#785):
//! its methods are the extension trait [`Sheets`] on the core's
//! `Importer`, and the core reaches them only through its `Families`
//! trait, which `mitcad-import` fills, so that an edit here does not
//! rebuild the core.

// Sheet-metal items as native sheet features (mitcad#211).
mod sheet;

pub use sheet::Sheets;
