// SPDX-License-Identifier: MIT
//! Faces moved by a rigid motion
//! (`geometry/include/mitcad/geometry/move_faces.hpp`, mitcad#343).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    unsafe extern "C++" {
        include!("bridge/move_faces.hpp");
        include!("bridge/trycatch.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type Affine = crate::kernel::transform::ffi::Affine;

        /// geometry::move_faces_rigidly with the motion's matrix.
        fn move_faces(
            feature: &str,
            body: &Shape,
            faces: &[String],
            motion: &Affine,
        ) -> Result<SharedPtr<Shape>>;
    }
}
