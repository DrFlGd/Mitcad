// SPDX-License-Identifier: MIT
//! Items other Mitcad crates use; not part of this crate's API.
//!
//! The model reads old design files' sketches, settles a sketch's
//! references to the design and stores sketch points and curves in its
//! features with these helpers of the sketch (mitcad#780); the JSON api
//! reads its commands' points and curves with them. They are re-exported
//! here (mitcad#811), so that the crate root and its documented modules
//! show the sketch's API alone.

pub use crate::edit::settle_references;
pub use crate::legacy::settle;
pub use crate::project::same_curve;
pub use crate::{curve_opt_serde, curves_serde, point_serde};
