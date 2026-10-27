# mitcad-import-joints

The joints family of the `.f3d` dump replay: the file's joints, as-built
joints, joint origins, captured positions, rigid groups, ground items
(mitcad#55) and assembly relationships (mitcad#155) as MitCad's. The
crate docs of `src/joints.rs` and `src/relationships.rs` describe what
comes in how; the notes are in
[core/import/docs/f3d-components.md](../import/docs/f3d-components.md).

It sits above the replay's core
([mitcad-import-replay](../import-replay/README.md), mitcad#785): its
methods are the extension traits `Joints` and `Relationships` on the
core's `Importer`, and the core's timeline walk reaches them only through
its `Families` trait (`assembly_item`), which `mitcad-import` fills. An
edit here rechecks this crate and `mitcad-import`, not the core.

Tests: the replay's tests in `mitcad-import` (`tests::joint_tests`,
`tests::relationship_tests`, ctest `core.import`) run the whole replay
with this family; the crate has no tests of its own.
