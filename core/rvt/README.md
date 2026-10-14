# mitcad-rvt: the `.rvt` project and `.rfa` family file reader

`core/rvt` reads `.rvt` project files and `.rfa` family files (and their
templates, `.rte` and `.rft`): the container, the metadata, the file's own
serialisation schema, the element table and the element records, walked
field by field by that schema, as typed views (category, class, level,
family and type, transform, bounding box, saved meshes, levels,
materials). It has no geometry: the `.rvt` import (mitcad#274,
`mitcad_import::rvt`, `core/ffi/src/rvt_import.rs`, the `import_rvt`
command in [commands.md](../model/src/api/commands.md#rvt-import),
`mitcad-cli import-rvt`, File › Open) builds the bodies through the
model's geometry kernel.

The container follows Microsoft's published specification (MS-CFB; the
`mitcad-cfb` crate, shared with the `.ipt` reader). Everything else comes
from public format notes and from inspecting files. One public reader of
the format (rvt-rs 0.4.0, Apache-2.0) was read for understanding and its
outputs on the test corpus serve as a cross-check in the tests; its code
was neither copied, translated nor linked. Where this reader and that one
disagree, this README says why. Claims are marked as in
[core/ipt](../ipt/README.md): *(verified)* means checked on every file of
the test corpus (below), *(seen)* means observed but not understood
further.

## Modules

| Module | Contents |
|---|---|
| `gzip` | Page trailers, gzip members (RFC 1952) with their CRC-32 and size checked, members inflated on demand; writers for tests. |
| `info` | `BasicFileInfo` (release, build; personal keys left out), `PartAtom`, `TransmissionData`, the preview PNG. |
| `schema` | The serialisation schema (`Formats/Latest`): classes, bases, fields, type encodings, the known unknowns. A writer for tests. |
| `walk` | Records walked by the schema: objects, pointers and the objects behind them, partial walks. |
| `partition` | The partition streams' two record chains (element headers, element objects). A writer for tests. |
| `elements` | The element table and the typed views: elements, levels, materials, type boxes, saved meshes. |
| `categories` | Built-in categories' names and which are building elements; system families by type kind. |
| `testfile` | A small project and family written by the writers (the tests', the CLI's and the UI test's). |
| `bin/mitcad-rvt-inspect` | `list`, `info [--json]`, `stream <path> [--raw]`, `schema [<class>] [--json]`, `table`, `elements [--json\|--csv] [--all]`, `levels`, `materials`, `record <id> [--all]`, `classes [<class>]`, `preview`, `write-test`. |

## The container

An MS-CFB compound file of **version 4 with 4096-byte sectors**, mini
sectors of 64 bytes and a mini-stream cutoff of 4096 *(verified)*. Entry
names repeat across storages (`Formats/Latest`, `Global/Latest`), so
streams are found by their full paths. At the root:

| Stream | Contents |
|---|---|
| `BasicFileInfo` | The saving release and build, the original path, a text block of `Key: value` lines *(verified)* |
| `Contents` | A wrapper (`62 19 22 05`) and one gzip member of UTF-16 text *(seen)* |
| `Formats/Latest` | The serialisation schema |
| `Global/ElemTable` | The element table |
| `Global/Latest`, `PartitionTable`, `History`, `DocumentIncrementTable`, `ContentDocuments` | The document's own objects, walked by the same schema *(verified for walking; not used)* |
| `Partitions/NN` | The elements' records; NN names a partition, not the release *(verified)* |
| `PartAtom` | A family's Atom XML (families only) *(verified)* |
| `TransmissionData` | The linked files (XML) *(verified)* |
| the preview stream (`…Preview4.0`) | The wrapper, then a PNG *(verified where present: 30 of 37 files)* |
| `ProjectInformation` | *(seen; not read)* |

## Pages and gzip members

The schema, every `Global/*` stream and every partition are stored in
pages of 65,249 bytes: 64,896 bytes of the stream and a 353-byte trailer,
the last page shorter and without one. With the trailers removed, the
schema is one gzip member at offset 0, a `Global/*` stream an 8-byte
prefix and one member, a partition a 44-byte header and members back to
back (3 to 935) *(verified)*. Every member is complete: its CRC-32 and
size are checked *(verified)*. The trailers' contents are not read
*(seen)*. The partition header's first word is 7, 8 or 9 by release
*(seen)*.

## Metadata

`BasicFileInfo` starts with a layout word (10 for the 2016–2018 releases,
13 for 2019 and 2020, 14 from 2021) *(verified)*; from byte 14 come
strings of an `i32` count of UTF-16 units: the release and the build
(layout 13 and later) or one text naming both (layout 10), then the
original path *(verified)*. The release is the text block's `Format`
line (2019 on), else the first string's year *(verified)*. The block's
user names and paths are left out of everything the reader shows.

The element table's class tag (its first word, see below) also names the
release, one value per release *(verified)*:

| Release | 2016 | 2017 | 2018 | 2019 | 2020 | 2021 | 2022 | 2023 | 2024 | 2025 | 2026 | 2027 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Tag | 1174 | 1191 | 1232 | 1233 | 1244 | 1271 | 1312 | 1370 | 1411 | 1451 | 1481 | 1510 |

`PartAtom` is UTF-8 Atom XML: title, id, update time, categories (term
and scheme) *(verified)*. `TransmissionData` is a `u32` count of UTF-16
units, then XML of `ExternalFileReference` elements (`ElementId`,
`ExternalFileReferenceType`, `DesiredPath`, `DesiredLoadState`, …); the
reader keeps the file names, not the folders *(verified)*. The preview is
the PNG from its signature to the end of its `IEND` chunk *(verified)*.

## The serialisation schema

`Formats/Latest` describes every class the file stores *(verified: every
corpus schema reads to its end, 3,792 classes in 2016 to 4,757 in 2027)*:

- records separated by a zero `u16`; a class is a `u16`-counted ASCII
  name, a `u16` base reference, a `u32` version, a `u32` field count, the
  fields (a `u32`-counted name and a type) and a `u32`-counted list of
  16-byte GUIDs;
- a class reference is a `u16`; with bit 15 set the class is defined
  inline right after a zero `u16`, and the low 15 bits repeat its tag
  (checked);
- a class's tag is its definition ordinal, from 12, inline definitions
  counted where they appear: tags are never stored, and they shift from
  release to release *(verified: the element records' class tags name the
  classes the reference reader names on every compared element)*;
- a type is a `u32` word: a kind byte, a modifier byte and 16 bits of
  flags; modifier 0x10 adds a `u32` array count, kind 0x0d adds
  `01 00 00 00 20` and an inner type (and a class reference when the inner
  type has one), kind 0x0e with a by-value modifier adds a class
  reference.

The schema is identical in every file of one release *(verified)*.

### Field encodings

What the walker reads for each type *(verified: every element record of
every corpus file walks to its end with these, but the extensible storage
data below)*:

| Kind | Stored as |
|---|---|
| 0x01 | a byte (boolean) |
| 0x02 | a byte |
| 0x03 | two bytes |
| 0x04, 0x05 | four bytes |
| 0x06 | a `f32` |
| 0x07 | a `f64` |
| 0x08 (modifier 0x60) | a `u32` count of UTF-16 units, then the units |
| 0x09 | 16 bytes (a GUID) |
| 0x0a | a `u16` class tag |
| 0x0b | eight bytes |
| 0x0d | its inner type |
| 0x0e, by value | the class's fields in place; an `ElementId` is its number: 4 bytes through 2023, 8 from 2024 (the schema's `Identifier` says which) |
| 0x0e, pointer 1, 2, 4 | a `u32` (0 null; all ones: an object of its own; else an object number, the first pointer with it bringing the object) and a `u16` class tag; the object follows the record's fields |
| 0x0e, pointer 3 | a `u32` (the document) |

Modifier 0x10 makes a fixed array of the type, 0x50 a container (a `u32`
count, then the elements). A field's flags 0x02 mark it as not stored
(only in the 2027 schema, on fields named as temporary, cached or unsaved
state) *(verified on the 2027 file)*.

No corpus schema has a type word outside this table (the tests fail on
one). The **known unknowns** (the storage is known, the meaning is not;
also printed by `mitcad-rvt-inspect schema`):

- flag 0x04 (`VarExpr.m_refCt` in every release): stored as its kind says;
- flags 0x10 and 0x20: seen only with the not-stored flag 0x02;
- pointer kinds 2 and 4: stored as kind 1; what sets them apart is not known;
- kinds 0x04 and 0x05: both four-byte integers; which is signed is not known;
- the classes' GUID lists.

## Records

An object is its class's fields in declaration order, the root base
class's first. The objects behind its pointers are not in place: each
pointer leaves its class tag, and the objects follow the record's own
fields in the order their pointers came, the pointers in those objects
queued behind *(verified)*. A record's walk must end exactly at its end,
so a misread is found, not passed on.

**Extensible storage.** An element whose cells hold extensible storage
entities (`ESEntityCell`) stores each entity's data (`ESEntity.m_blob`)
in a schema of its own, identified by a GUID, which `Formats/Latest` does
not describe. The walker stops there and keeps what it read: the
element's own fields always come before such data *(verified: 1,330
elements of the largest project, 1,171 of the MEP sample, 7 of each
structural sample, 1 to 5 elsewhere; one marshalled element of the MIT
project likewise)*. Reading those schemas is a later step.

## Partitions

A partition's inflated bytes hold two chains of records *(verified on
every partition of the corpus)*:

1. the element headers: `id`, `u32` size, the `ElementHeader` object (its
   `u16` class tag and fields), the size again; ended by an `id` of all
   ones and a `u64` 0;
2. the element objects, in the same order: `id`, a `u32` *(seen; not
   read)*, `u32` size, the element's object, the size again; ended by an
   `id` of all ones and a `u64` 1.

What follows (the loaded families' own documents, with their own ids) is
not read, and its members are not inflated: a partition is inflated only
as far as its chains go. An element can have records in several
partitions; the one in the partition with the highest number is the
element as last saved *(verified: on the MIT project the walls' types in
the higher partition are the IFC export's, those in the lower an earlier
state)*.

The `ElementHeader` gives `m_categroryId` (the category: a negative id of
the public built-in category enumeration), `m_familyId`, `m_ownerViewId`
(view-specific elements), `m_designOptionId`, `m_unplacedOwnerId`
(members of a group's definition), `m_viewRules`, `m_classDef` (the
element's class tag), `m_pBBox` (an `Outline`: the bounding box in model
feet) and `m_parents` (lists of elements it depends on).

## The element table

`Global/ElemTable` is an `ElemTable` object (its class tag, then its
fields): `m_elemArr`, an `ElemRec` per element (`m_history`, `m_id`,
`m_OwningElementId`, `m_partitionId`), then one 32-bit word *(verified:
the walk ends exactly 4 bytes before the stream's end)*. Its records are
28 bytes through 2023 and 40 from 2024 because the ElementId is 4 or 8
bytes. Every declared element has records in the partitions *(verified)*.

The reference reader reads this table from 24 bytes into it, in records
of 28 or 40 bytes: it leaves out the first element and (but in projects of
28-byte records) reads one record past the table; on one family it found
no record size at all. Its ids are otherwise exactly ours, in order
*(verified)*.

## The views

All values are fields the schema names, read by their names:

- **Element:** the header's category, class, box, view, design option and
  group definition; `Element.m_assocLevelId`; the family instance's
  transform (`m_pInstanceInfo` → `InstInfoBase.m_Trf`: a 3×3 matrix by
  rows, whose columns are the instance's axes, and an origin in feet) and
  placed symbol (`m_symbolId`); the host (`m_hostId`).
- **Level:** an element's own `m_assocLevelId`; else the first own
  ElementId field that names a level (`RoomElem.m_levelId`, …); else the
  first level its record names anywhere (a beam's
  `FamInstCurveDriver.m_attachmentData.m_levelReferenceId`, a column's
  `FamilyLevelRef`); else an ElementId parameter that names one; else its
  host's; else that of the first element it names that has one (a work
  plane, a stair).
- **Type:** the first own ElementId field naming a type (a class derived
  from `Symbol`) that has a name: `FamilyInstance.m_masterSymbolId`,
  `VWall.m_WallAttributesId`, `Floor.m_floorAttributesId`,
  `BuildingPad.m_buildingPadTypeId`, `StairsSupport.m_typeId`, …; a stair
  run's or landing's is its stair type's `m_runTypeId` or
  `m_landingTypeId`.
- **Names:** a type's `m_symbolInfo` → `SymbolInfo.m_name`; a family's
  `FamilyBase.m_name` (a family type's `FamilySymbol.m_familyId`); a
  level's `DatumPlane.m_text` and its elevation the origin of its
  `m_pSurface` plane; a material's `MaterialElem.m_pMaterial` →
  `Material.m_name`, `m_color` (red, green, blue in the low bytes) and
  `m_transparency`.
- **System families** are not stored: their names come from the type's
  kind, as the program shows them: `BasicWallType` Basic Wall,
  `NewCurtainWallType` Curtain Wall, `FloorAttributes` Floor,
  `BuildingPadType` Pad, `CompoundCeilingType` Compound Ceiling,
  `RoofAttributes` Basic Roof, `StairsRailingAttr` Railing,
  `CorniceAttr` Wall Sweep, `EdgeSlabAttr` Slab Edge, `RampAttributes`
  Ramp, `Text3dAttrSymbol` Model Text; stair runs and landings Monolithic
  or Non-Monolithic by `m_hasMonolithicSupport`, stairs Cast-In-Place
  (a monolithic run type) or Assembled, stair supports Stringer or
  Carriage by the stair type fields that name them *(verified: every
  compared element's family name)*.
- **Type boxes:** a family type's `FamilySymbol.m_outline`, in its own
  coordinates.
- **Saved meshes:** a `GPolyMesh` in an element's record →
  `m_pFacetedTopology`, a `FacetedTopology` of `f32` points in model feet
  and `u16` triangles *(verified on the corpus's topography surfaces,
  their points within their elements' boxes)*. B-rep snapshots of the
  elements' geometry steps (`SnapshotData`, `GElement`, `Face`, `Edge`)
  are in the records too *(seen; phase 2)*.

Built-in categories are not named in the file; `categories` lists the
public enumeration's names of the common ones.

## The test corpus

37 files under `MITCAD_RVT_CORPUS` (never in the repository): a sample
family saved by every release 2016–2026, a structural sample project by
every release 2016–2027, further 2024–2026 projects, an MIT-licensed 2024
project (`c805df44`), a 2023 project and four 2020 families. Every file
opens with its release, kind, preview (the reference reader's image byte
for byte), schema (whole, no unknown encoding, every class the reference
reader finds) and element table; every element header record walks to its
end, and every element object record but those that stop at extensible
storage data *(verified)*.

On the 2024 and 2025 projects every element the reference reader lists
agrees on its category, class, box (where the reference keeps the stored
box: it trims walls at joins, cuts columns by walls and draws some by
their type's profile), transform, type, type name, family name and level,
and every material on its name, colour and transparency, with these
explained differences (`tests/corpus.rs`):

- **Levels the reference finds by its own rules** (the nearest level below
  the box, a host of the same class, a constraint read by position) where
  this reader reads a level from the element's records: 132 elements of
  `6dc833f9` (beams on their work planes' levels; a sloped beam on the
  first of its two levels), 66 of `33271010` (slab edges on their slabs'
  levels, light fixtures on their hosts'), 1 of `037165e5` (model text
  whose record names no level).
- **Materials the reference misreads** (a black colour and a denormal
  transparency): 4 to 24 per file, not compared.
- `33271010`: a material whose own name differs from the one the
  reference reads, and 7 stair supports whose own `m_typeId` names
  another type than the reference reads.

| Content id | Release | Declared | Compared elements | Disagreements explained |
|---|---|---|---|---|
| `c805df44` | 2024 | 26,425 | 970 | none |
| `33271010` | 2024 | 47,233 | 6,075 | 66 levels, 7 types, 1 material |
| `6dc833f9` | 2024 | 16,571 | 1,282 | 132 levels |
| `ff791052` | 2024 | 17,120 | 5,657 | none |
| `bb62c161` | 2024 | 8,288 | 464 | none |
| `0eeb6814` | 2024 | 13,738 | 905 | none |
| `aab838f8` | 2024 | 13,819 | 454 | none |
| `41f1893a` | 2025 | 8,327 | 464 | none |
| `037165e5` | 2025 | 28,070 | 6,729 | 1 level |
| `5089b2c6` | 2025 | 13,861 | 454 | none |

The other releases' element records walk with the same schema-driven
reader; their views are not compared (the reference reader reads only
metadata there).

## Tests

`cargo test -p mitcad-rvt` (`core.rvt` in ctest): the written project and
family (container, pages, members, metadata, schema, walks, chains,
element views, saved meshes) and `tests/corpus.rs` on the corpus (skipped
without `MITCAD_RVT_CORPUS`). The import: `core/import/src/rvt.rs` (mock
kernel), `core.exchange`, `cli.import_rvt`, `cli.import_rfa`,
`rvt.corpus` and `tools/ui-rvt-import-test.sh`.
