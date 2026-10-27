// SPDX-License-Identifier: MIT
//! A 2D drawing ([`mitcad_dxf::Drawing`]: lines, arcs, circles, points,
//! ellipses, splines and texts on layers) written as an R2000 (`AC1015`)
//! `.dwg` file (mitcad#728, the drawing export beside PDF, SVG and DXF).
//!
//! R2000 is the version every reader of the format reads, and its
//! container is the plain one: the sections one after another at the
//! addresses the file header lists, nothing compressed. The file holds
//! what such a drawing needs (the specification's chapters 3, 9, 10, 20,
//! 21 to 23 and 26): the header variables in full, the classes, the
//! tables (blocks, layers, text style, line types, views, UCSs, viewports,
//! applications, dimension styles, viewport entity headers) with their
//! standard entries, the named objects dictionary with the groups, layouts,
//! multiline styles, plot settings and plot styles, the model and paper
//! space blocks with their layouts, the entities in model space, the
//! object map, the object free space record, the template record (the
//! measurement) and an empty preview. The second file header of R13 to
//! R2000 is left out: readers find the sections by the first header's
//! locators.
//!
//! Texts are in the drawing's code page (ANSI 1252); other characters are
//! written as `\U+XXXX`, as the format keeps them before R2007.

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, TAU};

use mitcad_dxf::{Drawing, Geometry, Layer, Point2, Spline, Text};

use crate::bits::{BitWriter, write_modular_short};
use crate::classes::Class;
use crate::container::{FILE_HEADER_END_SENTINEL, HEADER_END_SENTINEL, HEADER_SENTINEL};
use crate::crc::crc16;
use crate::{DwgError, Version};

const V: Version = Version::R2000;

/// Options of [`drawing`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WriteOptions {
    /// When the drawing was made, seconds since 1970-01-01 UTC (the
    /// creation and update dates of the file); None: now.
    pub time: Option<f64>,
    /// The paper's width and height in drawing units (the limits and the
    /// layout's paper); None: the drawing's extents.
    pub paper: Option<[f64; 2]>,
}

// Object types (the specification's 20.3) and the classes the file lists.
mod code {
    pub const TEXT: u16 = 1;
    pub const BLOCK: u16 = 4;
    pub const ENDBLK: u16 = 5;
    pub const ARC: u16 = 0x11;
    pub const CIRCLE: u16 = 0x12;
    pub const LINE: u16 = 0x13;
    pub const POINT: u16 = 0x1B;
    pub const ELLIPSE: u16 = 0x23;
    pub const SPLINE: u16 = 0x24;
    pub const DICTIONARY: u16 = 0x2A;
    pub const BLOCK_CONTROL: u16 = 0x30;
    pub const BLOCK_HEADER: u16 = 0x31;
    pub const LAYER_CONTROL: u16 = 0x32;
    pub const LAYER: u16 = 0x33;
    pub const STYLE_CONTROL: u16 = 0x34;
    pub const STYLE: u16 = 0x35;
    pub const LTYPE_CONTROL: u16 = 0x38;
    pub const LTYPE: u16 = 0x39;
    pub const VIEW_CONTROL: u16 = 0x3C;
    pub const UCS_CONTROL: u16 = 0x3E;
    pub const VPORT_CONTROL: u16 = 0x40;
    pub const VPORT: u16 = 0x41;
    pub const APPID_CONTROL: u16 = 0x42;
    pub const APPID: u16 = 0x43;
    pub const DIMSTYLE_CONTROL: u16 = 0x44;
    pub const DIMSTYLE: u16 = 0x45;
    pub const VX_CONTROL: u16 = 0x46;
    pub const MLINESTYLE: u16 = 0x49;
    /// The classes, numbered from 500 in the order of [`super::classes`].
    pub const DICTIONARY_WITH_DEFAULT: u16 = 500;
    pub const PLACEHOLDER: u16 = 501;
    pub const LAYOUT: u16 = 502;
}

/// The classes of the objects the file holds that have no fixed type.
pub fn classes() -> Vec<Class> {
    let class = |number, cpp: &str, dxf: &str| Class {
        number,
        app_name: "ObjectDBX Classes".into(),
        cpp_name: cpp.into(),
        dxf_name: dxf.into(),
        entity: false,
    };
    vec![
        class(
            code::DICTIONARY_WITH_DEFAULT,
            "AcDbDictionaryWithDefault",
            "ACDBDICTIONARYWDFLT",
        ),
        class(code::PLACEHOLDER, "AcDbPlaceHolder", "ACDBPLACEHOLDER"),
        class(code::LAYOUT, "AcDbLayout", "LAYOUT"),
    ]
}

// The handles of the objects every file has.
const BLOCK_CONTROL: u64 = 0x1;
const LAYER_CONTROL: u64 = 0x2;
const STYLE_CONTROL: u64 = 0x3;
const LTYPE_CONTROL: u64 = 0x5;
const VIEW_CONTROL: u64 = 0x6;
const UCS_CONTROL: u64 = 0x7;
const VPORT_CONTROL: u64 = 0x8;
const APPID_CONTROL: u64 = 0x9;
const DIMSTYLE_CONTROL: u64 = 0xA;
const VX_CONTROL: u64 = 0xB;
const NAMED_OBJECTS: u64 = 0xC;
const GROUPS: u64 = 0xD;
const MLINESTYLES: u64 = 0xE;
const PLOTSTYLE_NORMAL: u64 = 0xF;
const LAYER_0: u64 = 0x10;
const STYLE: u64 = 0x11;
const LT_BYBLOCK: u64 = 0x12;
const LT_BYLAYER: u64 = 0x13;
const LT_CONTINUOUS: u64 = 0x14;
const APPID: u64 = 0x15;
const DIMSTYLE: u64 = 0x16;
const VPORT: u64 = 0x17;
const MLINESTYLE: u64 = 0x18;
const LAYOUTS: u64 = 0x19;
const PLOTSETTINGS: u64 = 0x1A;
const PLOTSTYLES: u64 = 0x1B;
const MODEL: u64 = 0x1C;
const MODEL_BLOCK: u64 = 0x1D;
const MODEL_ENDBLK: u64 = 0x1E;
const MODEL_LAYOUT: u64 = 0x1F;
const PAPER: u64 = 0x20;
const PAPER_BLOCK: u64 = 0x21;
const PAPER_ENDBLK: u64 = 0x22;
const PAPER_LAYOUT: u64 = 0x23;
/// The drawing's own layers, then its entities, from here on.
const FIRST_FREE: u64 = 0x24;

// Handle reference codes (the specification's 2.13).
const SOFT_OWNER: u8 = 2;
const HARD_OWNER: u8 = 3;
const SOFT_POINTER: u8 = 4;
const HARD_POINTER: u8 = 5;

/// An entity's line weight, by layer (the R2000 index).
const LINEWEIGHT_BYLAYER: u8 = 29;
/// A layer's flags: plotted, the default line weight (index 31).
const LAYER_PLOTTED_DEFAULT_WEIGHT: i16 = 0x10 | (31 << 5);

/// The drawing written as an R2000 `.dwg` file.
pub fn drawing(d: &Drawing, options: &WriteOptions) -> Result<Vec<u8>, DwgError> {
    for (i, e) in d.entities.iter().enumerate() {
        if !valid(&e.geometry) {
            return Err(DwgError::Unsupported(format!(
                "entity {i}: invalid geometry on layer {}",
                e.layer
            )));
        }
    }
    let time = options.time.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |t| t.as_secs_f64())
    });
    let plan = Plan::new(d, options, time);
    let objects = plan.objects(d);
    Ok(assemble(&plan, &objects))
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

fn valid(g: &Geometry) -> bool {
    match g {
        Geometry::Point(p) => finite(&[p.x, p.y]),
        Geometry::Line { start, end } => finite(&[start.x, start.y, end.x, end.y]),
        Geometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => finite(&[center.x, center.y, *radius, *start_angle, *end_angle]) && *radius > 0.0,
        Geometry::Circle { center, radius } => {
            finite(&[center.x, center.y, *radius]) && *radius > 0.0
        }
        Geometry::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => {
            finite(&[
                center.x,
                center.y,
                major_axis.x,
                major_axis.y,
                *ratio,
                *start_param,
                *end_param,
            ]) && major_axis.length() > 0.0
                && *ratio > 0.0
                && *ratio <= 1.0
        }
        Geometry::Spline(s) => {
            let points = s
                .control_points
                .iter()
                .chain(&s.fit_points)
                .all(|p| finite(&[p.x, p.y]));
            let polygon =
                (s.control_points.is_empty() && s.knots.is_empty()) || s.domain().is_some();
            let weights = s.weights.is_empty() || s.weights.len() == s.control_points.len();
            points
                && polygon
                && weights
                && finite(&s.knots)
                && finite(&s.weights)
                && (s.control_points.len() > s.degree as usize || s.fit_points.len() >= 2)
        }
        Geometry::Text(t) => finite(&[t.position.x, t.position.y, t.height, t.rotation]),
    }
}

/// An entry of the layer table.
struct LayerEntry {
    name: String,
    /// The colour index, 1 to 255.
    color: i16,
    on: bool,
    frozen: bool,
    handle: u64,
}

/// What the file holds, worked out before any object is written: the
/// layers and their handles, the entities' handles, the extents, the
/// paper and the next free handle.
struct Plan {
    /// The layer table, layer 0 first.
    layers: Vec<LayerEntry>,
    /// Each entity of the drawing becomes this many entities (a text of
    /// several lines one per line), with handles from `first_entity` on.
    first_entity: u64,
    entity_count: u64,
    seed: u64,
    extents: Option<([f64; 3], [f64; 3])>,
    paper: [f64; 2],
    insunits: i16,
    metric: bool,
    /// The Julian day and the milliseconds into it.
    day: i32,
    ms: i32,
    guid: [String; 2],
}

impl Plan {
    fn new(d: &Drawing, options: &WriteOptions, time: f64) -> Plan {
        // The layer table: 0, the drawing's layers, then the layers only
        // entities name.
        let mut layers = vec![LayerEntry {
            name: "0".to_owned(),
            color: 7,
            on: true,
            frozen: false,
            handle: LAYER_0,
        }];
        let mut next = FIRST_FREE;
        let mut add = |layer: &Layer, layers: &mut Vec<LayerEntry>| {
            let color = layer.color.clamp(1, 255);
            if let Some(entry) = layers
                .iter_mut()
                .find(|l| l.name.eq_ignore_ascii_case(&layer.name))
            {
                if entry.handle == LAYER_0 {
                    entry.color = color;
                    entry.on = layer.visible;
                    entry.frozen = layer.frozen;
                }
                return;
            }
            layers.push(LayerEntry {
                name: layer.name.clone(),
                color,
                on: layer.visible,
                frozen: layer.frozen,
                handle: next,
            });
            next += 1;
        };
        for layer in &d.layers {
            add(layer, &mut layers);
        }
        for e in &d.entities {
            add(&Layer::new(e.layer.clone()), &mut layers);
        }
        let entity_count: u64 = d.entities.iter().map(|e| pieces(&e.geometry) as u64).sum();
        let extents = d
            .bounds()
            .map(|(lo, hi)| ([lo.x, lo.y, 0.0], [hi.x, hi.y, 0.0]));
        let paper = options.paper.unwrap_or(match extents {
            Some((lo, hi)) => [hi[0].max(lo[0] + 1.0), hi[1].max(lo[1] + 1.0)],
            None => [420.0, 297.0],
        });
        let (day, ms) = julian(time);
        // Identifiers of the drawing and of this version of it, from what
        // it holds and when it was written.
        let mut hash = Fnv::default();
        hash.add(&time.to_le_bytes());
        hash.add(&(d.entities.len() as u64).to_le_bytes());
        for l in &layers {
            hash.add(l.name.as_bytes());
        }
        if let Some((lo, hi)) = extents {
            for v in lo.iter().chain(&hi) {
                hash.add(&v.to_le_bytes());
            }
        }
        let a = hash.0;
        hash.add(b"version");
        let b = hash.0;
        hash.add(b"guid");
        let c = hash.0;
        Plan {
            layers,
            first_entity: next,
            entity_count,
            seed: next + entity_count,
            extents,
            paper,
            insunits: d.units.code() as i16,
            metric: d.units.is_metric(),
            day,
            ms,
            guid: [guid(a, b), guid(b, c)],
        }
    }

    fn layer_handle(&self, name: &str) -> u64 {
        self.layers
            .iter()
            .find(|l| l.name == name)
            .or_else(|| {
                self.layers
                    .iter()
                    .find(|l| l.name.eq_ignore_ascii_case(name))
            })
            .map_or(LAYER_0, |l| l.handle)
    }

    /// The model space's first and last entity.
    fn entity_range(&self) -> (u64, u64) {
        if self.entity_count == 0 {
            (0, 0)
        } else {
            (self.first_entity, self.first_entity + self.entity_count - 1)
        }
    }

    /// Every object by handle.
    fn objects(&self, d: &Drawing) -> BTreeMap<u64, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut put = |o: Obj| {
            out.insert(o.handle, o.finish());
        };
        self.tables(&mut put);
        self.dictionaries(&mut put);
        self.blocks(&mut put);
        // The entities in model space, linked in order.
        let (first, last) = self.entity_range();
        let mut h = first;
        for e in &d.entities {
            let layer = self.layer_handle(&e.layer);
            for piece in entities(&e.geometry) {
                let prev = if h == first { 0 } else { h - 1 };
                let next = if h == last { 0 } else { h + 1 };
                put(entity(h, layer, &piece, prev, next));
                h += 1;
            }
        }
        out
    }

    fn tables(&self, put: &mut impl FnMut(Obj)) {
        // The control objects.
        let control = |code, handle, entries: &[u64]| {
            let mut o = Obj::object(code, handle, 0, &[]);
            o.data.bl(entries.len() as i32);
            for &e in entries {
                o.h(SOFT_OWNER, e);
            }
            o
        };
        let mut blocks = control(code::BLOCK_CONTROL, BLOCK_CONTROL, &[]);
        blocks.h(HARD_OWNER, MODEL);
        blocks.h(HARD_OWNER, PAPER);
        put(blocks);
        let layers: Vec<u64> = self.layers.iter().map(|l| l.handle).collect();
        put(control(code::LAYER_CONTROL, LAYER_CONTROL, &layers));
        put(control(code::STYLE_CONTROL, STYLE_CONTROL, &[STYLE]));
        // Line types: BYLAYER and BYBLOCK are not counted, and come last.
        let mut ltypes = control(code::LTYPE_CONTROL, LTYPE_CONTROL, &[LT_CONTINUOUS]);
        ltypes.h(HARD_OWNER, LT_BYLAYER);
        ltypes.h(HARD_OWNER, LT_BYBLOCK);
        put(ltypes);
        put(control(code::VIEW_CONTROL, VIEW_CONTROL, &[]));
        put(control(code::UCS_CONTROL, UCS_CONTROL, &[]));
        put(control(code::VPORT_CONTROL, VPORT_CONTROL, &[VPORT]));
        put(control(code::APPID_CONTROL, APPID_CONTROL, &[APPID]));
        // The dimension style control: from R2000 a count of further
        // handles (none) after the entries' count. A reader that does not
        // expect it stops at the object's data size.
        let mut o = Obj::object(code::DIMSTYLE_CONTROL, DIMSTYLE_CONTROL, 0, &[]);
        o.data.bl(1).rc(0);
        o.h(SOFT_OWNER, DIMSTYLE);
        put(o);
        put(control(code::VX_CONTROL, VX_CONTROL, &[]));

        // The layers.
        for LayerEntry {
            name,
            color,
            on,
            frozen,
            handle,
        } in &self.layers
        {
            let mut o = Obj::object(code::LAYER, *handle, LAYER_CONTROL, &[]);
            o.tv(name);
            entry_flags(&mut o);
            let mut flags = LAYER_PLOTTED_DEFAULT_WEIGHT;
            if *frozen {
                flags |= 1;
            }
            // The specification names the 2 bit "on"; layers that are on
            // have it clear in drawings (0x3F0 in an R2018 one), so it
            // is set for a layer that is off, besides the negated colour.
            if !*on {
                flags |= 2;
            }
            o.data.bs(flags);
            // A layer that is off has its colour negated.
            o.data.bs(if *on { *color } else { -*color });
            o.h(HARD_POINTER, 0); // xref block
            o.h(HARD_POINTER, PLOTSTYLE_NORMAL);
            o.h(HARD_POINTER, LT_CONTINUOUS);
            o.h(HARD_POINTER, 0);
            put(o);
        }

        // The text style.
        let mut o = Obj::object(code::STYLE, STYLE, STYLE_CONTROL, &[]);
        o.tv("Standard");
        entry_flags(&mut o);
        o.data.bit(false).bit(false); // vertical, shape file
        o.data.bd(0.0).bd(1.0).bd(0.0); // fixed height, width factor, oblique
        o.data.rc(0); // generation
        o.data.bd(2.5); // last height
        o.tv("txt");
        o.tv("");
        o.h(HARD_POINTER, 0);
        put(o);

        // The line types.
        for (handle, name, description) in [
            (LT_BYBLOCK, "ByBlock", ""),
            (LT_BYLAYER, "ByLayer", ""),
            (LT_CONTINUOUS, "Continuous", "Solid line"),
        ] {
            let mut o = Obj::object(code::LTYPE, handle, LTYPE_CONTROL, &[]);
            o.tv(name);
            entry_flags(&mut o);
            o.tv(description);
            o.data.bd(0.0); // pattern length
            o.data.rc(b'A').rc(0); // alignment, no dashes
            o.data.bytes(&[0; 256]); // the strings area
            o.h(HARD_POINTER, 0);
            put(o);
        }

        // The application of the drawing's own extended data.
        let mut o = Obj::object(code::APPID, APPID, APPID_CONTROL, &[]);
        o.tv("ACAD");
        entry_flags(&mut o);
        o.data.rc(0);
        o.h(HARD_POINTER, 0);
        put(o);

        // The dimension style, ISO values.
        let mut o = Obj::object(code::DIMSTYLE, DIMSTYLE, DIMSTYLE_CONTROL, &[]);
        o.tv("Standard");
        entry_flags(&mut o);
        dimension_values(&mut o.data);
        o.data.bit(false);
        o.h(HARD_POINTER, 0); // xref block
        o.h(HARD_POINTER, STYLE); // DIMTXSTY
        for _ in 0..4 {
            o.h(HARD_POINTER, 0); // DIMLDRBLK, DIMBLK, DIMBLK1, DIMBLK2
        }
        put(o);

        // The active viewport: the whole drawing in view.
        let (lo, hi) = self.view();
        let mut o = Obj::object(code::VPORT, VPORT, VPORT_CONTROL, &[]);
        o.tv("*Active");
        entry_flags(&mut o);
        let height = (hi[1] - lo[1]).max(1.0) * 1.1;
        let width = (hi[0] - lo[0]).max(1.0) * 1.1;
        o.data.bd(height).bd(width); // the aspect ratio times the height
        o.data.rd((lo[0] + hi[0]) / 2.0).rd((lo[1] + hi[1]) / 2.0);
        o.data.bd3([0.0; 3]).bd3([0.0, 0.0, 1.0]); // target, direction
        o.data.bd(0.0).bd(50.0).bd(0.0).bd(0.0); // twist, lens, clips
        // View mode: no perspective, no clipping; the fourth bit is the
        // opposite of the front clip at the eye.
        o.data.bit(false).bit(false).bit(false).bit(true);
        o.data.rc(0); // render mode
        o.data.rd(0.0).rd(0.0).rd(1.0).rd(1.0); // lower left, upper right
        o.data.bit(false).bs(1000).bit(true); // UCSFOLLOW, circle zoom, fast zoom
        o.data.bit(true).bit(true); // UCS icon on, at the origin
        o.data.bit(false).rd(10.0).rd(10.0); // grid
        o.data.bit(false).bit(false).bs(0).bd(0.0); // snap, style, isopair, angle
        o.data.rd(0.0).rd(0.0).rd(10.0).rd(10.0); // snap base, spacing
        o.data.bit(false).bit(true); // unknown, UCS per viewport
        o.data
            .bd3([0.0; 3])
            .bd3([1.0, 0.0, 0.0])
            .bd3([0.0, 1.0, 0.0]);
        o.data.bd(0.0).bs(0); // UCS elevation, orthographic type
        o.h(HARD_POINTER, 0); // xref block
        o.h(HARD_POINTER, 0); // named UCS
        o.h(HARD_POINTER, 0); // base UCS
        put(o);
    }

    /// The box the view and the layouts show.
    fn view(&self) -> ([f64; 3], [f64; 3]) {
        self.extents
            .unwrap_or(([0.0; 3], [self.paper[0], self.paper[1], 0.0]))
    }

    fn dictionaries(&self, put: &mut impl FnMut(Obj)) {
        fn dictionary(handle: u64, owner: u64, reactors: &[u64], items: &[(&str, u64)]) -> Obj {
            let mut o = Obj::object(code::DICTIONARY, handle, owner, reactors);
            dictionary_items(&mut o, items);
            o
        }
        put(dictionary(
            NAMED_OBJECTS,
            0,
            &[],
            &[
                ("ACAD_GROUP", GROUPS),
                ("ACAD_LAYOUT", LAYOUTS),
                ("ACAD_MLINESTYLE", MLINESTYLES),
                ("ACAD_PLOTSETTINGS", PLOTSETTINGS),
                ("ACAD_PLOTSTYLENAME", PLOTSTYLES),
            ],
        ));
        let parent = [NAMED_OBJECTS];
        put(dictionary(GROUPS, NAMED_OBJECTS, &parent, &[]));
        put(dictionary(
            LAYOUTS,
            NAMED_OBJECTS,
            &parent,
            &[("Layout1", PAPER_LAYOUT), ("Model", MODEL_LAYOUT)],
        ));
        put(dictionary(
            MLINESTYLES,
            NAMED_OBJECTS,
            &parent,
            &[("Standard", MLINESTYLE)],
        ));
        put(dictionary(PLOTSETTINGS, NAMED_OBJECTS, &parent, &[]));
        // The plot styles: a dictionary with "Normal" as its default.
        let mut o = Obj::object(
            code::DICTIONARY_WITH_DEFAULT,
            PLOTSTYLES,
            NAMED_OBJECTS,
            &parent,
        );
        dictionary_items(&mut o, &[("Normal", PLOTSTYLE_NORMAL)]);
        o.h(HARD_POINTER, PLOTSTYLE_NORMAL);
        put(o);
        put(Obj::object(
            code::PLACEHOLDER,
            PLOTSTYLE_NORMAL,
            PLOTSTYLES,
            &[PLOTSTYLES],
        ));

        // The standard multiline style: two lines half a unit apart.
        let mut o = Obj::object(code::MLINESTYLE, MLINESTYLE, MLINESTYLES, &[MLINESTYLES]);
        o.tv("Standard");
        o.tv("");
        o.data.bs(0).bs(256); // flags, fill colour by layer
        o.data.bd(FRAC_PI_2).bd(FRAC_PI_2);
        o.data.rc(2);
        for offset in [0.5, -0.5] {
            // Colour and line type by layer.
            o.data.bd(offset).bs(256).bs(0x7fff);
        }
        put(o);

        // The layouts: Model and Layout1.
        let (lo, hi) = self.view();
        for (handle, name, block, order, model) in [
            (MODEL_LAYOUT, "Model", MODEL, 0, true),
            (PAPER_LAYOUT, "Layout1", PAPER, 1, false),
        ] {
            let mut o = Obj::object(code::LAYOUT, handle, LAYOUTS, &[LAYOUTS]);
            o.tv(""); // page setup name
            o.tv(""); // printer or configuration
            o.data.bs(if model { 1712 } else { 688 }); // plot layout flags
            o.data.bd(0.0).bd(0.0).bd(0.0).bd(0.0); // margins
            o.data.bd(self.paper[0]).bd(self.paper[1]);
            o.tv(""); // paper size
            o.data.bd(0.0).bd(0.0); // plot origin
            o.data.bs(1).bs(0).bs(if model { 1 } else { 5 }); // mm, no rotation, extents or layout
            o.data.bd(0.0).bd(0.0).bd(0.0).bd(0.0); // window
            tv(&mut o.data, ""); // plot view name
            o.data.bd(1.0).bd(1.0); // custom scale
            o.tv(""); // style sheet
            o.data.bs(if model { 0 } else { 16 }).bd(1.0); // scale type, factor
            o.data.bd(0.0).bd(0.0); // paper image origin
            o.tv(name);
            o.data.bl(order).bs(1); // tab order, flags (PSLTSCALE)
            o.data.bd3([0.0; 3]); // UCS origin
            o.data.rd(0.0).rd(0.0).rd(self.paper[0]).rd(self.paper[1]); // limits
            o.data.bd3([0.0; 3]); // insertion base
            o.data.bd3([1.0, 0.0, 0.0]).bd3([0.0, 1.0, 0.0]);
            o.data.bd(0.0).bs(0); // elevation, orthographic type
            if model {
                o.data.bd3(lo).bd3(hi);
            } else {
                o.data.bd3([1e20; 3]).bd3([-1e20; 3]);
            }
            o.h(SOFT_POINTER, block);
            o.h(SOFT_POINTER, 0); // last active viewport
            o.h(HARD_POINTER, 0); // base UCS
            o.h(HARD_POINTER, 0); // named UCS
            put(o);
        }
    }

    fn blocks(&self, put: &mut impl FnMut(Obj)) {
        let (first, last) = self.entity_range();
        for (handle, name, begin, end, layout, range) in [
            (
                MODEL,
                "*Model_Space",
                MODEL_BLOCK,
                MODEL_ENDBLK,
                MODEL_LAYOUT,
                (first, last),
            ),
            (
                PAPER,
                "*Paper_Space",
                PAPER_BLOCK,
                PAPER_ENDBLK,
                PAPER_LAYOUT,
                (0, 0),
            ),
        ] {
            let mut o = Obj::object(code::BLOCK_HEADER, handle, BLOCK_CONTROL, &[]);
            o.tv(name);
            entry_flags(&mut o);
            // Anonymous, attributes, xref, overlaid, loaded.
            o.data
                .bit(false)
                .bit(false)
                .bit(false)
                .bit(false)
                .bit(false);
            o.data.bd3([0.0; 3]);
            o.tv(""); // xref path
            o.data.rc(0); // no inserts
            o.tv(""); // description
            o.data.bl(0); // no preview
            o.h(HARD_POINTER, 0);
            o.h(HARD_OWNER, begin);
            o.h(SOFT_POINTER, range.0);
            o.h(SOFT_POINTER, range.1);
            o.h(HARD_OWNER, end);
            o.h(HARD_POINTER, layout);
            put(o);
            // Its BLOCK and ENDBLK entities.
            let mut b = Obj::entity(code::BLOCK, begin, LAYER_0, Some(handle), 0, 0);
            b.tv(name);
            put(b);
            put(Obj::entity(code::ENDBLK, end, LAYER_0, Some(handle), 0, 0));
        }
    }
}

/// The dimension variables of the dimension style, in the order of an
/// R2000 DIMSTYLE (the header's are in another order): ISO values.
fn dimension_values(w: &mut BitWriter) {
    tv(w, ""); // DIMPOST
    tv(w, ""); // DIMAPOST
    // DIMSCALE DIMASZ DIMEXO DIMDLI DIMEXE DIMRND DIMDLE DIMTP DIMTM
    for v in [1.0, 2.5, 0.625, 3.75, 1.25, 0.0, 0.0, 0.0, 0.0] {
        w.bd(v);
    }
    // DIMTOL DIMLIM DIMTIH DIMTOH DIMSE1 DIMSE2
    for _ in 0..6 {
        w.bit(false);
    }
    w.bs(1).bs(8).bs(0); // DIMTAD DIMZIN DIMAZIN
    // DIMTXT DIMCEN DIMTSZ DIMALTF DIMLFAC DIMTVP DIMTFAC DIMGAP DIMALTRND
    for v in [2.5, 2.5, 0.0, 0.0394, 1.0, 0.0, 1.0, 0.625, 0.0] {
        w.bd(v);
    }
    w.bit(false).bs(3); // DIMALT DIMALTD
    w.bit(true).bit(false).bit(false).bit(false); // DIMTOFL DIMSAH DIMTIX DIMSOXD
    w.bs(0).bs(0).bs(0); // DIMCLRD DIMCLRE DIMCLRT
    // DIMADEC DIMDEC DIMTDEC DIMALTU DIMALTTD DIMAUNIT DIMFRAC DIMLUNIT
    // DIMDSEP DIMTMOVE DIMJUST
    for v in [0, 2, 2, 2, 3, 0, 0, 2, 44, 0, 0] {
        w.bs(v);
    }
    w.bit(false).bit(false); // DIMSD1 DIMSD2
    w.bs(0).bs(8).bs(0).bs(0); // DIMTOLJ DIMTZIN DIMALTZ DIMALTTZ
    w.bit(false).bs(3); // DIMUPT DIMFIT
    w.bs(-2).bs(-2); // DIMLWD DIMLWE
}

fn dictionary_items(o: &mut Obj, items: &[(&str, u64)]) {
    o.data.bl(items.len() as i32);
    o.data.bs(1).rc(0); // cloning: keep existing; not the hard owner
    for (name, _) in items {
        o.tv(name);
    }
    for (_, handle) in items {
        o.h(SOFT_OWNER, *handle);
    }
}

/// A table entry's flags: not referenced, not from an xref.
fn entry_flags(o: &mut Obj) {
    o.data.bit(false).bs(0).bit(false);
}

/// One entity as the file stores it.
#[derive(Clone, Debug, PartialEq)]
enum Piece {
    Point([f64; 3]),
    Line([f64; 3], [f64; 3]),
    Circle([f64; 3], f64),
    Arc([f64; 3], f64, f64, f64),
    Ellipse {
        center: [f64; 3],
        major: [f64; 3],
        ratio: f64,
        start: f64,
        end: f64,
    },
    Spline(Spline),
    Text {
        at: [f64; 3],
        height: f64,
        rotation: f64,
        value: String,
    },
}

fn p3(p: Point2) -> [f64; 3] {
    [p.x, p.y, 0.0]
}

/// How many entities the geometry becomes.
fn pieces(g: &Geometry) -> usize {
    match g {
        Geometry::Text(t) => t.content.split('\n').count(),
        _ => 1,
    }
}

/// The entities of one geometry.
fn entities(g: &Geometry) -> Vec<Piece> {
    let full = |start: f64, end: f64| end - start >= TAU - 1e-12;
    match g {
        Geometry::Point(p) => vec![Piece::Point(p3(*p))],
        Geometry::Line { start, end } => vec![Piece::Line(p3(*start), p3(*end))],
        Geometry::Circle { center, radius } => vec![Piece::Circle(p3(*center), *radius)],
        Geometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            if full(*start_angle, *end_angle) {
                vec![Piece::Circle(p3(*center), *radius)]
            } else {
                vec![Piece::Arc(
                    p3(*center),
                    *radius,
                    start_angle.rem_euclid(TAU),
                    end_angle.rem_euclid(TAU),
                )]
            }
        }
        Geometry::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => {
            let (start, end) = if full(*start_param, *end_param) {
                (0.0, TAU)
            } else {
                (start_param.rem_euclid(TAU), end_param.rem_euclid(TAU))
            };
            vec![Piece::Ellipse {
                center: p3(*center),
                major: p3(*major_axis),
                ratio: *ratio,
                start,
                end,
            }]
        }
        Geometry::Spline(s) => vec![Piece::Spline(s.clone())],
        Geometry::Text(t) => text_lines(t),
    }
}

/// A text a TEXT entity per line, each line 5/3 of the height below the
/// one before.
fn text_lines(t: &Text) -> Vec<Piece> {
    let down = Point2::polar(t.rotation).perp() * (-t.height * 5.0 / 3.0);
    t.content
        .split('\n')
        .enumerate()
        .map(|(i, line)| Piece::Text {
            at: p3(t.position + down * i as f64),
            height: t.height,
            rotation: t.rotation,
            value: line.to_owned(),
        })
        .collect()
}

/// An entity in model space with its links to the one before and after.
fn entity(handle: u64, layer: u64, piece: &Piece, prev: u64, next: u64) -> Obj {
    let kind = match piece {
        Piece::Point(_) => code::POINT,
        Piece::Line(..) => code::LINE,
        Piece::Circle(..) => code::CIRCLE,
        Piece::Arc(..) => code::ARC,
        Piece::Ellipse { .. } => code::ELLIPSE,
        Piece::Spline(_) => code::SPLINE,
        Piece::Text { .. } => code::TEXT,
    };
    let mut o = Obj::entity(kind, handle, layer, None, prev, next);
    let z = [0.0, 0.0, 1.0];
    let w = &mut o.data;
    match piece {
        Piece::Point(p) => {
            w.bd3(*p).bt(0.0).be(z).bd(0.0);
        }
        Piece::Line(a, b) => {
            let flat = a[2] == 0.0 && b[2] == 0.0;
            w.bit(flat);
            w.rd(a[0]).dd(b[0], a[0]).rd(a[1]).dd(b[1], a[1]);
            if !flat {
                w.rd(a[2]).dd(b[2], a[2]);
            }
            w.bt(0.0).be(z);
        }
        Piece::Circle(c, r) => {
            w.bd3(*c).bd(*r).bt(0.0).be(z);
        }
        Piece::Arc(c, r, start, end) => {
            w.bd3(*c).bd(*r).bt(0.0).be(z).bd(*start).bd(*end);
        }
        Piece::Ellipse {
            center,
            major,
            ratio,
            start,
            end,
        } => {
            w.bd3(*center).bd3(*major).bd3(z);
            w.bd(*ratio).bd(*start).bd(*end);
        }
        Piece::Spline(s) => spline(w, s),
        Piece::Text {
            at,
            height,
            rotation,
            value,
        } => {
            // Data flags: no elevation, no alignment point, no oblique
            // angle, no width factor, generation and alignments the
            // defaults; the rotation when there is one.
            let mut flags = 0x01 | 0x02 | 0x04 | 0x10 | 0x20 | 0x40 | 0x80;
            if *rotation == 0.0 {
                flags |= 0x08;
            }
            w.rc(flags);
            w.rd(at[0]).rd(at[1]);
            w.be(z).bt(0.0);
            if *rotation != 0.0 {
                w.rd(rotation.rem_euclid(TAU));
            }
            w.rd(*height);
            tv(w, value);
            o.h(HARD_POINTER, STYLE);
        }
    }
    o
}

/// A spline: by its control points and knots, or by its fit points.
fn spline(w: &mut BitWriter, s: &Spline) {
    if s.control_points.is_empty() {
        w.bl(2).bl(s.degree as i32);
        w.bd(1e-10).bd3([0.0; 3]).bd3([0.0; 3]);
        w.bl(s.fit_points.len() as i32);
        for p in &s.fit_points {
            w.bd3(p3(*p));
        }
        return;
    }
    let rational = s.is_rational();
    w.bl(1).bl(s.degree as i32);
    w.bit(rational).bit(s.closed).bit(s.periodic);
    w.bd(1e-10).bd(1e-10);
    w.bl(s.knots.len() as i32).bl(s.control_points.len() as i32);
    w.bit(rational);
    for k in &s.knots {
        w.bd(*k);
    }
    for (i, p) in s.control_points.iter().enumerate() {
        w.bd3(p3(*p));
        if rational {
            w.bd(s.weight(i));
        }
    }
}

/// One object's data and handle streams while it is written.
struct Obj {
    handle: u64,
    code: u16,
    entity: bool,
    data: BitWriter,
    handles: BitWriter,
}

impl Obj {
    /// A non-entity object with its owner and persistent reactors.
    fn object(code: u16, handle: u64, owner: u64, reactors: &[u64]) -> Obj {
        let mut o = Obj {
            handle,
            code,
            entity: false,
            data: BitWriter::new(V),
            handles: BitWriter::new(V),
        };
        o.data.bl(reactors.len() as i32);
        o.h(SOFT_POINTER, owner);
        for &r in reactors {
            o.h(SOFT_POINTER, r);
        }
        o.h(HARD_OWNER, 0); // no extension dictionary
        o
    }

    /// An entity on a layer: in model space (`owner` None), else owned by
    /// a block header; linked to the entities before and after it.
    fn entity(code: u16, handle: u64, layer: u64, owner: Option<u64>, prev: u64, next: u64) -> Obj {
        let mut o = Obj {
            handle,
            code,
            entity: true,
            data: BitWriter::new(V),
            handles: BitWriter::new(V),
        };
        o.data.bb(if owner.is_some() { 0 } else { 2 });
        o.data.bl(0); // reactors
        o.data.bit(false); // the links are stored
        o.data.bs(256); // colour by layer
        o.data.bd(1.0); // line type scale
        o.data.bb(0).bb(0); // line type and plot style by layer
        o.data.bs(0); // visible
        o.data.rc(LINEWEIGHT_BYLAYER);
        if let Some(owner) = owner {
            o.h(SOFT_POINTER, owner);
        }
        o.h(HARD_OWNER, 0); // no extension dictionary
        o.h(SOFT_POINTER, prev);
        o.h(SOFT_POINTER, next);
        o.h(HARD_POINTER, layer);
        o
    }

    fn h(&mut self, code: u8, handle: u64) {
        self.handles.handle(code, handle);
    }

    fn tv(&mut self, s: &str) {
        tv(&mut self.data, s);
    }

    /// The object's bytes: its size, the data, the handles and the CRC.
    fn finish(self) -> Vec<u8> {
        let mut head = BitWriter::new(V);
        head.ot(self.code);
        let mut after = BitWriter::new(V);
        after.handle(0, self.handle).bs(0); // the handle, no extended data
        if self.entity {
            after.bit(false); // no graphics
        }
        // The size in bits of everything before the handles, counted from
        // the type on (with the 32 bits of this size).
        let endbit = head.len + 32 + after.len + self.data.len;
        let mut all = head;
        all.rl(endbit as u32);
        all.append(&after);
        all.append(&self.data);
        all.append(&self.handles);
        all.align();
        let mut out = Vec::new();
        write_modular_short(&mut out, all.data.len() as u64);
        out.extend_from_slice(&all.data);
        let crc = crc16(0xC0C1, &out);
        out.extend_from_slice(&crc.to_le_bytes());
        out
    }
}

/// TV of R2000: a bit short length and the text in the code page.
fn tv(w: &mut BitWriter, s: &str) {
    let mut bytes = encode(s);
    bytes.truncate(i16::MAX as usize);
    w.bs(bytes.len() as i16).bytes(&bytes);
}

/// The characters of code page 1252 at 0x80 to 0x9F (0 where it has none).
const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
    0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
];

/// A text in code page 1252; other characters as `\U+XXXX`, and `%%`
/// (which starts a control code) as `%%%%%%`.
pub fn encode(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut percents = 0;
    for c in s.chars() {
        let u = c as u32;
        if c == '%' {
            percents += 1;
            if percents == 2 {
                // "%%%" is a percent sign: "%%" becomes two of them.
                out.extend_from_slice(b"%%%%%");
                percents = 0;
            } else {
                out.push(b'%');
            }
            continue;
        }
        percents = 0;
        if (0x20..0x7f).contains(&u) || (0xa0..=0xff).contains(&u) {
            out.push(u as u8);
        } else if let Some(i) = CP1252_HIGH
            .iter()
            .position(|&h| h != 0 && u32::from(h) == u)
        {
            out.push(0x80 + i as u8);
        } else if u < 0x20 {
            // Control characters are left out (tabs become spaces).
            if c == '\t' {
                out.push(b' ');
            }
        } else {
            out.extend_from_slice(format!("\\U+{u:04X}").as_bytes());
        }
    }
    out
}

/// A text read back from the code page (as `latin1` gives it): the code
/// page's characters at 0x80 to 0x9F, `\U+XXXX`, and `%%%` a percent sign.
pub fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\'
            && chars.get(i + 1) == Some(&'U')
            && chars.get(i + 2) == Some(&'+')
            && i + 7 <= chars.len()
        {
            let hex: String = chars[i + 3..i + 7].iter().collect();
            if let Some(d) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                out.push(d);
                i += 7;
                continue;
            }
        }
        if c == '%' && chars.get(i + 1) == Some(&'%') && chars.get(i + 2) == Some(&'%') {
            out.push('%');
            i += 3;
            continue;
        }
        let u = c as u32;
        if (0x80..0xa0).contains(&u) {
            let h = CP1252_HIGH[(u - 0x80) as usize];
            out.push(char::from_u32(u32::from(h)).filter(|_| h != 0).unwrap_or(c));
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// The Julian day of a time (seconds since 1970) and the milliseconds
/// into it.
fn julian(seconds: f64) -> (i32, i32) {
    let jd = seconds / 86_400.0 + 2_440_587.5;
    let day = jd.floor();
    let ms = ((jd - day) * 86_400_000.0).round().min(86_399_999.0);
    (day as i32, ms as i32)
}

/// FNV-1a, 64 bits: identifiers from the drawing's content.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn add(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

fn guid(a: u64, b: u64) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:04X}-{:012X}}}",
        a >> 32,
        (a >> 16) & 0xffff,
        a & 0xffff,
        b >> 48,
        b & 0xffff_ffff_ffff
    )
}

/// The header variables (chapter 9) of R2000, from the sentinel to the
/// end sentinel.
fn header(p: &Plan) -> Vec<u8> {
    let mut w = BitWriter::new(V);
    let bits = |w: &mut BitWriter, values: &[bool]| {
        for &b in values {
            w.bit(b);
        }
    };
    let null = |w: &mut BitWriter| {
        w.handle(HARD_POINTER, 0);
    };
    w.bd(crate::header::FIRST_VALUE).bd(1.0).bd(1.0).bd(1.0);
    for _ in 0..4 {
        tv(&mut w, "");
    }
    w.bl(24).bl(0);
    null(&mut w); // the current viewport entity header
    bits(&mut w, &[true, true]); // DIMASO DIMSHO
    // PLINEGEN ORTHOMODE REGENMODE FILLMODE QTEXTMODE PSLTSCALE LIMCHECK
    bits(&mut w, &[false, false, true, true, false, true, false]);
    bits(&mut w, &[true, false, false, false]); // USRTIMER SKPOLY ANGDIR SPLFRAME
    bits(&mut w, &[false, true]); // MIRRTEXT WORLDVIEW
    bits(&mut w, &[true, false, true]); // TILEMODE PLIMCHECK VISRETAIN
    bits(&mut w, &[false, false]); // DISPSILH PELLIPSE
    w.bs(1); // PROXYGRAPHICS
    // TREEDEPTH LUNITS LUPREC AUNITS AUPREC
    for v in [3020, 2, 4, 0, 0] {
        w.bs(v);
    }
    w.bs(1).bs(0); // ATTMODE PDMODE
    // USERI1-5 SPLINESEGS SURFU SURFV SURFTYPE SURFTAB1 SURFTAB2
    // SPLINETYPE SHADEDGE SHADEDIF UNITMODE MAXACTVP ISOLINES CMLJUST
    // TEXTQLTY
    for v in [0, 0, 0, 0, 0, 8, 6, 6, 6, 6, 6, 6, 3, 70, 0, 64, 4, 0, 50] {
        w.bs(v);
    }
    // LTSCALE TEXTSIZE TRACEWID SKETCHINC FILLETRAD THICKNESS ANGBASE
    // PDSIZE PLINEWID USERR1-5 CHAMFERA-D FACETRES CMLSCALE CELTSCALE
    for v in [
        1.0, 2.5, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.5, 1.0, 1.0,
    ] {
        w.bd(v);
    }
    tv(&mut w, "acad"); // MENUNAME
    // TDCREATE TDUPDATE (day, milliseconds), TDINDWG TDUSRTIMER
    w.bl(p.day).bl(p.ms).bl(p.day).bl(p.ms);
    w.bl(0).bl(0).bl(0).bl(0);
    w.bs(256); // CECOLOR: by layer
    w.handle(0, p.seed); // HANDSEED
    w.handle(HARD_POINTER, LAYER_0); // CLAYER
    w.handle(HARD_POINTER, STYLE); // TEXTSTYLE
    w.handle(HARD_POINTER, LT_BYLAYER); // CELTYPE
    w.handle(HARD_POINTER, DIMSTYLE); // DIMSTYLE
    w.handle(HARD_POINTER, MLINESTYLE); // CMLSTYLE
    w.bd(0.0); // PSVPSCALE
    let origin = [0.0; 3];
    let x = [1.0, 0.0, 0.0];
    let y = [0.0, 1.0, 0.0];
    // Paper space: INSBASE EXTMIN EXTMAX LIMMIN LIMMAX ELEVATION UCSORG
    // UCSXDIR UCSYDIR UCSNAME, PUCSORTHOREF PUCSORTHOVIEW PUCSBASE and the
    // six orthographic origins.
    w.bd3(origin).bd3([1e20; 3]).bd3([-1e20; 3]);
    w.rd(0.0).rd(0.0).rd(p.paper[0]).rd(p.paper[1]);
    w.bd(0.0).bd3(origin).bd3(x).bd3(y);
    null(&mut w);
    null(&mut w);
    w.bs(0);
    null(&mut w);
    for _ in 0..6 {
        w.bd3(origin);
    }
    // Model space, the same.
    let (min, max) = p.extents.unwrap_or(([1e20; 3], [-1e20; 3]));
    w.bd3(origin).bd3(min).bd3(max);
    w.rd(0.0).rd(0.0).rd(p.paper[0]).rd(p.paper[1]);
    w.bd(0.0).bd3(origin).bd3(x).bd3(y);
    null(&mut w);
    null(&mut w);
    w.bs(0);
    null(&mut w);
    for _ in 0..6 {
        w.bd3(origin);
    }
    tv(&mut w, ""); // DIMPOST
    tv(&mut w, ""); // DIMAPOST
    // DIMSCALE DIMASZ DIMEXO DIMDLI DIMEXE DIMRND DIMDLE DIMTP DIMTM
    for v in [1.0, 2.5, 0.625, 3.75, 1.25, 0.0, 0.0, 0.0, 0.0] {
        w.bd(v);
    }
    bits(&mut w, &[false; 6]); // DIMTOL DIMLIM DIMTIH DIMTOH DIMSE1 DIMSE2
    w.bs(1).bs(8).bs(0); // DIMTAD DIMZIN DIMAZIN
    // DIMTXT DIMCEN DIMTSZ DIMALTF DIMLFAC DIMTVP DIMTFAC DIMGAP
    for v in [2.5, 2.5, 0.0, 0.0394, 1.0, 0.0, 1.0, 0.625] {
        w.bd(v);
    }
    w.bd(0.0).bit(false).bs(3); // DIMALTRND DIMALT DIMALTD
    bits(&mut w, &[true, false, false, false]); // DIMTOFL DIMSAH DIMTIX DIMSOXD
    w.bs(0).bs(0).bs(0); // DIMCLRD DIMCLRE DIMCLRT
    // DIMADEC DIMDEC DIMTDEC DIMALTU DIMALTTD DIMAUNIT DIMFRAC DIMLUNIT
    // DIMDSEP DIMTMOVE DIMJUST
    for v in [0, 2, 2, 2, 3, 0, 0, 2, 44, 0, 0] {
        w.bs(v);
    }
    bits(&mut w, &[false, false]); // DIMSD1 DIMSD2
    w.bs(0).bs(8).bs(0).bs(0); // DIMTOLJ DIMTZIN DIMALTZ DIMALTTZ
    w.bit(false).bs(3); // DIMUPT DIMATFIT
    w.handle(HARD_POINTER, STYLE); // DIMTXSTY
    for _ in 0..4 {
        null(&mut w); // DIMLDRBLK DIMBLK DIMBLK1 DIMBLK2
    }
    w.bs(-2).bs(-2); // DIMLWD DIMLWE
    for control in [
        BLOCK_CONTROL,
        LAYER_CONTROL,
        STYLE_CONTROL,
        LTYPE_CONTROL,
        VIEW_CONTROL,
        UCS_CONTROL,
        VPORT_CONTROL,
        APPID_CONTROL,
        DIMSTYLE_CONTROL,
        VX_CONTROL,
    ] {
        w.handle(HARD_OWNER, control);
    }
    w.handle(HARD_POINTER, GROUPS);
    w.handle(HARD_POINTER, MLINESTYLES);
    w.handle(HARD_OWNER, NAMED_OBJECTS);
    w.bs(1).bs(70); // TSTACKALIGN TSTACKSIZE
    tv(&mut w, ""); // HYPERLINKBASE
    tv(&mut w, ""); // STYLESHEET
    w.handle(HARD_POINTER, LAYOUTS);
    w.handle(HARD_POINTER, PLOTSETTINGS);
    w.handle(HARD_POINTER, PLOTSTYLES);
    // CELWEIGHT by layer, line weights not shown, long names, plot styles
    // by colour.
    w.bl(i32::from(LINEWEIGHT_BYLAYER) | 0x0200 | 0x0800 | 0x2000);
    w.bs(p.insunits);
    w.bs(0); // CEPSNTYPE: by layer
    tv(&mut w, &p.guid[0]); // FINGERPRINTGUID
    tv(&mut w, &p.guid[1]); // VERSIONGUID
    w.handle(HARD_POINTER, PAPER);
    w.handle(HARD_POINTER, MODEL);
    w.handle(HARD_POINTER, LT_BYLAYER);
    w.handle(HARD_POINTER, LT_BYBLOCK);
    w.handle(HARD_POINTER, LT_CONTINUOUS);
    // Four shorts of R14 on that readers do not need.
    w.bs(0).bs(0).bs(0).bs(0);
    w.align();
    let mut out = HEADER_SENTINEL.to_vec();
    out.extend_from_slice(&(w.data.len() as u32).to_le_bytes());
    out.extend_from_slice(&w.data);
    let crc = crc16(0xC0C1, &out[16..]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&HEADER_END_SENTINEL);
    out
}

/// The start and end sentinels of the preview (the specification's 14.2).
const PREVIEW_SENTINEL: [u8; 16] = [
    0x1F, 0x25, 0x6D, 0x07, 0xD4, 0x36, 0x28, 0x28, 0x9D, 0x57, 0xCA, 0x3F, 0x9D, 0x44, 0x10, 0x2B,
];
const PREVIEW_END_SENTINEL: [u8; 16] = [
    0xE0, 0xDA, 0x92, 0xF8, 0x2B, 0xC9, 0xD7, 0xD7, 0x62, 0xA8, 0x35, 0xC0, 0x62, 0xBB, 0xEF, 0xD4,
];

/// The file: the file header with five section locators, the header
/// variables, the classes, the padding, the objects, the object map, the
/// object free space record, the template record and the (empty) preview.
fn assemble(p: &Plan, objects: &BTreeMap<u64, Vec<u8>>) -> Vec<u8> {
    const LOCATORS: usize = 5;
    let header = header(p);
    let classes = crate::classes::write(&classes(), V);
    let header_at = 0x19 + 9 * LOCATORS + 2 + 16;
    let classes_at = header_at + header.len();
    // 0x200 bytes of padding after the classes (R13C3 on).
    let objects_at = classes_at + classes.len() + 0x200;
    let (object_data, map) = crate::objmap::object_stream(objects, Vec::new(), objects_at as u64);
    let map_at = objects_at + object_data.len();
    let map_bytes = crate::objmap::write(&map);
    let free_at = map_at + map_bytes.len();
    // The object free space record (chapter 21 and 25).
    let mut free = Vec::new();
    free.extend_from_slice(&0u32.to_le_bytes());
    free.extend_from_slice(&(map.len() as u32 + 3).to_le_bytes());
    free.extend_from_slice(&p.day.to_le_bytes());
    free.extend_from_slice(&p.ms.to_le_bytes());
    free.extend_from_slice(&(objects_at as u32).to_le_bytes());
    free.push(4);
    for v in [0x32u32, 0, 0x64, 0, 0x200, 0, 0xffff_ffff, 0] {
        free.extend_from_slice(&v.to_le_bytes());
    }
    debug_assert_eq!(free.len(), 53);
    let template_at = free_at + free.len();
    // The template record: no description, MEASUREMENT.
    let mut template = 0u16.to_le_bytes().to_vec();
    template.extend_from_slice(&u16::from(p.metric).to_le_bytes());
    let preview_at = template_at + template.len();

    let mut out = Vec::new();
    out.extend_from_slice(V.id().as_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 0, 0, 1]);
    out.extend_from_slice(&(preview_at as u32).to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&30u16.to_le_bytes()); // ANSI 1252
    out.extend_from_slice(&(LOCATORS as u32).to_le_bytes());
    for (n, at, size) in [
        (0u8, header_at, header.len()),
        (1, classes_at, classes.len()),
        (2, map_at, map_bytes.len()),
        (3, free_at, free.len()),
        (4, template_at, template.len()),
    ] {
        out.push(n);
        out.extend_from_slice(&(at as u32).to_le_bytes());
        out.extend_from_slice(&(size as u32).to_le_bytes());
    }
    let crc = crc16(0, &out) ^ 0x3CC4;
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&FILE_HEADER_END_SENTINEL);
    debug_assert_eq!(out.len(), header_at);
    out.extend(header);
    out.extend(classes);
    out.resize(objects_at, 0);
    out.extend(object_data);
    out.extend(map_bytes);
    out.extend(free);
    out.extend(template);
    out.extend_from_slice(&PREVIEW_SENTINEL);
    out.extend_from_slice(&1u32.to_le_bytes());
    out.push(0); // no images
    out.extend_from_slice(&PREVIEW_END_SENTINEL);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::{EntityData, Kind};
    use mitcad_dxf::Units;

    fn sample() -> Drawing {
        let mut d = Drawing::new(Units::Millimeters);
        let mut visible = Layer::new("VISIBLE");
        visible.color = 7;
        d.layers.push(visible);
        let mut hidden = Layer::new("HIDDEN");
        hidden.color = 8;
        hidden.visible = false;
        d.layers.push(hidden);
        d.push(
            "VISIBLE",
            Geometry::Line {
                start: Point2::new(10.0, 10.0),
                end: Point2::new(410.0, 10.0),
            },
        );
        d.push(
            "VISIBLE",
            Geometry::Line {
                start: Point2::new(10.0, 10.0),
                end: Point2::new(10.0, 287.0),
            },
        );
        d.push(
            "HIDDEN",
            Geometry::arc(Point2::new(100.0, 100.0), 20.0, 0.25, 4.0),
        );
        d.push(
            "VISIBLE",
            Geometry::Circle {
                center: Point2::new(200.0, 150.0),
                radius: 3.3,
            },
        );
        d.push("THIN", Geometry::Point(Point2::new(5.0, 6.0)));
        d.push(
            "THIN",
            Geometry::Ellipse {
                center: Point2::new(300.0, 200.0),
                major_axis: Point2::new(30.0, 10.0),
                ratio: 0.5,
                start_param: 0.0,
                end_param: TAU,
            },
        );
        d.push(
            "THIN",
            Geometry::Spline(Spline {
                degree: 2,
                control_points: vec![
                    Point2::new(0.0, 0.0),
                    Point2::new(5.0, 10.0),
                    Point2::new(10.0, 0.0),
                ],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                ..Spline::default()
            }),
        );
        d.push(
            "TEXT",
            Geometry::Text(Text {
                position: Point2::new(300.0, 20.0),
                height: 3.5,
                rotation: 0.0,
                content: "4x Ø6.6 THRU – 50%% ±0.1 ⌴".into(),
            }),
        );
        d.push(
            "TEXT",
            Geometry::Text(Text {
                position: Point2::new(50.0, 60.0),
                height: 2.5,
                rotation: FRAC_PI_2,
                content: "UP\nTWO".into(),
            }),
        );
        d
    }

    fn options() -> WriteOptions {
        WriteOptions {
            time: Some(1_793_000_000.0),
            paper: Some([420.0, 297.0]),
        }
    }

    #[test]
    fn a_drawing_reads_back_in_full() {
        let d = sample();
        let bytes = drawing(&d, &options()).unwrap();
        assert_eq!(&bytes[..6], b"AC1015");
        let file = crate::read(&bytes).unwrap();
        assert_eq!(file.version, Version::R2000);
        assert!(file.failed.is_empty(), "{:?}", file.failed);
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
        assert_eq!(file.codepage, 30);
        let h = file.header.as_ref().unwrap();
        assert_eq!(h.insunits, Some(4));
        assert_eq!(h.extmin.map(|p| p[2]), Some(0.0));
        assert_eq!(h.extmax.map(|p| p[0]), d.bounds().map(|b| b.1.x));
        assert_eq!(h.handseed, Some(file.objects.keys().max().unwrap() + 1));
        assert_eq!(file.classes, classes());
        let counts = file.type_counts();
        for (name, n) in [
            ("LINE", 2),
            ("ARC", 1),
            ("CIRCLE", 1),
            ("POINT", 1),
            ("ELLIPSE", 1),
            ("SPLINE", 1),
            ("TEXT", 3),
            ("BLOCK", 2),
            ("ENDBLK", 2),
            ("BLOCK_RECORD", 2),
            ("LAYER", 5),
            ("LAYOUT", 2),
            ("DICTIONARY", 5),
            ("ACDBDICTIONARYWDFLT", 1),
            ("ACDBPLACEHOLDER", 1),
            ("LTYPE", 3),
            ("STYLE", 1),
            ("DIMSTYLE", 1),
            ("VPORT", 1),
            ("APPID", 1),
            ("MLINESTYLE", 1),
        ] {
            assert_eq!(counts.get(name), Some(&n), "{name}: {counts:?}");
        }
        // The layers: 0 first, the hidden one off.
        let layers: Vec<_> = file
            .objects
            .values()
            .filter_map(|o| match &o.kind {
                Kind::Layer(l) => Some(l.clone()),
                _ => None,
            })
            .collect();
        let names: Vec<_> = layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["0", "VISIBLE", "HIDDEN", "THIN", "TEXT"]);
        assert!(layers[2].off && !layers[1].off && !layers[0].off);
        // The entities in model space, linked one after another.
        let entities: Vec<_> = file
            .objects
            .values()
            .filter_map(|o| match &o.kind {
                Kind::Entity(c, data) if c.entmode == 2 => Some((o.handle, c, data)),
                _ => None,
            })
            .collect();
        assert_eq!(entities.len(), 10);
        for pair in entities.windows(2) {
            assert_eq!(pair[0].1.next, Some(pair[1].0));
        }
        assert_eq!(entities.last().unwrap().1.next, Some(0));
        let block_header = |name: &str| {
            file.objects
                .values()
                .any(|o| matches!(&o.kind, Kind::BlockHeader(b) if b.name == name && !b.anonymous))
        };
        assert!(block_header("*Model_Space") && block_header("*Paper_Space"));
        assert!(matches!(entities[0].2, EntityData::Line { .. }));
    }

    #[test]
    fn the_geometry_reads_back_as_written() {
        let d = sample();
        let file = crate::read(&drawing(&d, &options()).unwrap()).unwrap();
        let back = crate::flat::drawing(&file);
        assert_eq!(back.units, Units::Millimeters);
        // The lines, the arc, the circle, the point and the ellipse exactly;
        // the spline is not read back; the text of two lines comes back
        // as two texts.
        assert_eq!(back.entities.len(), d.entities.len());
        assert_eq!(back.entities[..6], d.entities[..6]);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let texts: Vec<_> = back
            .entities
            .iter()
            .filter_map(|e| match &e.geometry {
                Geometry::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 3);
        assert_eq!(texts[0].content, "4x Ø6.6 THRU – 50%% ±0.1 ⌴");
        assert_eq!(texts[0].height, 3.5);
        assert_eq!(texts[1].content, "UP");
        assert_eq!(texts[2].content, "TWO");
        assert!(close(texts[1].rotation, FRAC_PI_2));
        // The second line lies 5/3 of the height left of the first (the
        // text runs upward).
        assert!(close(texts[2].position.x, 50.0 + 2.5 * 5.0 / 3.0));
        assert!(close(texts[2].position.y, 60.0));
        let layer = back.layer("HIDDEN").unwrap();
        assert!(!layer.visible);
        assert_eq!(layer.color, 8);
    }

    #[test]
    fn the_sections_have_their_checks() {
        let bytes = drawing(&sample(), &options()).unwrap();
        // The file header's CRC, five locators.
        let at = 0x19 + 9 * 5;
        let crc = crc16(0, &bytes[..at]) ^ 0x3CC4;
        assert_eq!(&bytes[at..at + 2], &crc.to_le_bytes());
        assert_eq!(&bytes[at + 2..at + 18], &FILE_HEADER_END_SENTINEL);
        // Each object's CRC covers its size and data.
        let (_, sections) = crate::container::read(&bytes).unwrap();
        let map = crate::objmap::read(&sections.handles).unwrap();
        for (&handle, &offset) in &map {
            let mut q = offset as usize;
            let size = crate::bits::modular_short(&bytes, &mut q).unwrap() as usize;
            let end = q + size;
            let crc = crc16(0xC0C1, &bytes[offset as usize..end]);
            assert_eq!(&bytes[end..end + 2], &crc.to_le_bytes(), "{handle:X}");
        }
        // The free space record names where the objects start; the preview
        // is where the file header says.
        let locator = |n: usize| {
            let at = 0x19 + 9 * n;
            u32::from_le_bytes(bytes[at + 1..at + 5].try_into().unwrap()) as usize
        };
        let free = locator(3);
        let first = *map.values().min().unwrap() as usize;
        assert_eq!(&bytes[free + 16..free + 20], &(first as u32).to_le_bytes());
        let template = locator(4);
        assert_eq!(&bytes[template..template + 4], &[0, 0, 1, 0]);
        let preview = u32::from_le_bytes(bytes[0x0D..0x11].try_into().unwrap()) as usize;
        assert_eq!(&bytes[preview..preview + 16], &PREVIEW_SENTINEL);
        assert!(bytes.ends_with(&PREVIEW_END_SENTINEL));
    }

    #[test]
    fn an_empty_drawing_is_a_drawing() {
        let bytes = drawing(&Drawing::new(Units::Inches), &WriteOptions::default()).unwrap();
        let file = crate::read(&bytes).unwrap();
        assert!(file.failed.is_empty(), "{:?}", file.failed);
        let h = file.header.unwrap();
        assert_eq!(h.insunits, Some(1));
        assert_eq!(h.extmin, Some([1e20; 3]));
    }

    #[test]
    fn invalid_geometry_is_refused() {
        let mut d = Drawing::new(Units::Millimeters);
        d.push(
            "A",
            Geometry::Circle {
                center: Point2::new(0.0, f64::NAN),
                radius: 1.0,
            },
        );
        let error = drawing(&d, &options()).unwrap_err();
        assert!(error.to_string().contains("entity 0"), "{error}");
    }

    #[test]
    fn texts_keep_their_characters() {
        for s in ["Ø6.6", "a – b", "50%% off", "⌀ 6", "x\ty", "€ ™ “q”"] {
            let bytes = encode(s);
            let latin: String = bytes.iter().map(|&b| char::from(b)).collect();
            let expected = s.replace('\t', " ");
            assert_eq!(decode(&latin), expected, "{s}");
        }
        assert_eq!(encode("⌀"), b"\\U+2300");
        assert_eq!(encode("Ø"), [0xD8]);
        assert_eq!(encode("–"), [0x96]);
    }

    #[test]
    fn julian_dates() {
        // 2000-01-01 00:00 UTC is Julian day 2451544.5: the day 2451544
        // and half of it.
        assert_eq!(julian(946_684_800.0), (2_451_544, 43_200_000));
    }
}
