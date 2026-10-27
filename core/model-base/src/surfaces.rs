// SPDX-License-Identifier: MIT
//! Patch, Stitch and Thicken (mitcad#456): what the kernel is given to fill
//! a loop of edges with a surface body and what it says of a body's open
//! boundaries. The features are `features/patch.rs`, `features/stitch.rs`
//! and `features/thicken.rs`; the kernel's methods are grouped under
//! "Patch, Stitch and Thicken" in [`crate::kernel::Kernel`].

use serde::{Deserialize, Serialize};

use crate::datum::Vec3;
use crate::ids::FeatureUid;
use crate::topo::{EdgeName, FaceName};

/// How a patch meets the face beside each edge of the loop it fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchContinuity {
    /// Through the edge only.
    #[default]
    G0,
    /// Tangent to the face beside the edge.
    G1,
    /// Tangent, and curved as the face beside the edge.
    G2,
}

impl PatchContinuity {
    pub fn is_g0(&self) -> bool {
        *self == Self::G0
    }

    /// `g0`, `g1` or `g2`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::G0 => "g0",
            Self::G1 => "g1",
            Self::G2 => "g2",
        }
    }
}

/// The closed loop a patch fills ([`PatchLoopSpec`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatchBoundary<'a> {
    /// Named edges of the body: every edge each name resolves to.
    Edges(&'a [EdgeName]),
    /// The body's open boundary with this index in the list
    /// [`crate::kernel::Kernel::open_boundaries`] gives: free edges, which
    /// have no names of their own.
    Opening(usize),
}

/// Input of [`crate::kernel::Kernel::patch_loop`]: a surface body of one
/// face filling a closed loop of the body's edges, named after `feature`.
#[derive(Debug, Clone, Copy)]
pub struct PatchLoopSpec<'a, S> {
    pub feature: FeatureUid,
    pub body: &'a S,
    pub boundary: PatchBoundary<'a>,
    /// How the patch meets the face beside each edge of the loop.
    pub continuity: PatchContinuity,
    /// Edges of the loop that meet their face otherwise, by name.
    pub edge_continuity: &'a [(EdgeName, PatchContinuity)],
    /// Points the patch passes through.
    pub points: &'a [Vec3],
}

/// A loop of free edges of a body: the edges of one face only, an open
/// boundary of a surface body ([`crate::kernel::Kernel::open_boundaries`]).
#[derive(Debug, Clone, PartialEq)]
pub struct OpenBoundary {
    /// The names of the faces its edges bound, sorted, each once; unnamed
    /// faces are left out.
    pub faces: Vec<FaceName>,
    /// Points along its edges, their ends included, no more than about
    /// 0.5 mm apart (an importer finds the loop a point was picked on).
    pub points: Vec<Vec3>,
    /// Whether its edges close up into one loop.
    pub closed: bool,
}
