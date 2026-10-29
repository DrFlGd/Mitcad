// SPDX-License-Identifier: MIT
//! A planar face of a body revolved as a profile
//! (`geometry/include/mitcad/geometry/face_revolve.hpp`, mitcad#539).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// How the face turns (see `FaceRevolveSpec`): about the axis through
    /// `axis_origin` along `axis_direction` (a unit vector), `angle1` the
    /// right hand way and, with `two_sides`, `angle2` the other way
    /// (radians).
    struct FaceRevolution {
        axis_origin: [f64; 3],
        axis_direction: [f64; 3],
        angle1: f64,
        two_sides: bool,
        angle2: f64,
    }

    unsafe extern "C++" {
        include!("bridge/face_revolve.hpp");
        include!("bridge/trycatch.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;

        /// geometry::revolve_face: the face `face` of the body turned, its
        /// faces named after `feature` and its caps keyed by `key`.
        fn revolve_face(
            feature: &str,
            body: &Shape,
            face: &str,
            key: &str,
            revolution: &FaceRevolution,
        ) -> Result<SharedPtr<Shape>>;
    }
}
