// SPDX-License-Identifier: MIT
//! Gears: toothed profiles swept along their axis or ruled to an apex
//! (`geometry/include/mitcad/geometry/gear.hpp`, mitcad#472).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// A stretch of a gear sweep (`geometry::GearStep`).
    #[derive(Clone, Copy, Debug)]
    struct GearStep {
        length: f64,
        twist: f64,
        shift: f64,
    }

    unsafe extern "C++" {
        include!("bridge/gear.hpp");
        include!("bridge/trycatch.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type Frame = crate::kernel::profile::ffi::Frame;
        type Region = crate::kernel::profile::ffi::Region;
        type Loop = crate::kernel::profile::ffi::Loop;

        /// geometry::gear_sweep: the region's loops turned by `turn` and
        /// moved `start` along the frame's normal, swept by the steps.
        #[allow(clippy::too_many_arguments)]
        fn gear_sweep(
            feature: &str,
            frame: &Frame,
            region: &Region,
            turn: f64,
            start: f64,
            steps: &[GearStep],
            start_name: &str,
            end_name: &str,
        ) -> Result<SharedPtr<Shape>>;

        /// geometry::gear_cone: the heel section ruled to its copy scaled
        /// about the apex, within the blank turned about the axis.
        fn gear_cone(
            feature: &str,
            frame: &Frame,
            region: &Region,
            apex: f64,
            scale: f64,
            blank: &Loop,
        ) -> Result<SharedPtr<Shape>>;
    }
}

use mitcad_model::features::gear_profile::NamedSegment;

use super::profile;

/// Named segments as a loop of the profile bridge.
pub fn loop_of(segments: &[NamedSegment]) -> profile::ffi::Loop {
    profile::ffi::Loop {
        segments: segments
            .iter()
            .map(|s| profile::segment(s.name.clone(), &s.geometry))
            .collect(),
    }
}

/// Loops as one region (the first the outer boundary).
pub fn region_of(loops: &[Vec<NamedSegment>]) -> profile::ffi::Region {
    profile::ffi::Region {
        name: "gear".to_owned(),
        loops: loops.iter().map(|l| loop_of(l)).collect(),
    }
}
