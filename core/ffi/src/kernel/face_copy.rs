// SPDX-License-Identifier: MIT
//! The faces a run of features made, as solids a pattern copies
//! (`geometry/include/mitcad/geometry/face_copy.hpp`).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    unsafe extern "C++" {
        include!("bridge/face_copy.hpp");
        include!("bridge/trycatch.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type ShapeList = crate::kernel::shape::ffi::ShapeList;

        /// The material `after` has and `before` lacks, and the material
        /// it lacks.
        type FeatureChanges;

        fn feature_changes(before: &Shape, after: &Shape) -> Result<UniquePtr<FeatureChanges>>;

        fn added_count(self: &FeatureChanges) -> usize;
        fn added(self: &FeatureChanges, index: usize) -> SharedPtr<Shape>;
        /// Null when the added material has no seam.
        fn added_seam(self: &FeatureChanges, index: usize) -> SharedPtr<Shape>;
        fn removed_count(self: &FeatureChanges) -> usize;
        fn removed(self: &FeatureChanges, index: usize) -> SharedPtr<Shape>;

        /// The parts of copies that stand on the body (a null entry for
        /// one with nothing under it); a seam may be null.
        fn supported_copies(
            body: &Shape,
            solids: &ShapeList,
            seams: &ShapeList,
        ) -> Result<UniquePtr<ShapeList>>;
    }
}
