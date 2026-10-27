# mitcad-ffi

The static library of the C++ bridges (the CMake target `mitcad_ffi`, which
the application, `mitcad-cli` and the C++ tests link). It has no code of its
own: it links `mitcad-ffi-bridges` (`core/ffi`), where the bridges, the
`Kernel` on OCCT and the document API for C++ are, with every Rust library
they use.

The split (mitcad#792) is for build times. Cargo starts an rlib as soon as
its dependencies' metadata is ready, beside their code generation, but a
static library only after their code is complete. With the bridges in an
rlib, an edit in the model or an import crate no longer makes the bridges
wait for that crate's code generation; only this crate and the archive do.

The cxx bridges' C++ side is still generated from `core/ffi/src`
(`corrosion_add_cxxbridge` in `core/CMakeLists.txt`, through the target
`mitcad_ffi_bridges_sources` that names `core/ffi/Cargo.toml`).
