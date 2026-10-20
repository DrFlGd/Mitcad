// SPDX-License-Identifier: MIT
//! Emboss: profiles wrapped onto cylinders
//! (`geometry/include/mitcad/geometry/emboss.hpp`, mitcad#458).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// The cylinder of `axis_origin`, `axis_direction` (a unit vector) and
    /// `radius` the profiles wrap onto, and the radius they grow to.
    struct EmbossWrap {
        axis_origin: [f64; 3],
        axis_direction: [f64; 3],
        radius: f64,
        to_radius: f64,
    }

    unsafe extern "C++" {
        include!("bridge/emboss.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type Frame = crate::kernel::profile::ffi::Frame;
        type Region = crate::kernel::profile::ffi::Region;

        /// The regions wrapped onto the cylinder and grown radially, faces
        /// named after `feature`.
        fn wrapped_emboss(
            feature: &str,
            frame: &Frame,
            regions: &[Region],
            wrap: &EmbossWrap,
        ) -> Result<SharedPtr<Shape>>;
    }
}
