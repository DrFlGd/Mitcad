// SPDX-License-Identifier: MIT
//! A writer of small drawings for the tests (here, in the import's tests
//! and `mitcad-cli import-dwg --write-test`): the structures the reader
//! reads, written as the specification describes them, for every
//! version. The bodies' B-rep records come from Mitcad's own ASM writer
//! (`mitcad_asm::testdata`).
//!
//! [`test_drawing`] is a small truss: a block `Brace` (a cube solid), a
//! block `Segment` (a cylinder chord and two inserts of `Brace`), two
//! inserts of `Segment` in model space, an array insert of `Brace`, an
//! insert of `Brace` scaled by 2, two 3D faces, a polyface mesh (a
//! tetrahedron), a subdivision mesh (a cube) and a line.

use std::collections::BTreeMap;

use crate::Version;
use crate::bits::{BitWriter, write_modular_char, write_modular_short};
use crate::classes::Class;
use crate::container;
use crate::header::HeaderVars;
use crate::objects::types;

/// The maintenance release the test files claim.
pub use crate::header::MAINTENANCE;
/// The objects one after another (in the object map's module).
pub use crate::objmap::object_stream;

/// What an entity of the test drawing is.
#[derive(Clone, Debug)]
pub enum TestEntity {
    /// A B-rep entity with an ASM binary file; `text` keeps it in the text
    /// form up to R2010.
    Solid {
        asm: Vec<u8>,
        text: bool,
    },
    Face3d([[f64; 3]; 4]),
    /// A polyface mesh: vertices, faces (1-based indices).
    Pface {
        vertices: Vec<[f64; 3]>,
        faces: Vec<[i16; 4]>,
    },
    Insert {
        block: u64,
        point: [f64; 3],
        scale: [f64; 3],
        rotation: f64,
        array: Option<(i16, i16, f64, f64)>,
    },
    Mesh {
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
    },
    Line([f64; 3], [f64; 3]),
}

/// An entity with its handle, owner (a block header, or None for model
/// space), layer and colour index.
#[derive(Clone, Debug)]
pub struct Placed {
    pub handle: u64,
    pub owner: Option<u64>,
    pub layer: u64,
    pub color: i16,
    pub entity: TestEntity,
}

/// A test drawing.
#[derive(Clone, Debug)]
pub struct TestDrawing {
    pub header: HeaderVars,
    /// Layer handle, name, colour index.
    pub layers: Vec<(u64, String, i16)>,
    /// Block header handle, name, base point.
    pub blocks: Vec<(u64, String, [f64; 3])>,
    pub entities: Vec<Placed>,
}

pub const MODEL_SPACE: u64 = 0x1f;
pub const BRACE: u64 = 0x20;
pub const SEGMENT: u64 = 0x21;
pub const LAYER_0: u64 = 0x10;
pub const LAYER_TUBES: u64 = 0x11;

/// The test drawing described in the module docs.
pub fn test_drawing() -> TestDrawing {
    let cube = mitcad_asm::testdata::cube_blob();
    let cylinder = mitcad_asm::testdata::cylinder_blob();
    let rigid = |block, point, rotation| TestEntity::Insert {
        block,
        point,
        scale: [1.0; 3],
        rotation,
        array: None,
    };
    let e = |handle, owner, layer, color, entity| Placed {
        handle,
        owner,
        layer,
        color,
        entity,
    };
    let s = 1.0 / 3f64.sqrt();
    TestDrawing {
        header: HeaderVars {
            insunits: Some(4),
            // The box of the content: the chords' radius at the origin, the
            // subdivision mesh's far side, the array's last row, the 3D
            // faces' height.
            extmin: Some([-1.0, -1.0, 0.0]),
            extmax: Some([81.732, 45.0, 31.0]),
            handseed: Some(0x100),
        },
        layers: vec![(LAYER_0, "0".into(), 7), (LAYER_TUBES, "Tubes".into(), 1)],
        blocks: vec![
            (MODEL_SPACE, "*Model_Space".into(), [0.0; 3]),
            (BRACE, "Brace".into(), [0.0; 3]),
            (SEGMENT, "Segment".into(), [0.0; 3]),
        ],
        entities: vec![
            e(
                0x30,
                Some(BRACE),
                LAYER_TUBES,
                256,
                TestEntity::Solid {
                    asm: cube,
                    text: true,
                },
            ),
            e(
                0x31,
                Some(SEGMENT),
                LAYER_TUBES,
                3,
                TestEntity::Solid {
                    asm: cylinder,
                    text: false,
                },
            ),
            e(
                0x32,
                Some(SEGMENT),
                LAYER_0,
                256,
                rigid(BRACE, [5.0, 0.0, 0.0], 0.0),
            ),
            e(
                0x33,
                Some(SEGMENT),
                LAYER_0,
                256,
                rigid(BRACE, [10.0, 0.0, 0.0], std::f64::consts::FRAC_PI_2),
            ),
            e(
                0x40,
                None,
                LAYER_0,
                256,
                rigid(SEGMENT, [0.0, 0.0, 0.0], 0.0),
            ),
            e(
                0x41,
                None,
                LAYER_0,
                256,
                rigid(SEGMENT, [0.0, 20.0, 0.0], std::f64::consts::FRAC_PI_2),
            ),
            e(
                0x42,
                None,
                LAYER_0,
                5,
                TestEntity::Face3d([
                    [0.0, 0.0, 30.0],
                    [1.0, 0.0, 30.0],
                    [1.0, 1.0, 30.0],
                    [0.0, 1.0, 30.0],
                ]),
            ),
            e(
                0x43,
                None,
                LAYER_0,
                5,
                TestEntity::Face3d([
                    [0.0, 0.0, 31.0],
                    [1.0, 0.0, 31.0],
                    [1.0, 1.0, 31.0],
                    [1.0, 1.0, 31.0],
                ]),
            ),
            e(
                0x44,
                None,
                LAYER_TUBES,
                256,
                TestEntity::Pface {
                    vertices: vec![
                        [50.0, 0.0, 0.0],
                        [51.0, 0.0, 0.0],
                        [50.0, 1.0, 0.0],
                        [50.0, 0.0, 1.0],
                    ],
                    faces: vec![[1, 3, 2, 0], [1, 2, 4, 0], [2, 3, 4, 0], [3, 1, 4, 0]],
                },
            ),
            e(
                0x60,
                None,
                LAYER_0,
                256,
                TestEntity::Insert {
                    block: BRACE,
                    point: [0.0, 40.0, 0.0],
                    scale: [1.0; 3],
                    rotation: 0.0,
                    array: Some((3, 2, 2.0, 4.0)),
                },
            ),
            e(
                0x61,
                None,
                LAYER_0,
                256,
                TestEntity::Insert {
                    block: BRACE,
                    point: [70.0, 0.0, 0.0],
                    scale: [2.0; 3],
                    rotation: 0.0,
                    array: None,
                },
            ),
            e(
                0x62,
                None,
                LAYER_0,
                256,
                TestEntity::Mesh {
                    vertices: (0..8)
                        .map(|i| {
                            [
                                80.0 + f64::from(i & 1) * s * 3.0,
                                f64::from(i >> 1 & 1),
                                f64::from(i >> 2 & 1),
                            ]
                        })
                        .collect(),
                    faces: vec![
                        vec![0, 2, 3, 1],
                        vec![4, 5, 7, 6],
                        vec![0, 1, 5, 4],
                        vec![2, 6, 7, 3],
                        vec![0, 4, 6, 2],
                        vec![1, 3, 7, 5],
                    ],
                },
            ),
            e(
                0x63,
                None,
                LAYER_0,
                256,
                TestEntity::Line([0.0; 3], [1.0, 1.0, 1.0]),
            ),
        ],
    }
}

/// The test drawing with a case of an import test added (`mitcad-cli
/// import-dwg --write-test --case`), or `None` for an unknown case:
///
/// - `empty-solid`: a solid in model space (handle 64) whose B-rep record
///   has faces but no point positions, so that none of its faces can be
///   built (mitcad#589: such a body ended the whole import).
pub fn test_case(name: &str) -> Option<TestDrawing> {
    let mut d = test_drawing();
    match name {
        "" => {}
        "empty-solid" => d.entities.push(Placed {
            handle: 0x64,
            owner: None,
            layer: LAYER_0,
            color: 256,
            entity: TestEntity::Solid {
                asm: mitcad_asm::testdata::cube_without_points_blob(),
                text: false,
            },
        }),
        _ => return None,
    }
    Some(d)
}

/// The classes of the test drawing.
pub fn test_classes() -> Vec<Class> {
    vec![Class {
        number: 500,
        app_name: "ObjectDBX Classes".into(),
        cpp_name: "AcDbSubDMesh".into(),
        dxf_name: "MESH".into(),
        entity: true,
    }]
}

/// One object's streams while it is written.
struct ObjectWriter {
    v: Version,
    handle: u64,
    entity: bool,
    type_code: u16,
    data: BitWriter,
    strings: BitWriter,
    handles: BitWriter,
}

impl ObjectWriter {
    fn new(v: Version, type_code: u16, handle: u64, entity: bool) -> Self {
        ObjectWriter {
            v,
            handle,
            entity,
            type_code,
            data: BitWriter::new(v),
            strings: BitWriter::new(v),
            handles: BitWriter::new(v),
        }
    }

    fn text(&mut self, s: &str) {
        if self.v >= Version::R2007 {
            self.strings.text16(s);
        } else {
            self.data.text(s);
        }
    }

    fn h(&mut self, code: u8, handle: u64) {
        self.handles.handle(code, handle);
    }

    /// The object's bytes: size, data and CRC.
    fn finish(self, has_ds: bool) -> Vec<u8> {
        let _ = has_ds;
        let v = self.v;
        let mut head = BitWriter::new(v);
        head.ot(self.type_code);
        let mut after = BitWriter::new(v);
        after.handle(0, self.handle).bs(0);
        if self.entity {
            after.bit(false);
        }
        let mut rest = self.data;
        if v >= Version::R2007 {
            if self.strings.len > 0 {
                rest.append(&self.strings);
                rest.rs(self.strings.len as u16).bit(true);
            } else {
                rest.bit(false);
            }
        }
        let rl = v >= Version::R2000 && v <= Version::R2007 || v <= Version::R14;
        let endbit = head.len + after.len + rest.len + if rl { 32 } else { 0 };
        let mut all = head;
        if v >= Version::R2000 && v <= Version::R2007 {
            all.rl(endbit as u32);
            all.append(&after);
        } else if v <= Version::R14 {
            all.append(&after);
            all.rl(endbit as u32);
        } else {
            all.append(&after);
        }
        all.append(&rest);
        all.append(&self.handles);
        all.align();
        let mut out = Vec::new();
        write_modular_short(&mut out, all.data.len() as u64);
        if v >= Version::R2010 {
            write_modular_char(&mut out, (all.len - endbit) as u64);
        }
        out.extend_from_slice(&all.data);
        let crc = crate::crc::crc16(0xC0C1, &out);
        out.extend_from_slice(&crc.to_le_bytes());
        out
    }
}

/// A table entry's flags: the 64-flag (set: referenced), before R2007 the
/// xref index plus one, the xref dependency.
fn table_entry_flags(w: &mut ObjectWriter) {
    w.data.bit(true);
    if w.v < Version::R2007 {
        w.data.bs(0);
    }
    w.data.bit(false);
}

/// The common data of a non-entity object.
fn object_common(w: &mut ObjectWriter, owner: u64) {
    w.data.bl(0);
    if w.v >= Version::R2004 {
        w.data.bit(true);
    }
    if w.v >= Version::R2013 {
        w.data.bit(false);
    }
    w.h(4, owner);
    if w.v < Version::R2004 {
        w.h(3, 0);
    }
}

/// The common data of an entity, with the layer and the colour.
fn entity_common(w: &mut ObjectWriter, owner: Option<u64>, layer: u64, color: i16, has_ds: bool) {
    let v = w.v;
    w.data.bb(if owner.is_some() { 0 } else { 2 });
    w.data.bl(0);
    if v >= Version::R2004 {
        w.data.bit(true);
    }
    if v >= Version::R2013 {
        w.data.bit(has_ds);
    }
    if v <= Version::R14 {
        w.data.bit(true);
    }
    // No links (up to R2000; later versions have no bit for them).
    if v <= Version::R2000 {
        w.data.bit(true);
    }
    w.data.bs(color);
    w.data.bd(1.0);
    if v >= Version::R2000 {
        w.data.bb(0).bb(0);
    }
    if v >= Version::R2007 {
        w.data.bb(0).rc(0);
    }
    if v >= Version::R2010 {
        w.data.bit(false).bit(false).bit(false);
    }
    w.data.bs(0);
    if v >= Version::R2000 {
        w.data.rc(0x1d);
    }
    if let Some(o) = owner {
        w.h(4, o);
    }
    if v < Version::R2004 {
        w.h(3, 0);
    }
    if v <= Version::R14 {
        w.h(5, layer);
    }
    if v >= Version::R2000 {
        w.h(5, layer);
    }
}

/// The objects of a drawing: handle to bytes.
/// The objects by handle, and the B-rep records for the data storage.
type Written = (BTreeMap<u64, Vec<u8>>, Vec<(u64, Vec<u8>)>);

fn objects(d: &TestDrawing, v: Version) -> Written {
    let mut out = BTreeMap::new();
    let mut stored = Vec::new();
    // Block control and layer control.
    let mut w = ObjectWriter::new(v, types::BLOCK_CONTROL, 1, false);
    object_common(&mut w, 0);
    w.data.bl(d.blocks.len() as i32 - 1);
    out.insert(1, w.finish(false));
    let mut w = ObjectWriter::new(v, types::LAYER_CONTROL, 2, false);
    object_common(&mut w, 0);
    w.data.bl(d.layers.len() as i32);
    out.insert(2, w.finish(false));
    for (handle, name, color) in &d.layers {
        let mut w = ObjectWriter::new(v, types::LAYER, *handle, false);
        object_common(&mut w, 2);
        w.text(name);
        table_entry_flags(&mut w);
        if v <= Version::R14 {
            w.data.bit(false).bit(true).bit(false).bit(false);
        } else {
            w.data.bs(2);
        }
        w.data.bs(*color);
        if v >= Version::R2004 {
            w.data.bl(0).rc(0);
        }
        out.insert(*handle, w.finish(false));
    }
    for (handle, name, base) in &d.blocks {
        let mut w = ObjectWriter::new(v, types::BLOCK_HEADER, *handle, false);
        object_common(&mut w, 1);
        w.text(name);
        table_entry_flags(&mut w);
        w.data
            .bit(name.starts_with('*'))
            .bit(false)
            .bit(false)
            .bit(false);
        if v >= Version::R2000 {
            w.data.bit(false);
        }
        if v >= Version::R2004 {
            w.data.bl(0);
        }
        w.data.bd3(*base);
        w.text("");
        if v >= Version::R2000 {
            w.data.rc(0);
            w.text("");
            w.data.bl(0);
        }
        if v >= Version::R2007 {
            w.data.bs(0).bit(true).rc(0);
        }
        out.insert(*handle, w.finish(false));
    }
    for p in &d.entities {
        let (code, has_ds) = match &p.entity {
            TestEntity::Solid { .. } => (types::SOLID3D, v >= Version::R2013),
            TestEntity::Face3d(_) => (types::FACE3D, false),
            TestEntity::Pface { .. } => (types::POLYLINE_PFACE, false),
            TestEntity::Insert { array: None, .. } => (types::INSERT, false),
            TestEntity::Insert { .. } => (types::MINSERT, false),
            TestEntity::Mesh { .. } => (500, false),
            TestEntity::Line(..) => (types::LINE, false),
        };
        let mut w = ObjectWriter::new(v, code, p.handle, true);
        entity_common(&mut w, p.owner, p.layer, p.color, has_ds);
        match &p.entity {
            TestEntity::Solid { asm, text } => {
                if has_ds {
                    w.data.bit(true);
                    stored.push((p.handle, asm.clone()));
                } else if *text {
                    let file = mitcad_asm::asm::AsmFile::parse(asm).expect("a test body");
                    let sat = mitcad_asm::asm::text::from_binary(&file);
                    let masked = crate::solid::mask(sat.as_bytes());
                    w.data.bit(false).bit(false).bs(1);
                    for chunk in masked.chunks(4096) {
                        w.data.bl(chunk.len() as i32).bytes(chunk);
                    }
                    w.data.bl(0);
                } else {
                    w.data.bit(false).bit(false).bs(2).bytes(asm);
                }
                // No wireframe or silhouettes.
                w.data.bit(false).bl(0);
                if v >= Version::R2007 {
                    w.data.bl(0);
                    w.h(5, 0);
                }
            }
            TestEntity::Face3d(c) => {
                if v < Version::R2000 {
                    for p in c {
                        w.data.bd3(*p);
                    }
                    w.data.bs(0);
                } else {
                    w.data.bit(true).bit(c[0][2] == 0.0);
                    w.data.rd(c[0][0]).rd(c[0][1]);
                    if c[0][2] != 0.0 {
                        w.data.rd(c[0][2]);
                    }
                    for i in 1..4 {
                        for (v, d) in c[i].iter().zip(c[i - 1]) {
                            w.data.dd(*v, d);
                        }
                    }
                }
            }
            TestEntity::Pface { vertices, faces } => {
                w.data.bs(vertices.len() as i16).bs(faces.len() as i16);
                let first = p.handle + 1;
                let count = vertices.len() + faces.len();
                let seqend = first + count as u64;
                if v >= Version::R2004 {
                    w.data.bl(count as i32);
                    for k in 0..count as u64 {
                        w.h(3, first + k);
                    }
                } else {
                    w.h(4, first);
                    w.h(4, seqend - 1);
                }
                w.h(3, seqend);
                // The vertices, the faces and the end.
                let mut k = first;
                for point in vertices {
                    let mut vw = ObjectWriter::new(v, types::VERTEX_PFACE, k, true);
                    entity_common(&mut vw, Some(p.handle), p.layer, 256, false);
                    vw.data.rc(192).bd3(*point);
                    out.insert(k, vw.finish(false));
                    k += 1;
                }
                for f in faces {
                    let mut fw = ObjectWriter::new(v, types::VERTEX_PFACE_FACE, k, true);
                    entity_common(&mut fw, Some(p.handle), p.layer, 256, false);
                    for i in f {
                        fw.data.bs(*i);
                    }
                    out.insert(k, fw.finish(false));
                    k += 1;
                }
                let mut sw = ObjectWriter::new(v, types::SEQEND, k, true);
                entity_common(&mut sw, Some(p.handle), p.layer, 256, false);
                out.insert(k, sw.finish(false));
            }
            TestEntity::Insert {
                block,
                point,
                scale,
                rotation,
                array,
            } => {
                w.data.bd3(*point);
                if v < Version::R2000 {
                    w.data.bd3(*scale);
                } else if *scale == [1.0; 3] {
                    w.data.bb(3);
                } else if scale[1] == scale[0] && scale[2] == scale[0] {
                    w.data.bb(2).rd(scale[0]);
                } else {
                    w.data
                        .bb(0)
                        .rd(scale[0])
                        .dd(scale[1], scale[0])
                        .dd(scale[2], scale[0]);
                }
                w.data.bd(*rotation).bd3([0.0, 0.0, 1.0]).bit(false);
                if let Some((cols, rows, cs, rs)) = array {
                    w.data.bs(*cols).bs(*rows).bd(*cs).bd(*rs);
                }
                w.h(5, *block);
            }
            TestEntity::Mesh { vertices, faces } => {
                w.data.bs(2).bit(false).bl(0).bl(vertices.len() as i32);
                for p in vertices {
                    w.data.bd3(*p);
                }
                let list: usize = faces.iter().map(|f| f.len() + 1).sum();
                w.data.bl(list as i32);
                for f in faces {
                    w.data.bl(f.len() as i32);
                    for &i in f {
                        w.data.bl(i as i32);
                    }
                }
                w.data.bl(0).bl(0);
            }
            TestEntity::Line(a, b) => {
                // R2000 on: the z's zero bit, x, y and z each as RD and
                // DD (the specification's 20.4.21); then the thickness
                // and the extrusion.
                if v < Version::R2000 {
                    w.data.bd3(*a).bd3(*b);
                } else {
                    let flat = a[2] == 0.0 && b[2] == 0.0;
                    w.data.bit(flat);
                    w.data.rd(a[0]).dd(b[0], a[0]).rd(a[1]).dd(b[1], a[1]);
                    if !flat {
                        w.data.rd(a[2]).dd(b[2], a[2]);
                    }
                }
                w.data.bt(0.0).be([0.0, 0.0, 1.0]);
            }
        }
        out.insert(p.handle, w.finish(has_ds));
    }
    (out, stored)
}

/// Writes the test drawing in a version.
pub fn write(d: &TestDrawing, v: Version) -> Vec<u8> {
    let classes = test_classes();
    let (objects, stored) = objects(d, v);
    match v {
        Version::R13 | Version::R14 | Version::R2000 => write_r13(d, v, &classes, &objects),
        Version::R2007 => crate::r2007::write(&d.header, &classes, &objects),
        _ => write_r2004(d, v, &classes, &objects, &stored),
    }
}

fn write_r13(
    d: &TestDrawing,
    v: Version,
    classes: &[Class],
    objects: &BTreeMap<u64, Vec<u8>>,
) -> Vec<u8> {
    let header = crate::header::write(v, &d.header);
    let classes = crate::classes::write(classes, v);
    // The file header: the version, the locators (header, classes, object
    // map, an empty fourth), their CRC and the sentinel.
    let locators = 4;
    let start = 0x19 + 9 * locators + 2 + 16;
    let header_at = start;
    let classes_at = header_at + header.len();
    let objects_at = classes_at + classes.len();
    let (object_data, map) = object_stream(objects, Vec::new(), objects_at as u64);
    let map_at = objects_at + object_data.len();
    let map_bytes = crate::objmap::write(&map);
    let mut out = Vec::new();
    out.extend_from_slice(v.id().as_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 0, MAINTENANCE, 1]);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&[0x1b, 0x00]);
    out.extend_from_slice(&30u16.to_le_bytes());
    out.extend_from_slice(&(locators as u32).to_le_bytes());
    for (n, at, size) in [
        (0u8, header_at, header.len()),
        (1, classes_at, classes.len()),
        (2, map_at, map_bytes.len()),
        (3, 0, 0),
    ] {
        out.push(n);
        out.extend_from_slice(&(at as u32).to_le_bytes());
        out.extend_from_slice(&(size as u32).to_le_bytes());
    }
    let crc = crate::crc::crc16(0, &out) ^ 0x8101;
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&container::FILE_HEADER_END_SENTINEL);
    assert_eq!(out.len(), start);
    out.extend(header);
    out.extend(classes);
    out.extend(object_data);
    out.extend(map_bytes);
    out
}

/// One page of an R2004 file.
struct Page {
    number: u32,
    bytes: Vec<u8>,
}

fn pad32(mut b: Vec<u8>) -> Vec<u8> {
    b.resize(b.len().div_ceil(0x20) * 0x20, 0);
    b
}

fn system_page(kind: u32, data: &[u8]) -> Vec<u8> {
    let compressed = crate::lz::compress(data);
    let mut head = Vec::new();
    for v in [kind, data.len() as u32, compressed.len() as u32, 2, 0] {
        head.extend_from_slice(&v.to_le_bytes());
    }
    let sum = crate::crc::page_checksum(crate::crc::page_checksum(0, &head), &compressed);
    head[16..20].copy_from_slice(&sum.to_le_bytes());
    head.extend(compressed);
    pad32(head)
}

fn write_r2004(
    d: &TestDrawing,
    v: Version,
    classes: &[Class],
    objects: &BTreeMap<u64, Vec<u8>>,
    stored: &[(u64, Vec<u8>)],
) -> Vec<u8> {
    let (object_data, map) = object_stream(objects, 0x0dcau32.to_le_bytes().to_vec(), 0);
    let mut sections: Vec<(&str, Vec<u8>, usize)> = vec![
        (
            container::HEADER,
            crate::header::write(v, &d.header),
            0x7400,
        ),
        (
            container::CLASSES,
            crate::classes::write(classes, v),
            0x7400,
        ),
        (container::HANDLES, crate::objmap::write(&map), 0x7400),
        // Small pages, so that the objects take several.
        (container::OBJECTS, object_data, 0x400),
    ];
    if v >= Version::R2013 {
        sections.push((
            container::DATA_STORAGE,
            crate::datastore::write(stored),
            0x7400,
        ));
    }
    // The data pages, from page 1, at 0x100 on.
    let mut pages: Vec<Page> = Vec::new();
    let mut address = 0x100u64;
    let mut described = Vec::new();
    for (id, (name, data, page_size)) in sections.iter().enumerate() {
        let mut list = Vec::new();
        for (k, chunk) in data.chunks(*page_size).enumerate() {
            let number = pages.len() as u32 + 1;
            let compressed = crate::lz::compress(chunk);
            let start = (k * page_size) as u64;
            let mut head = [0u32; 8];
            head[0] = container::DATA_PAGE_TYPE;
            head[1] = id as u32 + 1;
            head[2] = compressed.len() as u32;
            head[3] = (32 + compressed.len()).div_ceil(0x20) as u32 * 0x20;
            head[4] = start as u32;
            head[5] = (start >> 32) as u32;
            head[7] = crate::crc::page_checksum(0, &compressed);
            let mask = 0x4164_536b_u32 ^ address as u32;
            let mut bytes = Vec::new();
            for h in head {
                bytes.extend_from_slice(&(h ^ mask).to_le_bytes());
            }
            bytes.extend(compressed);
            let bytes = pad32(bytes);
            list.push((number, head[2], start));
            address += bytes.len() as u64;
            pages.push(Page { number, bytes });
        }
        described.push((
            name.to_string(),
            data.len() as u64,
            *page_size,
            id as u32 + 1,
            list,
        ));
    }
    // The section map.
    let mut smap = Vec::new();
    for v in [described.len() as u32, 2, 0x7400, 0, described.len() as u32] {
        smap.extend_from_slice(&v.to_le_bytes());
    }
    for (name, size, page_size, id, list) in &described {
        smap.extend_from_slice(&size.to_le_bytes());
        for v in [list.len() as u32, *page_size as u32, 1, 2, *id, 0] {
            smap.extend_from_slice(&v.to_le_bytes());
        }
        let mut n = name.as_bytes().to_vec();
        n.resize(64, 0);
        smap.extend(n);
        for (number, data_size, start) in list {
            smap.extend_from_slice(&number.to_le_bytes());
            smap.extend_from_slice(&data_size.to_le_bytes());
            smap.extend_from_slice(&start.to_le_bytes());
        }
    }
    let section_map_id = pages.len() as u32 + 2;
    let section_map = system_page(container::SECTION_MAP_TYPE, &smap);
    let page_map_id = section_map_id + 1;
    // The page map lists every page with its size, itself too; its size
    // depends on its own length, so it is made twice.
    let page_map = |own: u32| {
        let mut m = Vec::new();
        for p in &pages {
            m.extend_from_slice(&p.number.to_le_bytes());
            m.extend_from_slice(&(p.bytes.len() as u32).to_le_bytes());
        }
        for (n, size) in [
            (section_map_id, section_map.len() as u32),
            (page_map_id, own),
        ] {
            m.extend_from_slice(&n.to_le_bytes());
            m.extend_from_slice(&size.to_le_bytes());
        }
        system_page(container::PAGE_MAP_TYPE, &m)
    };
    let mut pm = page_map(0);
    for _ in 0..3 {
        pm = page_map(pm.len() as u32);
    }
    let section_map_at = address;
    let page_map_at = section_map_at + section_map.len() as u64;
    // The file header.
    let mut out = Vec::new();
    out.extend_from_slice(v.id().as_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 0, MAINTENANCE, 0]);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&[0x1f, 0x00]);
    out.extend_from_slice(&30u16.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0]);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0x80u32.to_le_bytes());
    out.resize(0x80, 0);
    let mut h = b"AcFssFcAJMB\0".to_vec();
    let total_pages = pages.len() as u32 + 2;
    let end = page_map_at + pm.len() as u64;
    for v in [0u32, 0x6c, 4, 0, 0, 0, 1, page_map_id] {
        h.extend_from_slice(&v.to_le_bytes());
    }
    h.extend_from_slice(&end.to_le_bytes());
    h.extend_from_slice(&end.to_le_bytes());
    for v in [0u32, total_pages, 0x20, 0x80, 0x40, page_map_id] {
        h.extend_from_slice(&v.to_le_bytes());
    }
    h.extend_from_slice(&(page_map_at - 0x100).to_le_bytes());
    for v in [section_map_id, total_pages, 0, 0] {
        h.extend_from_slice(&v.to_le_bytes());
    }
    assert_eq!(h.len(), 0x6c);
    let crc = crate::crc::crc32(0, &h);
    h[0x68..].copy_from_slice(&crc.to_le_bytes());
    let mask = container::magic_sequence(0x100);
    out.extend(h.iter().zip(&mask).map(|(a, b)| a ^ b));
    out.extend_from_slice(&mask[0x100 - 0x14..]);
    assert_eq!(out.len(), 0x100);
    for p in pages {
        out.extend(p.bytes);
    }
    out.extend(section_map);
    out.extend(pm);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An R2004 3D solid as drawings store it, field by field (not with
    /// the writer's helpers): the common entity data has no links bit,
    /// and the B-rep data follows the line weight.
    #[test]
    fn an_r2004_solid_without_the_links_bit() {
        let v = Version::R2004;
        let mut w = ObjectWriter::new(v, types::SOLID3D, 0x7f, true);
        w.data.bb(2).bl(0).bit(true); // model space, no reactors, no xdictionary
        w.data.bs(256).bd(1.0).bb(0).bb(0).bs(0).rc(0x1d); // colour .. line weight
        let asm = mitcad_asm::testdata::cube_blob();
        w.data.bit(false).bit(false).bs(2).bytes(&asm);
        w.h(5, LAYER_0);
        let bytes = w.finish(false);
        let o = crate::objects::read(&bytes, 0, v, &[]).unwrap();
        let crate::objects::Kind::Entity(common, data) = o.kind else {
            panic!("not an entity");
        };
        assert_eq!(common.entmode, 2);
        assert_eq!(common.layer, Some(LAYER_0));
        assert_eq!(
            data,
            crate::objects::EntityData::Brep(crate::objects::BrepData::Binary(asm))
        );
    }

    /// An R2018 layer as drawings store it: after its 64-flag the xref
    /// dependency, no xref index; the colour as index, RGB and flags.
    #[test]
    fn an_r2018_layer_without_the_xref_index() {
        let v = Version::R2018;
        let mut w = ObjectWriter::new(v, types::LAYER, 0x46, false);
        w.data.bl(0).bit(false).bit(false); // reactors, xdictionary, data storage
        w.text("Tubes");
        w.data.bit(true).bit(false); // 64-flag, xref dependency
        w.data.bs(0x3f0); // plotting, line weight
        w.data.bs(0).bl(0xc300_0003u32 as i32).rc(0);
        w.h(4, 2);
        w.h(3, 0);
        let bytes = w.finish(false);
        let o = crate::objects::read(&bytes, 0, v, &[]).unwrap();
        let crate::objects::Kind::Layer(layer) = o.kind else {
            panic!("not a layer");
        };
        assert_eq!(layer.name, "Tubes");
        assert!(!layer.frozen);
    }

    #[test]
    fn extents_never_computed_are_none() {
        let mut d = test_drawing();
        d.header.extmin = Some([1e20; 3]);
        d.header.extmax = Some([-1e20; 3]);
        let file = crate::read(&write(&d, Version::R2018)).unwrap();
        let scene = crate::scene::Scene::of(&file);
        assert_eq!(scene.extents, None);
        assert!(scene.account.warnings.iter().any(|w| w.contains("not set")));
    }

    #[test]
    fn a_block_inserted_nowhere_is_named() {
        let mut d = test_drawing();
        d.blocks.push((0x22, "Spare".into(), [0.0; 3]));
        d.entities.push(Placed {
            handle: 0x70,
            owner: Some(0x22),
            layer: LAYER_0,
            color: 256,
            entity: TestEntity::Solid {
                asm: mitcad_asm::testdata::cube_blob(),
                text: false,
            },
        });
        let file = crate::read(&write(&d, Version::R2000)).unwrap();
        let plan = crate::scene::Scene::of(&file).plan();
        assert!(
            plan.warnings
                .iter()
                .any(|w| w.contains("Spare") && w.contains("inserted nowhere")),
            "{:?}",
            plan.warnings
        );
    }

    #[test]
    fn the_test_drawing_reads_back_in_every_version() {
        let d = test_drawing();
        for v in Version::ALL {
            let bytes = write(&d, v);
            let file = match crate::read(&bytes) {
                Ok(f) => f,
                Err(e) => panic!("{v}: {e}"),
            };
            assert_eq!(file.version, v);
            assert!(file.failed.is_empty(), "{v}: {:?}", file.failed);
            assert!(file.warnings.is_empty(), "{v}: {:?}", file.warnings);
            let h = file.header.as_ref().unwrap();
            assert_eq!(h.extmax, Some([81.732, 45.0, 31.0]), "{v}");
            assert_eq!(h.extmin, Some([-1.0, -1.0, 0.0]), "{v}");
            if v >= Version::R2000 {
                assert_eq!(h.insunits, Some(4), "{v}");
            }
            assert_eq!(file.classes, test_classes(), "{v}");
            let counts = file.type_counts();
            assert_eq!(counts["3DSOLID"], 2, "{v}");
            assert_eq!(counts["INSERT"], 5, "{v}");
            assert_eq!(counts["MINSERT"], 1, "{v}");
            assert_eq!(counts["3DFACE"], 2, "{v}");
            assert_eq!(counts["VERTEX"], 8, "{v}");
            assert_eq!(counts["MESH"], 1, "{v}");
            assert_eq!(counts["BLOCK_RECORD"], 3, "{v}");
            assert_eq!(counts["LAYER"], 2, "{v}");
            if v >= Version::R2013 {
                assert_eq!(file.stored_breps.len(), 2, "{v}");
            }
            // The line, read as a 2D entity (mitcad#728).
            let line = &file.objects[&0x63].kind;
            assert!(
                matches!(line, crate::objects::Kind::Entity(_, crate::objects::EntityData::Line { start, end })
                    if *start == [0.0; 3] && *end == [1.0; 3]),
                "{v}: {line:?}"
            );
        }
    }
}
