// SPDX-License-Identifier: MIT
//! Emboss: profiles wrapped onto cylinders and cones
//! (`geometry/include/mitcad/geometry/emboss.hpp`, mitcad#458, mitcad#473).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// The cylinder of `axis_origin`, `axis_direction` (a unit vector) and
    /// `radius` the profiles wrap onto, and the radius they grow to; a cone
    /// with `half_angle` (see `EmbossWrapSpec`).
    struct EmbossWrap {
        axis_origin: [f64; 3],
        axis_direction: [f64; 3],
        radius: f64,
        to_radius: f64,
        half_angle: f64,
    }

    /// A conical face: its apex, its axis where the radius grows, its half
    /// angle, and whether the material lies outside it.
    struct ConeOutput {
        apex: [f64; 3],
        axis: [f64; 3],
        half_angle: f64,
        internal: bool,
    }

    unsafe extern "C++" {
        include!("bridge/emboss.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type Frame = crate::kernel::profile::ffi::Frame;
        type Region = crate::kernel::profile::ffi::Region;

        /// The regions wrapped onto the cylinder or the cone and grown
        /// along its normals, faces named after `feature`.
        fn wrapped_emboss(
            feature: &str,
            frame: &Frame,
            regions: &[Region],
            wrap: &EmbossWrap,
        ) -> Result<SharedPtr<Shape>>;

        /// A conical face of a body (all pieces of a split one).
        fn face_cone(shape: &Shape, face: &str) -> Result<ConeOutput>;
    }
}
