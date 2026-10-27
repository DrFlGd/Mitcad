# mitcad-import-primitives

The primitives family of the `.f3d` dump replay (mitcad#25): boxes,
cylinders, spheres, tori and coils placed on a plane or face as MitCad
features, where MitCad's placement on the plane gives the file's. The
module docs (`src/primitives.rs`) describe the rules.

It sits above the replay's core
([mitcad-import-replay](../import-replay/README.md), mitcad#785): its
methods are the extension trait `Primitives` on the core's `Importer`,
which the core reaches only through its `Families` trait (`primitive`,
`coil_primitive`), filled by `mitcad-import`. An edit here rechecks this
crate and `mitcad-import`, not the core.

Tests: the replay's tests in `mitcad-import` (`tests::primitive_tests`,
ctest `core.import`); the crate has no tests of its own.
