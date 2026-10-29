// SPDX-License-Identifier: MIT
//! The face names of the BUILD tab's derived parts
//! (`geometry/include/mitcad/geometry/build.hpp`, mitcad#375).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    unsafe extern "C++" {
        include!("bridge/build.hpp");
        include!("bridge/trycatch.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;

        /// The shape with `from[i]` renamed `to[i]` and every other face
        /// name n `<prefix>(n)` (n itself for an empty prefix).
        fn rename_faces(
            shape: &Shape,
            prefix: &str,
            from: &[String],
            to: &[String],
        ) -> Result<SharedPtr<Shape>>;
    }
}
