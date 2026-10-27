# SolidCraft: format upgrade steps, cam geometry and plastic rules

The `.solidcraft` import (`core/solidcraft`, mitcad#450) reads designs of
every SolidCraft file format by first upgrading an older file to the one
format it maps (27 with the changes pending after it). The upgrade steps
are SolidCraft's own, adapted here. The cam geometry of motion links
(`cam.rs`, mitcad#466: where a follower touches a cam's profile over a
turn) is adapted from SolidCraft too and built into `core/model`
(`motion_links.rs`). The plastic rule library (`plastic.rs`, mitcad#459)
is SolidCraft's too, as data the import maps onto MitCad's plastic
rules.

- Source: https://github.com/tangency-labs/solidcraft, commit
  `0b787f8eae4244cd40f07dddccd2e1b27e4cf9f8` (19.10.2026, after release
  0.16.2).
- Licence: MIT OR Apache-2.0, Copyright (c) 2026 SolidCraft contributors;
  used under MIT. Both licence texts are here (`LICENSE-MIT`,
  `LICENSE-APACHE`, copied from the repository). `format.rs`, `cam.rs`
  and `plastic.rs` keep the licence and copyright in their headers.
- Nothing of SolidCraft is a dependency: no crate of it is built or linked.

## Files

| Here | From | SHA-256 of the source file |
|---|---|---|
| `format.rs` | `crates/doc/src/format.rs` (`v2_to_v3`, `v3_to_v4`, `v4_to_v5`, `v5_to_v6`, `v13_to_v14` and their helpers) | `7a9c0001c9f7680b818d52187433126e662c20a8eb95c227abcca1a6ef669558` |
| | `crates/doc/src/format/pending/two-sided-extrude-direction.rs` (`step`) | `9f540a89ce06f08d99819042ce8a81168b36c3c5cd6f20a9147da28df65b3b47` |
| | `crates/doc/src/params.rs` (`Document::name_all_feature_inputs`) | `5e453620d6be17163b0dcfb4d7f7c0d3983c64d05521ba4671ae407c02a22515` |
| `cam.rs` | `crates/doc/src/joints_cam.rs` (the follower's contact with a cam's profile: `first_contact_along`, `touches_turning`, `overlaps_turning`, `first_contact_turning`, `next_contact_turning`, `dist_to_segment`, `inside`, `area`, `round`, the sampling loop of `table`) | `b31e49db7a82e8a72a472d2f2b128b5dce3c71ad6926093eb38436bc59a877cc` |
| `mesh_convert.rs` | `crates/kernel/src/mesh_convert.rs` (Convert Mesh, mitcad#462, built into `core/model/src/features/mesh_convert.rs`: `convert_prismatic_mesh`, `weld`, `extrusion`, `certify_nesting`, `fit_loop`) | `982246e109a52b8270d07073fad4ebcf144bdbde16c05096a165a7747665905e` |
| `plastic.rs` | `crates/doc/src/plastic.rs` (`PlasticRule::new`, `PlasticRule::library`, the order of `PlasticRule::fields`; mitcad#459) | `2a6dbad07597fe331bb5cf2a1f11f12e1bfbc38a2a2f8029fc0a7152a72aa7d4` |

## Changes

- The steps work on `serde_json::Value` and return `Result<(), String>`
  instead of SolidCraft's document error.
- The upgrade path (`upgrade`) is written out for the pinned format: the
  hand-written steps up to format 22, no steps for formats 23 to 27 (their
  changes need no rewrite), then the pending changes' steps.
- `name_all_feature_inputs` works on the design's JSON; the number of
  inputs of a feature comes from MitCad's port of SolidCraft's feature
  walk (`core/solidcraft/src/inputs.rs`).
- `cam.rs`: points are `[f64; 2]` with a small vector type of its own,
  errors are `String`s, and the setup (the profile polygon, the
  follower's line or pivot and arm, the signs) comes from MitCad's frames
  and joints (`CamSetup`); the table's interpolation is the joint
  solver's (`mitcad_solver::Relation::Table`), not SolidCraft's `lift_at`.
- `mesh_convert.rs`: the mesh is MitCad's `TriangleMesh`, errors are
  `String`s, and the result is the section (a frame, the height, regions of
  polygons and circles), which the Convert Mesh feature extrudes with
  MitCad's kernel and names; the rebuilt volume compared with the mesh's is
  the section's area times the height, and the fitted loops' nesting is
  certified geometrically (raw loops may not meet, a fitted circle may not
  reach across another loop) instead of by SolidCraft's planar booleans.
- `plastic.rs` gives the rule library as the design file's JSON form of a
  rule (the form of a design's own rules), which the import maps onto
  MitCad's plastic rules.
