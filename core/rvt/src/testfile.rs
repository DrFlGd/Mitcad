// SPDX-License-Identifier: MIT
//! Small project and family files made by the writers, for tests: a
//! version 4 compound file with the metadata streams, a schema of the
//! classes the reader uses (named as the files name them, with tags of
//! their own), an element table and two partitions of element records.
//!
//! The project holds two levels, a family with a type, a column placed
//! from it (with an older record of it in the lower partition), a wall
//! type and a wall, a material, an element declared without records and a
//! record of an undeclared one.

use crate::categories::{OST_COLUMNS, OST_LEVELS, OST_TOPOGRAPHY, OST_WALLS};
use crate::gzip::{add_pages, gzip};
use crate::partition::write_partition;
use crate::schema::types::*;
use crate::schema::{SchemaWriter, Type};

pub const LEVEL_GROUND: i64 = 10;
pub const LEVEL_UPPER: i64 = 11;
pub const FAMILY: i64 = 20;
pub const SYMBOL: i64 = 21;
pub const WALL_TYPE: i64 = 22;
pub const MATERIAL: i64 = 23;
pub const COLUMN: i64 = 30;
pub const WALL: i64 = 31;
pub const NO_RECORD: i64 = 40;
pub const SURFACE: i64 = 32;
/// The element table's ids, in its order.
pub const DECLARED: [i64; 11] = [
    0,
    LEVEL_GROUND,
    LEVEL_UPPER,
    FAMILY,
    SYMBOL,
    WALL_TYPE,
    MATERIAL,
    COLUMN,
    WALL,
    NO_RECORD,
    SURFACE,
];
/// A record whose id the table does not declare.
pub const UNDECLARED: i64 = 99;

pub const PART_ATOM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<entry xmlns="http://www.w3.org/2005/Atom" xmlns:A="urn:schemas-test">
<title>Test Family &amp; Co</title>
<id>0001</id>
<updated>2020-01-01T00:00:00Z</updated>
<A:taxonomy><term>test:family</term></A:taxonomy>
<category><term>Furniture</term><scheme>std:oc1</scheme></category>
</entry>"#;

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn counted(s: &str) -> Vec<u8> {
    let mut out = (s.encode_utf16().count() as u32).to_le_bytes().to_vec();
    out.extend(utf16(s));
    out
}

/// A `BasicFileInfo` of layout 14.
pub fn basic_file_info(release: u32, build: &str) -> Vec<u8> {
    let mut out = 14u32.to_le_bytes().to_vec();
    out.extend([0u8; 10]);
    out.extend(counted(&release.to_string()));
    out.extend(counted(build));
    out.extend(counted("C:\\Projects\\test file"));
    out.push(0);
    out.extend(utf16(&format!(
        "Worksharing: Not enabled\r\nUsername: someone\r\nCentral Model Path: \r\nFormat: {release}\r\n\
         Build: {build}\r\nLast Save Path: C:\\Projects\\test file\r\nLocale when saved: ENU\r\n"
    )));
    out
}

/// `TransmissionData` naming one linked model.
pub fn transmission_data() -> Vec<u8> {
    let xml = "<?xml version=\"1.0\"?>\r\n<TransmissionData><ExternalFileReference>\
               <ElementId>1234</ElementId><ExternalFileReferenceType>Link</ExternalFileReferenceType>\
               <DesiredPath>C:\\Models\\linked.rvt</DesiredPath><DesiredLoadState>Loaded</DesiredLoadState>\
               </ExternalFileReference></TransmissionData>";
    let mut out = (xml.encode_utf16().count() as u32).to_le_bytes().to_vec();
    out.extend(utf16(xml));
    out
}

/// A preview stream: the wrapper, some bytes, a 2×1 PNG and bytes after it.
pub fn preview() -> Vec<u8> {
    let mut out = crate::WRAPPER_MAGIC.to_vec();
    out.extend([0u8; 40]);
    out.extend(crate::info::PNG_SIGNATURE);
    let chunk = |out: &mut Vec<u8>, kind: &[u8], data: &[u8]| {
        out.extend((data.len() as u32).to_be_bytes());
        out.extend(kind);
        out.extend(data);
        let mut crc_input = kind.to_vec();
        crc_input.extend(data);
        out.extend(crate::gzip::crc32(&crc_input).to_be_bytes());
    };
    let mut ihdr = 2u32.to_be_bytes().to_vec();
    ihdr.extend(1u32.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(
        &mut out,
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&[0, 255, 0, 0, 0, 0, 255], 6),
    );
    chunk(&mut out, b"IEND", &[]);
    out.extend([0u8; 8]);
    out
}

/// Record bytes, written field by field.
#[derive(Default)]
struct Bytes {
    out: Vec<u8>,
    id_width: usize,
}

impl Bytes {
    fn new(id_width: usize, tag: u16) -> Bytes {
        let mut b = Bytes {
            out: Vec::new(),
            id_width,
        };
        b.u16(tag);
        b
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.out.extend(v.to_le_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.out.extend(v.to_le_bytes());
        self
    }
    fn f64s(&mut self, values: &[f64]) -> &mut Self {
        for v in values {
            self.out.extend(v.to_le_bytes());
        }
        self
    }
    fn id(&mut self, v: i64) -> &mut Self {
        if self.id_width == 8 {
            self.out.extend(v.to_le_bytes());
        } else {
            self.out.extend((v as i32).to_le_bytes());
        }
        self
    }
    fn text(&mut self, s: &str) -> &mut Self {
        self.out.extend(counted(s));
        self
    }
    /// An owned pointer to an object of class `tag` (written later).
    fn pointer(&mut self, tag: u16) -> &mut Self {
        self.u32(u32::MAX).u16(tag)
    }
    fn null(&mut self) -> &mut Self {
        self.u32(0)
    }
    fn flag(&mut self, v: bool) -> &mut Self {
        self.out.push(u8::from(v));
        self
    }
}

/// The classes' tags in a written schema.
struct Tags {
    outline: u16,
    parents: u16,
    header: u16,
    symbol_info: u16,
    family_symbol: u16,
    family: u16,
    instance_info: u16,
    family_instance: u16,
    plane: u16,
    level: u16,
    wall_type: u16,
    wall: u16,
    material: u16,
    material_elem: u16,
    elem_table: u16,
    surface: u16,
    poly_mesh: u16,
    topology: u16,
}

fn schema(id_width: usize) -> (Vec<u8>, Tags) {
    let mut w = SchemaWriter::new();
    let identifier = w.class(
        "Identifier",
        None,
        &[(
            if id_width == 8 { "m_id64" } else { "m_id" },
            word(if id_width == 8 { U64 } else { U32 }),
        )],
    );
    let id = w.class("ElementId", None, &[("m_id", object(identifier))]);
    let eid = || object(id);
    let outline = w.class("Outline", None, &[("m_minmax", matrix(2, 3))]);
    let class_ref = w.class("ClassDefinitionRef", None, &[("m_ref", word(CLASS_TAG))]);
    let view_rules = w.class(
        "RejectedViewRules",
        None,
        &[("m_nVisibleViewFlags", word(U32))],
    );
    let parents = w.class(
        "ElementParents",
        None,
        &[
            ("m_deletion", container_of(id)),
            ("m_hasNonDetermRegenChildren", word(BOOL)),
        ],
    );
    let header = w.class(
        "ElementHeader",
        None,
        &[
            ("m_categroryId", eid()),
            ("m_familyId", eid()),
            ("m_ownerViewId", eid()),
            ("m_designOptionId", eid()),
            ("m_unplacedOwnerId", eid()),
            ("m_viewRules", object(view_rules)),
            ("m_abFlags4Bytes", word(U32)),
            ("m_classDef", object(class_ref)),
            ("m_pBBox", word(OWNED)),
            ("m_parents", word(OWNED)),
        ],
    );
    let element = w.class(
        "Element",
        None,
        &[
            ("m_pParamValueSetDouble", word(OWNED)),
            ("m_docAccess", word(DOC)),
            ("m_id", eid()),
            ("m_assocLevelId", eid()),
            ("m_designOptionId", eid()),
            ("m_locked", word(BOOL)),
        ],
    );
    let symbol_info = w.class("SymbolInfo", None, &[("m_name", word(TEXT))]);
    let symbol = w.class("Symbol", Some(element), &[("m_symbolInfo", word(OWNED))]);
    let family_symbol = w.class(
        "FamilySymbol",
        Some(symbol),
        &[("m_familyId", eid()), ("m_outline", object(outline))],
    );
    let family_base = w.class("FamilyBase", Some(element), &[("m_name", word(TEXT))]);
    let family = w.class("Family", Some(family_base), &[]);
    let trf = w.class(
        "Trf",
        None,
        &[("m_3x3", matrix(3, 3)), ("m_or", array(F64, 3))],
    );
    let inst_info_base = w.class(
        "InstInfoBase",
        None,
        &[("m_Trf", object(trf)), ("m_symbolId", eid())],
    );
    let instance_info = w.class("InstanceInfo", Some(inst_info_base), &[]);
    let instance = w.class(
        "Instance",
        Some(element),
        &[("m_pInstanceInfo", word(OWNED))],
    );
    let family_instance = w.class(
        "FamilyInstance",
        Some(instance),
        &[
            ("m_masterSymbolId", eid()),
            ("m_hostId", eid()),
            ("m_flippedX", word(BOOL)),
        ],
    );
    let plane = w.class("Plane", None, &[("m_origin", array(F64, 3))]);
    let datum = w.class(
        "DatumPlane",
        Some(element),
        &[("m_text", word(TEXT)), ("m_pSurface", word(OWNED))],
    );
    let level = w.class("Level", Some(datum), &[("m_isBuildingStory", word(BOOL))]);
    let wall_type = w.class("BasicWallType", Some(symbol), &[]);
    let vwall = w.class(
        "VWall",
        Some(element),
        &[("m_WallAttributesId", eid()), ("m_upToLevelId", eid())],
    );
    let wall = w.class("SWall", Some(vwall), &[]);
    let material = w.class(
        "Material",
        None,
        &[
            ("m_name", word(TEXT)),
            ("m_transparency", word(U32)),
            ("m_color", word(U32)),
        ],
    );
    let material_elem = w.class(
        "MaterialElem",
        Some(element),
        &[("m_pMaterial", word(OWNED))],
    );
    let elem_rec = w.class(
        "ElemRec",
        None,
        &[("m_id", eid()), ("m_OwningElementId", eid())],
    );
    let elem_table = w.class(
        "ElemTable",
        None,
        &[
            ("m_elemArr", container_of(elem_rec)),
            ("m_bExpandAllOnLoad", word(BOOL)),
        ],
    );
    let topology = w.class(
        "FacetedTopology0",
        None,
        &[
            (
                "m_pointsArr",
                Type {
                    word: 0x500d,
                    count: None,
                    inner: Some(Box::new(array(0x06, 3))),
                    class: None,
                },
            ),
            (
                "m_facetsArr",
                Type {
                    word: 0x500d,
                    count: None,
                    inner: Some(Box::new(array(0x03, 3))),
                    class: None,
                },
            ),
        ],
    );
    let poly_mesh = w.class("GPolyMesh", None, &[("m_pFacetedTopology", word(OWNED))]);
    let surface = w.class("SiteSurface", Some(element), &[("m_pMesh", word(OWNED))]);
    // A field the 2027 schema marks as not stored: skipped by the walker.
    w.class(
        "Unused",
        None,
        &[(
            "m_cache",
            Type {
                word: 0x0002_0001,
                count: None,
                inner: None,
                class: None,
            },
        )],
    );
    let tags = Tags {
        outline,
        parents,
        header,
        symbol_info,
        family_symbol,
        family,
        instance_info,
        family_instance,
        plane,
        level,
        wall_type,
        wall,
        material,
        material_elem,
        elem_table,
        surface,
        poly_mesh,
        topology,
    };
    (w.finish(), tags)
}

/// An element header record: category, class, box.
fn header(t: &Tags, w: usize, category: i64, class: u16, bbox: Option<[f64; 6]>) -> Vec<u8> {
    let mut b = Bytes::new(w, t.header);
    b.id(category).id(-1).id(-1).id(-1).id(-1);
    b.u32(0xffff_ef7f).u32(0).u16(class);
    match bbox {
        Some(_) => b.pointer(t.outline),
        None => b.null(),
    };
    b.pointer(t.parents);
    if let Some(bbox) = bbox {
        b.f64s(&bbox);
    }
    b.u32(0).flag(false);
    b.out
}

/// `Element`'s fields.
fn element(b: &mut Bytes, id: i64, level: i64) {
    b.null().u32(1).id(id).id(level).id(-1).flag(false);
}

fn named_type(
    t: &Tags,
    w: usize,
    class: u16,
    id: i64,
    name: &str,
    family: Option<(i64, [f64; 6])>,
) -> Vec<u8> {
    let mut b = Bytes::new(w, class);
    element(&mut b, id, -1);
    b.pointer(t.symbol_info);
    if let Some((family, outline)) = family {
        b.id(family).f64s(&outline);
    }
    b.text(name);
    b.out
}

fn column(t: &Tags, w: usize, level: i64, origin: [f64; 3]) -> Vec<u8> {
    let mut b = Bytes::new(w, t.family_instance);
    element(&mut b, COLUMN, level);
    b.pointer(t.instance_info);
    b.id(SYMBOL).id(-1).flag(false);
    b.f64s(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
        .f64s(&origin)
        .id(SYMBOL);
    b.out
}

fn level(t: &Tags, w: usize, id: i64, name: &str, elevation: f64) -> Vec<u8> {
    let mut b = Bytes::new(w, t.level);
    element(&mut b, id, -1);
    b.text(name).pointer(t.plane).flag(true);
    b.f64s(&[0.0, 0.0, elevation]);
    b.out
}

/// A surface whose record holds a saved mesh: a pyramid of 4 triangles.
fn surface(t: &Tags, w: usize) -> Vec<u8> {
    let mut b = Bytes::new(w, t.surface);
    element(&mut b, SURFACE, LEVEL_GROUND);
    b.pointer(t.poly_mesh);
    b.pointer(t.topology);
    let points: [[f32; 3]; 5] = [
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [4.0, 4.0, 0.0],
        [0.0, 4.0, 0.0],
        [2.0, 2.0, 1.0],
    ];
    b.u32(points.len() as u32);
    for p in points {
        for v in p {
            b.out.extend(v.to_le_bytes());
        }
    }
    let faces: [[u16; 3]; 4] = [[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]];
    b.u32(faces.len() as u32);
    for f in faces {
        for v in f {
            b.u16(v);
        }
    }
    b.out
}

fn compound(streams: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut w = mitcad_cfb::Writer::new();
    w.version(4);
    for (path, data) in streams {
        w.stream(path, data);
    }
    w.finish()
}

fn global(prefix: u64, inflated: &[u8]) -> Vec<u8> {
    let mut out = prefix.to_le_bytes().to_vec();
    out.extend(gzip(inflated));
    add_pages(&out)
}

fn elem_table(t: &Tags, w: usize, ids: &[i64]) -> Vec<u8> {
    let mut b = Bytes::new(w, t.elem_table);
    b.u32(ids.len() as u32);
    for id in ids {
        b.id(*id).id(-1);
    }
    b.flag(false);
    // The table's trailing word, not read.
    b.u32(0);
    b.out
}

/// The test project (release 2024, ids of 8 bytes).
pub fn project() -> Vec<u8> {
    let w = 8;
    let (schema, t) = schema(w);
    let column_box = [19.0, 29.0, 10.0, 21.0, 31.0, 19.0];
    let wall_box = [0.0, 0.0, 0.0, 10.0, 0.5, 10.0];
    let mut material = Bytes::new(w, t.material_elem);
    element(&mut material, MATERIAL, -1);
    material
        .pointer(t.material)
        .text("Concrete")
        .u32(0)
        .u32(0x0020_4080);
    let mut wall = Bytes::new(w, t.wall);
    element(&mut wall, WALL, LEVEL_GROUND);
    wall.id(WALL_TYPE).id(LEVEL_UPPER);
    let current = vec![
        (
            LEVEL_GROUND,
            header(&t, w, OST_LEVELS, t.level, None),
            level(&t, w, LEVEL_GROUND, "Ground", 0.0),
        ),
        (
            LEVEL_UPPER,
            header(&t, w, OST_LEVELS, t.level, None),
            level(&t, w, LEVEL_UPPER, "Upper", 10.0),
        ),
        (FAMILY, header(&t, w, OST_COLUMNS, t.family, None), {
            let mut b = Bytes::new(w, t.family);
            element(&mut b, FAMILY, -1);
            b.text("Square Column");
            b.out
        }),
        (
            SYMBOL,
            header(
                &t,
                w,
                OST_COLUMNS,
                t.family_symbol,
                Some([-1.0, -1.0, 0.0, 1.0, 1.0, 9.0]),
            ),
            named_type(
                &t,
                w,
                t.family_symbol,
                SYMBOL,
                "24 x 24",
                Some((FAMILY, [-1.0, -1.0, 0.0, 1.0, 1.0, 9.0])),
            ),
        ),
        (
            WALL_TYPE,
            header(&t, w, OST_WALLS, t.wall_type, None),
            named_type(&t, w, t.wall_type, WALL_TYPE, "Generic 8", None),
        ),
        (
            MATERIAL,
            header(&t, w, -2_000_700, t.material_elem, None),
            material.out,
        ),
        (
            COLUMN,
            header(&t, w, OST_COLUMNS, t.family_instance, Some(column_box)),
            column(&t, w, LEVEL_UPPER, [20.0, 30.0, 10.0]),
        ),
        (
            WALL,
            header(&t, w, OST_WALLS, t.wall, Some(wall_box)),
            wall.out,
        ),
        (
            SURFACE,
            header(
                &t,
                w,
                OST_TOPOGRAPHY,
                t.surface,
                Some([0.0, 0.0, 0.0, 4.0, 4.0, 1.0]),
            ),
            surface(&t, w),
        ),
    ];
    let older = vec![
        (
            COLUMN,
            header(&t, w, OST_COLUMNS, t.family_instance, Some([0.0; 6])),
            column(&t, w, LEVEL_GROUND, [0.0, 0.0, 0.0]),
        ),
        (UNDECLARED, header(&t, w, OST_WALLS, t.wall, None), {
            let mut b = Bytes::new(w, t.wall);
            element(&mut b, UNDECLARED, -1);
            b.id(-1).id(-1);
            b.out
        }),
    ];
    let mut ids = DECLARED.to_vec();
    ids.sort_unstable();
    compound(&[
        ("BasicFileInfo", basic_file_info(2024, "20230509_0315(x64)")),
        ("Formats/Latest", add_pages(&gzip(&schema))),
        ("Global/ElemTable", global(0, &elem_table(&t, w, &DECLARED))),
        ("Partitions/5", add_pages(&write_partition(9, w, &older))),
        ("Partitions/7", add_pages(&write_partition(9, w, &current))),
        (crate::PREVIEW_STREAM, preview()),
        ("TransmissionData", transmission_data()),
    ])
}

/// The test family (release 2020, ids of 4 bytes): its family element and
/// one type.
pub fn family() -> Vec<u8> {
    let w = 4;
    let (schema, t) = schema(w);
    let records = vec![
        (FAMILY, header(&t, w, OST_COLUMNS, t.family, None), {
            let mut b = Bytes::new(w, t.family);
            element(&mut b, FAMILY, -1);
            b.text("Test Family");
            b.out
        }),
        (
            SYMBOL,
            header(
                &t,
                w,
                OST_COLUMNS,
                t.family_symbol,
                Some([-1.0, -1.0, 0.0, 1.0, 1.0, 9.0]),
            ),
            named_type(
                &t,
                w,
                t.family_symbol,
                SYMBOL,
                "Type 1",
                Some((FAMILY, [-1.0, -1.0, 0.0, 1.0, 1.0, 9.0])),
            ),
        ),
    ];
    let mut info = basic_file_info(2020, "20190207_1515(x64)");
    info[0] = 13;
    compound(&[
        ("BasicFileInfo", info),
        ("Formats/Latest", add_pages(&gzip(&schema))),
        (
            "Global/ElemTable",
            global(0, &elem_table(&t, w, &[0, FAMILY, SYMBOL])),
        ),
        ("Partitions/63", add_pages(&write_partition(9, w, &records))),
        (crate::PREVIEW_STREAM, preview()),
        ("PartAtom", PART_ATOM.as_bytes().to_vec()),
        ("TransmissionData", transmission_data()),
    ])
}

/// A building project for the phase-2 readers and the import's features
/// (mitcad#275): two levels, a wall type of one layer with a material, two
/// walls meeting at a corner (the first listed at the joint runs on), a
/// floor of a rectangle with a hole, a grid, a link to `linked.rvt` and a
/// chair (a family instance known by its box only, mitcad#333).
pub mod building {
    use super::*;

    pub const LEVEL_GROUND: i64 = 10;
    pub const LEVEL_UPPER: i64 = 11;
    pub const WALL_TYPE: i64 = 20;
    pub const FLOOR_TYPE: i64 = 21;
    pub const MATERIAL: i64 = 22;
    pub const WALL_A: i64 = 30;
    pub const WALL_B: i64 = 31;
    pub const FLOOR: i64 = 32;
    pub const GRID: i64 = 33;
    pub const LINK_TYPE: i64 = 40;
    pub const LINK: i64 = 41;
    /// A chair: a family instance (furniture) without geometry, its box
    /// its only shape.
    pub const CHAIR: i64 = 42;
    /// The element table's ids.
    pub const DECLARED: [i64; 13] = [
        0,
        LEVEL_GROUND,
        LEVEL_UPPER,
        WALL_TYPE,
        FLOOR_TYPE,
        MATERIAL,
        WALL_A,
        WALL_B,
        FLOOR,
        GRID,
        LINK_TYPE,
        LINK,
        CHAIR,
    ];
    /// The walls' type width and height (feet).
    pub const WIDTH: f64 = 0.5;
    pub const HEIGHT: f64 = 10.0;
    /// The linked file's name.
    pub const LINKED: &str = "linked.rvt";

    /// The classes' tags.
    struct Tags {
        outline: u16,
        parents: u16,
        header: u16,
        level: u16,
        plane: u16,
        param_set: u16,
        symbol_info: u16,
        wall_type: u16,
        floor_type: u16,
        structure: u16,
        material: u16,
        material_elem: u16,
        line: u16,
        driver: u16,
        wall: u16,
        curve_loop: u16,
        helper: u16,
        cells: u16,
        floor: u16,
        grid: u16,
        instance_info: u16,
        link: u16,
        link_type: u16,
        instance: u16,
        elem_table: u16,
    }

    /// A container of owned pointers.
    const POINTERS: u32 = 0x510e;

    fn schema() -> (Vec<u8>, Tags) {
        let mut w = SchemaWriter::new();
        let identifier = w.class("Identifier", None, &[("m_id64", word(U64))]);
        let id = w.class("ElementId", None, &[("m_id", object(identifier))]);
        let eid = || object(id);
        let outline = w.class("Outline", None, &[("m_minmax", matrix(2, 3))]);
        let class_ref = w.class("ClassDefinitionRef", None, &[("m_ref", word(CLASS_TAG))]);
        let view_rules = w.class(
            "RejectedViewRules",
            None,
            &[("m_nVisibleViewFlags", word(U32))],
        );
        let parents = w.class("ElementParents", None, &[("m_deletion", container_of(id))]);
        let header = w.class(
            "ElementHeader",
            None,
            &[
                ("m_categroryId", eid()),
                ("m_familyId", eid()),
                ("m_ownerViewId", eid()),
                ("m_designOptionId", eid()),
                ("m_unplacedOwnerId", eid()),
                ("m_viewRules", object(view_rules)),
                ("m_abFlags4Bytes", word(U32)),
                ("m_classDef", object(class_ref)),
                ("m_pBBox", word(OWNED)),
                ("m_parents", word(OWNED)),
            ],
        );
        let param = w.class(
            "ParamValueDouble",
            None,
            &[("m_paramId", eid()), ("m_value", word(F64))],
        );
        let param_set = w.class(
            "ParamValueSetDouble",
            None,
            &[("m_paramSet", container_of(param))],
        );
        let element = w.class(
            "Element",
            None,
            &[
                ("m_pParamValueSetDouble", word(OWNED)),
                ("m_id", eid()),
                ("m_assocLevelId", eid()),
            ],
        );
        let envelope = w.class("Envelope", None, &[("m_corners", matrix(2, 2))]);
        let plane = w.class(
            "Plane",
            None,
            &[
                ("m_origin", array(F64, 3)),
                ("m_xVec", array(F64, 3)),
                ("m_yVec", array(F64, 3)),
                ("m_Envelope", object(envelope)),
            ],
        );
        let datum = w.class(
            "DatumPlane",
            Some(element),
            &[("m_text", word(TEXT)), ("m_pSurface", word(OWNED))],
        );
        let level = w.class("Level", Some(datum), &[("m_isBuildingStory", word(BOOL))]);
        let symbol_info = w.class("SymbolInfo", None, &[("m_name", word(TEXT))]);
        let symbol = w.class("Symbol", Some(element), &[("m_symbolInfo", word(OWNED))]);
        let layer = w.class(
            "CompoundStructureLayer",
            None,
            &[
                ("m_layerWidth", word(F64)),
                ("m_materialId", eid()),
                ("m_layerFunction", word(U32)),
            ],
        );
        let structure = w.class(
            "CompoundStructure",
            None,
            &[("m_layers", container_of(layer))],
        );
        let wall_type = w.class(
            "BasicWallType",
            Some(symbol),
            &[("m_pCompoundStructure", word(OWNED))],
        );
        let floor_type = w.class(
            "FloorAttributes",
            Some(symbol),
            &[("m_pCompoundStructure", word(OWNED))],
        );
        let material = w.class(
            "Material",
            None,
            &[
                ("m_name", word(TEXT)),
                ("m_transparency", word(U32)),
                ("m_color", word(U32)),
            ],
        );
        let material_elem = w.class(
            "MaterialElem",
            Some(element),
            &[("m_pMaterial", word(OWNED))],
        );
        let line = w.class(
            "GLine",
            None,
            &[
                ("m_origin", array(F64, 3)),
                ("m_dirVec", array(F64, 3)),
                ("m_endParams", array(F64, 2)),
            ],
        );
        let end_info = w.class(
            "EndInfo",
            None,
            &[("m_elemId", eid()), ("m_joinedEnd", word(U32))],
        );
        let join_info = w.class(
            "CDJoinInfo",
            None,
            &[
                ("m_info", container_of(end_info)),
                ("m_wallJoinType", word(U32)),
            ],
        );
        let pair = w.class(
            "IndexJoinPair",
            None,
            &[("m_JoinInfo", object(join_info)), ("m_index", word(U32))],
        );
        let driver = w.class(
            "VWallDriver",
            None,
            &[
                ("m_controlJoinsSet", container_of(pair)),
                ("m_pCrv", word(OWNED)),
            ],
        );
        let vwall = w.class(
            "VWall",
            Some(element),
            &[
                ("m_WallAttributesId", eid()),
                ("m_upToLevelId", eid()),
                ("m_pCurveDriver", word(OWNED)),
            ],
        );
        let wall = w.class("SWall", Some(vwall), &[]);
        let curve_loop = w.class("CurveLoop", None, &[("m_curves", word(POINTERS))]);
        let helper = w.class(
            "FloorExtrusionHelper",
            None,
            &[("m_pCurveLoops", word(POINTERS))],
        );
        let cells = w.class("CellList", None, &[("m_cells", word(POINTERS))]);
        let floor = w.class(
            "Floor",
            Some(element),
            &[
                ("m_floorAttributesId", eid()),
                ("m_top", word(F64)),
                ("m_bottom", word(F64)),
                ("m_cellList", word(OWNED)),
            ],
        );
        let grid = w.class(
            "Grid",
            Some(element),
            &[("m_text", word(TEXT)), ("m_pSurface", word(OWNED))],
        );
        let trf = w.class(
            "Trf",
            None,
            &[("m_3x3", matrix(3, 3)), ("m_or", array(F64, 3))],
        );
        let instance_info = w.class(
            "InstanceInfo",
            None,
            &[("m_Trf", object(trf)), ("m_symbolId", eid())],
        );
        let link = w.class(
            "RvtLinkInstance",
            Some(element),
            &[("m_name", word(TEXT)), ("m_pInstanceInfo", word(OWNED))],
        );
        let link_type = w.class("RvtLinkSymbol", Some(symbol), &[]);
        let instance = w.class(
            "FamilyInstance",
            Some(element),
            &[("m_pInstanceInfo", word(OWNED))],
        );
        let elem_rec = w.class(
            "ElemRec",
            None,
            &[("m_id", eid()), ("m_OwningElementId", eid())],
        );
        let elem_table = w.class(
            "ElemTable",
            None,
            &[
                ("m_elemArr", container_of(elem_rec)),
                ("m_bExpandAllOnLoad", word(BOOL)),
            ],
        );
        let tags = Tags {
            outline,
            parents,
            header,
            level,
            plane,
            param_set,
            symbol_info,
            wall_type,
            floor_type,
            structure,
            material,
            material_elem,
            line,
            driver,
            wall,
            curve_loop,
            helper,
            cells,
            floor,
            grid,
            instance_info,
            link,
            link_type,
            instance,
            elem_table,
        };
        (w.finish(), tags)
    }

    fn header(t: &Tags, category: i64, class: u16, bbox: Option<[f64; 6]>) -> Vec<u8> {
        let mut b = Bytes::new(8, t.header);
        b.id(category).id(-1).id(-1).id(-1).id(-1);
        b.u32(0xffff_ef7f).u32(0).u16(class);
        match bbox {
            Some(_) => b.pointer(t.outline),
            None => b.null(),
        };
        b.pointer(t.parents);
        if let Some(bbox) = bbox {
            b.f64s(&bbox);
        }
        b.u32(0);
        b.out
    }

    /// `Element`'s fields: a parameter set (written first after the
    /// record's fields) or none.
    fn element(b: &mut Bytes, t: &Tags, params: bool, id: i64, level: i64) {
        if params {
            b.pointer(t.param_set);
        } else {
            b.null();
        }
        b.id(id).id(level);
    }

    /// A type with a compound structure of one layer.
    fn layered_type(t: &Tags, class: u16, id: i64, name: &str, width: f64) -> Vec<u8> {
        let mut b = Bytes::new(8, class);
        element(&mut b, t, false, id, -1);
        b.pointer(t.symbol_info).pointer(t.structure);
        b.text(name);
        b.u32(1).f64s(&[width]).id(MATERIAL).u32(1);
        b.out
    }

    /// A wall from `start` to `end` (feet, at height 0), joined at its
    /// start with `corner` (elements and their ends, in order).
    fn wall(t: &Tags, id: i64, start: [f64; 3], end: [f64; 3], corner: &[(i64, u32)]) -> Vec<u8> {
        let mut b = Bytes::new(8, t.wall);
        element(&mut b, t, true, id, LEVEL_GROUND);
        b.id(WALL_TYPE).id(LEVEL_UPPER).pointer(t.driver);
        // The parameter set: the height.
        b.u32(1).id(-1_001_101).f64s(&[HEIGHT]);
        // The driver: its join at the start, then its curve.
        b.u32(1).u32(corner.len() as u32);
        for (e, joined) in corner {
            b.id(*e).u32(*joined);
        }
        b.u32(0).u32(0);
        b.pointer(t.line);
        let d = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        b.f64s(&start)
            .f64s(&[d[0] / l, d[1] / l, d[2] / l])
            .f64s(&[0.0, l]);
        b.out
    }

    /// A line record's fields from `a` to `b` (at height 0).
    fn line(b: &mut Bytes, a: [f64; 2], c: [f64; 2]) {
        let d = [c[0] - a[0], c[1] - a[1]];
        let l = d[0].hypot(d[1]);
        b.f64s(&[a[0], a[1], 0.0, d[0] / l, d[1] / l, 0.0])
            .f64s(&[0.0, l]);
    }

    /// The building project (release 2024, ids of 8 bytes).
    pub fn project() -> Vec<u8> {
        let (schema, t) = schema();
        let level = |id, name: &str, z: f64| {
            let mut b = Bytes::new(8, t.level);
            element(&mut b, &t, false, id, -1);
            b.text(name).pointer(t.plane).flag(true);
            b.f64s(&[0.0, 0.0, z, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0])
                .f64s(&[0.0, 0.0, 100.0, 100.0]);
            b.out
        };
        let mut material = Bytes::new(8, t.material_elem);
        element(&mut material, &t, false, MATERIAL, -1);
        material
            .pointer(t.material)
            .text("Brick")
            .u32(0)
            .u32(0x0030_60c0);
        // The corner at (0, 0): wall A along x, listed first, runs on to
        // wall B's far face.
        let corner = [(WALL_A, 0), (WALL_B, 0)];
        let wall_a = wall(&t, WALL_A, [0.0, 0.0, 0.0], [20.0, 0.0, 0.0], &corner);
        let wall_b = wall(&t, WALL_B, [0.0, 0.0, 0.0], [0.0, 12.0, 0.0], &corner);
        // The floor: a 20 × 12 rectangle with a 2 × 2 hole, its top at 0.
        let mut floor = Bytes::new(8, t.floor);
        element(&mut floor, &t, false, FLOOR, LEVEL_GROUND);
        floor.id(FLOOR_TYPE).f64s(&[0.0, -0.5]).pointer(t.cells);
        floor.u32(1).pointer(t.helper);
        floor.u32(2).pointer(t.curve_loop).pointer(t.curve_loop);
        let rect = |x0: f64, y0: f64, w: f64, h: f64| {
            let c = [[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]];
            (0..4).map(|i| (c[i], c[(i + 1) % 4])).collect::<Vec<_>>()
        };
        for _ in 0..2 {
            floor.u32(4);
            for _ in 0..4 {
                floor.pointer(t.line);
            }
        }
        for (a, c) in rect(0.0, 0.0, 20.0, 12.0)
            .into_iter()
            .chain(rect(5.0, 5.0, 2.0, 2.0))
        {
            line(&mut floor, a, c);
        }
        let mut grid = Bytes::new(8, t.grid);
        element(&mut grid, &t, false, GRID, -1);
        grid.text("A").pointer(t.plane);
        grid.f64s(&[-5.0, -2.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0])
            .f64s(&[0.0, 0.0, 30.0, 20.0]);
        let mut link_type = Bytes::new(8, t.link_type);
        element(&mut link_type, &t, false, LINK_TYPE, -1);
        link_type.pointer(t.symbol_info).text(LINKED);
        let mut link = Bytes::new(8, t.link);
        element(&mut link, &t, false, LINK, -1);
        link.text("1").pointer(t.instance_info);
        link.f64s(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
            .f64s(&[100.0, 0.0, 0.0])
            .id(LINK_TYPE);
        let mut chair = Bytes::new(8, t.instance);
        element(&mut chair, &t, false, CHAIR, LEVEL_GROUND);
        chair.pointer(t.instance_info);
        chair
            .f64s(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
            .f64s(&[3.0, 3.0, 0.0])
            .id(-1);
        let floors = crate::categories::OST_FLOORS;
        let records = vec![
            (
                LEVEL_GROUND,
                header(&t, OST_LEVELS, t.level, None),
                level(LEVEL_GROUND, "Ground", 0.0),
            ),
            (
                LEVEL_UPPER,
                header(&t, OST_LEVELS, t.level, None),
                level(LEVEL_UPPER, "Upper", 10.0),
            ),
            (
                WALL_TYPE,
                header(&t, OST_WALLS, t.wall_type, None),
                layered_type(&t, t.wall_type, WALL_TYPE, "Brick 6", WIDTH),
            ),
            (
                FLOOR_TYPE,
                header(&t, floors, t.floor_type, None),
                layered_type(&t, t.floor_type, FLOOR_TYPE, "Slab 6", 0.5),
            ),
            (
                MATERIAL,
                header(&t, -2_000_700, t.material_elem, None),
                material.out,
            ),
            (
                WALL_A,
                header(
                    &t,
                    OST_WALLS,
                    t.wall,
                    Some([-0.25, -0.25, 0.0, 20.0, 0.25, 10.0]),
                ),
                wall_a,
            ),
            (
                WALL_B,
                header(
                    &t,
                    OST_WALLS,
                    t.wall,
                    Some([-0.25, 0.0, 0.0, 0.25, 12.0, 10.0]),
                ),
                wall_b,
            ),
            (
                FLOOR,
                header(&t, floors, t.floor, Some([0.0, 0.0, -0.5, 20.0, 12.0, 0.0])),
                floor.out,
            ),
            (GRID, header(&t, -2_000_220, t.grid, None), grid.out),
            (
                LINK_TYPE,
                header(&t, -2_001_352, t.link_type, None),
                link_type.out,
            ),
            (LINK, header(&t, -2_001_352, t.link, None), link.out),
            (
                CHAIR,
                header(
                    &t,
                    -2_000_080,
                    t.instance,
                    Some([2.0, 2.0, 0.0, 4.0, 4.0, 3.0]),
                ),
                chair.out,
            ),
        ];
        let mut table = Bytes::new(8, t.elem_table);
        table.u32(DECLARED.len() as u32);
        for id in DECLARED {
            table.id(id).id(-1);
        }
        table.flag(false).u32(0);
        compound(&[
            ("BasicFileInfo", basic_file_info(2024, "20230509_0315(x64)")),
            ("Formats/Latest", add_pages(&gzip(&schema))),
            ("Global/ElemTable", global(0, &table.out)),
            ("Partitions/3", add_pages(&write_partition(9, 8, &records))),
            ("TransmissionData", transmission_data()),
        ])
    }
}
