// SPDX-License-Identifier: MIT
//! Imported bodies and data exchange: the B-rep data of base features
//! (`geometry/include/mitcad/geometry/import.hpp`), reading and writing
//! files (`geometry/io`) and building the bodies of .f3d files
//! (`geometry/include/mitcad/geometry/brep_import.hpp`).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// What a body is made of (`mitcad::geometry::BodyKind`).
    enum BodyKind {
        Empty,
        Solid,
        Sheet,
        Mesh,
    }

    /// Name and colour of a body read from or written to a file; its shape
    /// is at the same index of the shape list.
    struct BodyInfo {
        name: String,
        has_color: bool,
        /// sRGB components in [0, 1].
        color: [f64; 3],
        /// Writing: where the body goes (`mitcad::io::Placement`), once as
        /// it is when empty. Reading leaves it empty.
        placements: Vec<BodyPlacement>,
    }

    /// A placement of a body to write (mitcad#19): the rotation's rows and
    /// the translation (a rotation and a translation only), and the name of
    /// what places it (an occurrence path; may be empty).
    struct BodyPlacement {
        rotation: [f64; 9],
        translation: [f64; 3],
        name: String,
    }

    enum ExportFormat {
        Step,
        Iges,
        Stl,
        Obj,
        Brep,
    }

    enum LengthUnit {
        Millimeter,
        Centimeter,
        Meter,
        Inch,
        Foot,
    }

    struct ExportSettings {
        format: ExportFormat,
        /// STEP AP242 instead of AP214.
        ap242: bool,
        /// STEP and IGES.
        unit: LengthUnit,
        /// STL and OBJ triangulation: surface deviation (mm) and the angle
        /// between neighbouring facet normals (radians).
        deviation: f64,
        angle: f64,
        /// STL as text.
        ascii: bool,
    }

    /// A body of an .f3d file and how building it went. Its shape (null
    /// when not built) is at the same index of the shape list.
    struct F3dBody {
        /// Document of an .f3z package; empty for an .f3d.
        document: String,
        /// Zip entry of the body blob and the body's record in it.
        blob: String,
        record: u32,
        /// The blob is an .smbh (bodies with ASM history).
        history: bool,
        /// Saved at the top level of the blob (not only the owner of
        /// faces saved there).
        top_level: bool,
        built: bool,
        /// Every lump became a solid.
        solid: bool,
        /// OCCT's checker accepts the shape.
        valid: bool,
        volume: f64,
        area: f64,
        faces: i32,
        /// Converter issues and geometry the builder rejected.
        issues: u32,
        /// Why it was not built.
        error: String,
        /// The builder's messages: geometry it rejected, and what OCCT's
        /// checker finds wrong.
        messages: Vec<String>,
        /// The volume before the solids were oriented (negative when the
        /// faces have the material on their outer side).
        raw_volume: f64,
    }

    unsafe extern "C++" {
        include!("bridge/exchange.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type ShapeList = crate::kernel::shape::ffi::ShapeList;

        /// Reads B-rep data; face i is named `<feature>:import(<first_face + i>)`.
        fn import_brep(feature: &str, data: &[u8], first_face: u32) -> Result<SharedPtr<Shape>>;
        fn face_count(shape: &Shape) -> usize;
        /// OCCT binary B-rep of the shape.
        fn brep_data(shape: &Shape) -> Result<Vec<u8>>;
        /// A compound of the shapes, with their face names.
        fn compound(shapes: &ShapeList) -> Result<SharedPtr<Shape>>;
        fn body_kind(shape: &Shape) -> BodyKind;

        /// Reads a STEP, IGES, BRep, STL or OBJ file (by extension) into
        /// the list; `unit_mm` scales STL and OBJ.
        fn read_file(
            path: &str,
            unit_mm: f64,
            shapes: Pin<&mut ShapeList>,
        ) -> Result<Vec<BodyInfo>>;
        fn write_file(
            path: &str,
            shapes: &ShapeList,
            bodies: &[BodyInfo],
            settings: &ExportSettings,
        ) -> Result<()>;

        /// Builds the bodies of an .f3d or .f3z file with OCCT: the
        /// top-level bodies of the `.smb` blobs, with `history` also of the
        /// `.smbh` blobs and with `owners` also the bodies that only own
        /// faces saved at the top level.
        fn f3d_bodies(
            path: &str,
            history: bool,
            owners: bool,
            shapes: Pin<&mut ShapeList>,
        ) -> Result<Vec<F3dBody>>;

        // .f3d import (T1).
        #[namespace = "mitcad::f3d"]
        type BrepBodyData = crate::brep_import::ffi::BrepBodyData;
        /// Builds one body of an .f3d file from its neutral B-rep data
        /// (`brep_import.rs`); null when it cannot be built, an error when
        /// an allocation failed meanwhile (the process is out of memory,
        /// mitcad#132).
        fn f3d_build_body(data: &BrepBodyData) -> Result<SharedPtr<Shape>>;
        /// A body `f3d_build_body` built, from its B-rep data (`brep_data`):
        /// the next try of an import takes it from one the watchdog gave up
        /// while it built it (mitcad#82); null when it cannot be read.
        fn f3d_read_body(data: &[u8]) -> SharedPtr<Shape>;
        /// Crashes inside OCCT as errors from now on
        /// (`mitcad/geometry/guard.hpp`).
        fn catch_occt_crashes();

        // .f3d import: healed faces reused between history states
        // (mitcad#181).
        /// A body built by `f3d_build_body_from`, with what the builds of
        /// its other states can take from it.
        type F3dBuilt;
        /// The builds a body's build takes healed faces from.
        type F3dBases;
        fn f3d_bases() -> UniquePtr<F3dBases>;
        /// Adds a build to the bases (one that kept nothing adds nothing).
        fn f3d_bases_add(bases: Pin<&mut F3dBases>, built: &SharedPtr<F3dBuilt>);
        /// `f3d_build_body`, taking the faces whose data is the same as
        /// one of the bases' as they were healed there (the base that
        /// shares the most), and healing only the others; the body as a
        /// whole when less than half are shared or that is not valid.
        fn f3d_build_body_from(
            data: &BrepBodyData,
            bases: &F3dBases,
        ) -> Result<SharedPtr<F3dBuilt>>;
        /// The body built (null when it does not build).
        fn f3d_built_shape(built: &F3dBuilt) -> SharedPtr<Shape>;
        /// How many of its faces were taken from a base.
        fn f3d_built_reused(built: &F3dBuilt) -> u32;
        /// Whether it keeps healed faces for the builds of other states.
        fn f3d_built_kept(built: &F3dBuilt) -> bool;
        /// Why it was healed as a whole although it had bases (empty when
        /// it was not).
        fn f3d_built_note(built: &F3dBuilt) -> String;

        // .ipt import (mitcad#60).
        /// Builds one body from its neutral B-rep data with OCCT and
        /// reports how it went, as `f3d_bodies` does for each body; its
        /// shape (null when not built) is appended to the list.
        fn build_brep_body(data: &BrepBodyData, shapes: Pin<&mut ShapeList>) -> F3dBody;
        /// Whether OCCT's checker finds the shape valid (the bodies of the
        /// `.ipt` import with its history, mitcad#60); an error when an
        /// allocation failed in the checker (mitcad#132).
        fn shape_is_valid(shape: &Shape) -> Result<bool>;
    }

    extern "Rust" {
        /// Writes a small .f3d file for tests: Mitcad's own ASM test bodies
        /// (a 10 mm cube and a cylinder of radius 10 mm and height 20 mm)
        /// in a `.smb` blob each, without the rest of an .f3d design.
        fn f3d_write_test_file(path: &str) -> Result<()>;
        /// Writes a test assembly of components of other documents with its
        /// reference folder (`mitcad_f3d::testreferences`, mitcad#196) into
        /// `dir`: `Assembly v1.f3d` and `Assembly v1.refs/` with
        /// `Holder v1.f3d`, `Part v2.f3d` and `references.json`; or the
        /// two files in `<folder>/` without `references.json`.
        fn f3d_write_test_references(dir: &str, folder: &str) -> Result<()>;
        /// Writes the test assembly as `.f3z` packages into `dir`
        /// (`mitcad_f3d::testreferences::package_file`, mitcad#229):
        /// `Package.f3z` with its `Manifest.json` and
        /// `DesignDescription.json`, and `Package-plain.f3z` without them.
        fn f3d_write_test_package(dir: &str) -> Result<()>;

        // .ipt import (mitcad#60).
        /// Writes a small .ipt part file for tests (`mitcad_ipt::testdata`):
        /// the cube and the cylinder above, each in a B-rep record of its
        /// own, part number `MITCAD-TEST-1`, material `Aluminum`, inches.
        fn ipt_write_test_file(path: &str) -> Result<()>;
        /// Writes a small .ipt part file with a design for tests
        /// (`mitcad_ipt::testdata::test_part_with_design`): the 10 mm cube
        /// and the parameter, sketch and extrusion that make it, mm.
        fn ipt_write_test_design_file(path: &str) -> Result<()>;
        /// Writes .ipt part files without B-rep bodies for tests: with a
        /// mesh feature's tetrahedron (`mitcad_ipt::testdata::test_mesh_part`),
        /// or empty (`test_empty_part`).
        fn ipt_write_test_mesh_file(path: &str) -> Result<()>;
        fn ipt_write_test_empty_file(path: &str) -> Result<()>;
        /// Writes an .ipt part file for tests whose result list keeps a
        /// body hidden (`mitcad_ipt::testdata::test_part_with_hidden_body`):
        /// the cube shown, the cylinder hidden, mm.
        fn ipt_write_test_hidden_file(path: &str) -> Result<()>;
        /// Writes a small project with .iam assemblies into a folder for
        /// tests (`mitcad_ipt::testassembly::write_test_project`): a top
        /// assembly as saved elsewhere, a sub-assembly and parts.
        fn ipt_write_test_assembly(dir: &str) -> Result<()>;

        // .f3d import (mitcad#82).
        /// `f3d_build_body` advanced (its healing, face by face): progress
        /// of the import running on this thread, for its watchdog.
        fn f3d_build_progress();
    }
}

// A build is not changed once made; the builds of other states read its
// shapes (copying the faces they take) as the replay reads its body (see
// `ffi::Shape`'s).
unsafe impl Send for ffi::F3dBuilt {}
unsafe impl Sync for ffi::F3dBuilt {}

/// See [`ffi::f3d_build_progress`].
fn f3d_build_progress() {
    crate::f3d_import::build_progress();
}

/// The blobs of [`ffi::f3d_write_test_file`] in a zip of stored entries.
fn f3d_write_test_file(path: &str) -> Result<(), String> {
    use mitcad_f3d::testdata::{cube_blob, cylinder_blob};
    use mitcad_f3d::zip::{Method, Writer};

    let blob = |name: &str| format!("FusionAssetName[Active]/Breps.BlobParts/BREP.{name}.smb");
    let mut writer = Writer::new();
    for (name, data) in [
        (blob("mitcad-test-cube"), cube_blob()),
        (blob("mitcad-test-cylinder"), cylinder_blob()),
    ] {
        writer
            .add(&name, &data, Method::Stored)
            .map_err(|e| e.to_string())?;
    }
    let out = writer.finish().map_err(|e| e.to_string())?;
    std::fs::write(path, out).map_err(|e| format!("{path}: {e}"))
}

/// The files of [`ffi::f3d_write_test_references`].
fn f3d_write_test_references(dir: &str, folder: &str) -> Result<(), String> {
    let dir = std::path::Path::new(dir);
    for (path, data) in mitcad_f3d::testreferences::files() {
        if !folder.is_empty() && path.ends_with("references.json") {
            continue;
        }
        let path = match path.split_once('/') {
            Some((_, file)) if !folder.is_empty() => format!("{folder}/{file}"),
            _ => path,
        };
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// The packages of [`ffi::f3d_write_test_package`].
fn f3d_write_test_package(dir: &str) -> Result<(), String> {
    let dir = std::path::Path::new(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, described) in [("Package.f3z", true), ("Package-plain.f3z", false)] {
        let path = dir.join(name);
        std::fs::write(&path, mitcad_f3d::testreferences::package_file(described))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// The part of [`ffi::ipt_write_test_file`].
fn ipt_write_test_file(path: &str) -> Result<(), String> {
    std::fs::write(path, mitcad_ipt::testdata::test_part()).map_err(|e| format!("{path}: {e}"))
}

/// The part of [`ffi::ipt_write_test_design_file`].
fn ipt_write_test_design_file(path: &str) -> Result<(), String> {
    std::fs::write(path, mitcad_ipt::testdata::test_part_with_design())
        .map_err(|e| format!("{path}: {e}"))
}

/// The part of [`ffi::ipt_write_test_mesh_file`].
fn ipt_write_test_mesh_file(path: &str) -> Result<(), String> {
    std::fs::write(path, mitcad_ipt::testdata::test_mesh_part()).map_err(|e| format!("{path}: {e}"))
}

/// The part of [`ffi::ipt_write_test_empty_file`].
fn ipt_write_test_empty_file(path: &str) -> Result<(), String> {
    std::fs::write(path, mitcad_ipt::testdata::test_empty_part())
        .map_err(|e| format!("{path}: {e}"))
}

/// The part of [`ffi::ipt_write_test_hidden_file`].
fn ipt_write_test_hidden_file(path: &str) -> Result<(), String> {
    std::fs::write(path, mitcad_ipt::testdata::test_part_with_hidden_body())
        .map_err(|e| format!("{path}: {e}"))
}

/// The project of [`ffi::ipt_write_test_assembly`].
fn ipt_write_test_assembly(dir: &str) -> Result<(), String> {
    mitcad_ipt::testassembly::write_test_project(std::path::Path::new(dir))
        .map_err(|e| format!("{dir}: {e}"))
}
