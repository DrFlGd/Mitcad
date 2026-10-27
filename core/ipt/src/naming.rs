// SPDX-License-Identifier: MIT
//! The topological names features give B-rep entities (mitcad#242).
//!
//! A feature that selects edges or faces keeps them as items (`82695c37…`)
//! whose value is the node number of a reference record (`534d8790…`)
//! that names its entity by an entry of the definitions segment's naming
//! table (`4220e9cc…`). The table's entries are name records: names of
//! faces made by a feature (a list of feature nodes and the feature's own
//! numbers for the face) and names composed of other entries (an edge as
//! the faces it lies between). See `README.md`, *Topological names*.

use std::collections::HashMap;

use mitcad_asm::asm::AsmFile;
use mitcad_asm::asm::history::History;
use mitcad_asm::asm::token::Token;
use mitcad_asm::ids::View;

use crate::dc::{self, Definitions, type_id};

/// A feature's reference to a B-rep entity (`534d8790…`).
pub const REFERENCE: [u8; 16] = type_id("534d8790d011f8d10008cabc0663dc09");
/// The naming table (`4220e9cc…`).
pub const TABLE: [u8; 16] = type_id("4220e9ccd111c5710008a7ba32a3dc09");
/// An item of a feature's edge or face list (`82695c37…`).
pub const ITEM: [u8; 16] = type_id("82695c37d111516b0008a1ba32a3dc09");

/// A vertex named by the faces that meet at it (`b619c605…`).
pub const VERTEX: [u8; 16] = type_id("b619c605d1114c4a000888ba32a3dc09");
/// An edge named by its end vertices and the two faces it lies between
/// (`a6e032bf…`).
pub const EDGE: [u8; 16] = type_id("a6e032bfd111464a000888ba32a3dc09");
/// An edge a feature made, named by its ends, the faces it lies between
/// and a tag of its own (`180d36f4…`; `1c28b49b…` of a swept feature).
pub const GENERATED_EDGE: [u8; 16] = type_id("180d36f4d111424a000888ba32a3dc09");
pub const SWEPT_EDGE: [u8; 16] = type_id("1c28b49b02482ca8278b5098fed72ea8");

/// Tags: the classes of names of the faces, edges and vertices a feature
/// makes, by the feature's own numbers (see [`TagName::matches`]).
pub const TAG_GENERATED: [u8; 16] = type_id("205fa956d111af49000887ba32a3dc09");
pub const TAG_SWEPT: [u8; 16] = type_id("a5c245062846bbe61da9eda12072ce35");
pub const TAG_FLANGE: [u8; 16] = type_id("341b896fc649bafd4aaecf8eec120897");
pub const TAG_BLEND: [u8; 16] = type_id("98415d8ec341c7a5adc5f990460579a4");
pub const TAG_HOLE: [u8; 16] = type_id("7a260eaed211715d60007eb71269f1b0");
pub const TAG_CUT_ACROSS_BEND: [u8; 16] = type_id("74ed69219f48c3b041ddf394d130e1b0");

/// A decoded name of a table entry.
#[derive(Clone, Debug, PartialEq)]
pub enum Name {
    Tag(TagName),
    /// The faces that meet at a vertex (entries).
    Vertex(Vec<u32>),
    /// An edge: its end vertices and the faces it lies between (entries),
    /// and the tag of an edge a feature made.
    Edge {
        ends: [u32; 2],
        faces: Vec<u32>,
        own: Option<TagName>,
    },
    /// A name of another class, not decoded.
    Other([u8; 16]),
}

/// A tag: a name a feature gives an entity it makes.
#[derive(Clone, Debug, PartialEq)]
pub struct TagName {
    pub class: [u8; 16],
    /// The feature's node (a pattern's copy: the pattern's node, the
    /// copy's number and the copied feature's node).
    pub features: Vec<i64>,
    /// Names it is made in the context of (entries).
    pub context: Vec<u32>,
    /// The class's own numbers, as stored.
    pub fields: Vec<u8>,
}

fn u8_at(b: &[u8], at: usize) -> Option<i64> {
    b.get(at).map(|&x| i64::from(x))
}

fn u16_at(b: &[u8], at: usize) -> Option<i64> {
    b.get(at..at + 2)
        .map(|s| i64::from(u16::from_le_bytes([s[0], s[1]])))
}

fn i64_at(b: &[u8], at: usize) -> Option<i64> {
    u32_at(b, at).map(i64::from)
}

impl TagName {
    /// The naming attribute class of the B-rep entities this tag names
    /// ([`Tag::kind`]) and the numbers the attribute holds after its -1,
    /// `None` where a number is not compared (not known). The layouts
    /// *(seen: every tag the sheet-metal features of the test parts name,
    /// each matching its entity's attribute)*:
    ///
    /// - generated (`205fa956…`): u32 0, u32 the feature's number for the
    ///   entity (a sketch curve's), u32 its kind (20 a side face, 12 and 4
    ///   the end caps), u32 0; the attribute: the number, 0, the kind;
    /// - swept (`a5c24506…`): as generated, then seven u32; the attribute
    ///   the same ten numbers;
    /// - flange (`341b896f…`): bytes a, b, c, u32 d, f, e, g; the
    ///   attribute: a, b, c, d, e, f, g;
    /// - blend (`98415d8e…`): u8, u32, u32, zeros; the attribute the three
    ///   numbers and four zeros;
    /// - hole (`7a260eae…`): u32, u16; the attribute the two;
    /// - cut across a bend (`74ed6921…`): u32 0, u32, u32, u8, u32, u16;
    ///   the attribute: the second and third u32 around a 0, then 0, the
    ///   last u32, 0.
    pub fn expected(&self) -> Option<(&'static str, Vec<Option<i64>>)> {
        let b = &self.fields;
        Some(match self.class {
            TAG_GENERATED => (
                "generated",
                vec![Some(i64_at(b, 4)?), None, Some(i64_at(b, 8)?)],
            ),
            TAG_SWEPT => {
                let mut v = vec![Some(i64_at(b, 4)?), None, Some(i64_at(b, 8)?)];
                for k in 0..7 {
                    v.push(Some(i64_at(b, 12 + 4 * k)?));
                }
                ("sweep", v)
            }
            TAG_FLANGE => (
                "flange",
                vec![
                    Some(u8_at(b, 0)?),
                    Some(u8_at(b, 1)?),
                    Some(u8_at(b, 2)?),
                    Some(i64_at(b, 3)?),
                    Some(i64_at(b, 11)?),
                    Some(i64_at(b, 7)?),
                    Some(i64_at(b, 15)?),
                ],
            ),
            TAG_BLEND => (
                "blend",
                vec![
                    Some(u8_at(b, 0)?),
                    Some(i64_at(b, 1)?),
                    Some(i64_at(b, 5)?),
                    None,
                    None,
                    None,
                    None,
                ],
            ),
            TAG_HOLE => ("hole", vec![Some(i64_at(b, 0)?), Some(u16_at(b, 4)?)]),
            TAG_CUT_ACROSS_BEND => (
                "cutxbendrim",
                vec![
                    Some(i64_at(b, 4)?),
                    None,
                    Some(i64_at(b, 8)?),
                    None,
                    Some(i64_at(b, 13)?),
                    None,
                ],
            ),
            _ => return None,
        })
    }

    /// Whether a B-rep entity's naming attribute is this tag.
    pub fn matches(&self, tag: &Tag) -> bool {
        let Some((kind, fields)) = self.expected() else {
            return false;
        };
        tag.kind() == kind
            && tag.features == self.features
            && tag.fields.len() == fields.len()
            && tag
                .fields
                .iter()
                .zip(&fields)
                .all(|(a, e)| e.is_none_or(|e| *a == e))
    }
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// The naming of a part's definitions segment.
pub struct Naming<'a> {
    pub dc: &'a Definitions,
    /// Records by their header's node number.
    pub by_node: HashMap<u32, usize>,
    /// The naming table's record and its entries: index -> record.
    pub table: Option<usize>,
    pub entries: HashMap<u32, usize>,
}

impl<'a> Naming<'a> {
    pub fn new(dc: &'a Definitions) -> Naming<'a> {
        let mut by_node = HashMap::new();
        for i in 0..dc.len() {
            if let Some(h) = dc.header(i)
                && h.node != 0
            {
                by_node.entry(h.node).or_insert(i);
            }
        }
        let table = dc.of_type(&TABLE).next();
        let mut entries = HashMap::new();
        if let Some(t) = table {
            // u32, u16, u32, the next index, the count, then (index,
            // reference) pairs *(seen)*.
            let b = dc.bytes(t);
            let count = u32_at(b, 14).unwrap_or(0) as usize;
            for k in 0..count {
                let at = 18 + 8 * k;
                let (Some(i), Some(r)) = (u32_at(b, at), u32_at(b, at + 4)) else {
                    break;
                };
                if let Some(r) = dc::reference(r).filter(|&r| r < dc.len()) {
                    entries.insert(i, r);
                }
            }
        }
        Naming {
            dc,
            by_node,
            table,
            entries,
        }
    }

    /// An item's value: the node number of its reference record (after the
    /// header and the prefix a list, kind 2, of the one value, then -1 and
    /// 0) *(seen)*.
    pub fn item_value(&self, item: usize) -> Option<u32> {
        let b = self.dc.bytes(item);
        u32_at(b, b.len().checked_sub(12)?)
    }

    /// The table entry an item names: the item's value is the node number
    /// of its reference record, which ends with the table and the entry's
    /// index.
    pub fn item_entry(&self, item: usize) -> Option<u32> {
        let v = self.item_value(item)?;
        let r = *self.by_node.get(&v)?;
        if !self.dc.is(r, &REFERENCE) {
            return None;
        }
        let rb = self.dc.bytes(r);
        let n = rb.len();
        let table = dc::reference(u32_at(rb, n - 8)?)?;
        if Some(table) != self.table {
            return None;
        }
        u32_at(rb, n - 4)
    }

    /// Where a name record's own data starts: after the head (u32, u16,
    /// u32, a list, five u32, u32, u32 *(seen)*).
    fn payload_start(&self, r: usize) -> Option<usize> {
        let b = self.dc.bytes(r);
        let count = u32_at(b, 14)?;
        let mut at = 18;
        if count > 0 {
            at += 8 + 4 * count as usize;
        }
        at += 28;
        (at <= b.len()).then_some(at)
    }

    /// A table entry (the table's reference and an index) at `at`.
    fn entry_at(&self, b: &[u8], at: usize) -> Option<u32> {
        let table = self.table? as u32 + 1;
        (u32_at(b, at)? == 0x8000_0000 | table)
            .then(|| u32_at(b, at + 4))
            .flatten()
    }

    /// A list of feature nodes at `at` (kind 2: the count, its capacity,
    /// `0x10000000`, then the numbers up to the -1 that ends them; a
    /// pattern's copy is named by the pattern, the copy's number and the
    /// feature copied *(seen)*), and where its -1 ends; an empty list has
    /// no -1.
    fn features_at(b: &[u8], at: usize) -> Option<(Vec<i64>, usize)> {
        if u32_at(b, at)? != 0x3000_0002 {
            return None;
        }
        let count = u32_at(b, at + 4)?;
        if count == 0 {
            return Some((Vec::new(), at + 8));
        }
        let mut k = at + 16;
        let mut out = Vec::new();
        loop {
            let v = u32_at(b, k)?;
            k += 4;
            if v == 0xffff_ffff {
                return (!out.is_empty()).then_some((out, k));
            }
            out.push(i64::from(v));
            if out.len() > 16 {
                return None;
            }
        }
    }

    /// The decoded name of a table entry.
    pub fn name(&self, entry: u32) -> Option<Name> {
        let &r = self.entries.get(&entry)?;
        let class = self.dc.type_of(r)?;
        let b = self.dc.bytes(r);
        let start = self.payload_start(r)?;
        // A tag: the features, the -1, the count of names it is made in
        // the context of and those names, then the class's own numbers.
        if let Some((features, at)) = Self::features_at(b, start)
            && !features.is_empty()
        {
            let n = u32_at(b, at)? as usize;
            let mut k = at + 4;
            let mut context = Vec::new();
            for _ in 0..n {
                context.push(self.entry_at(b, k)?);
                k += 8;
            }
            return Some(Name::Tag(TagName {
                class: *class,
                features,
                context,
                fields: b[k..].to_vec(),
            }));
        }
        match *class {
            // A vertex: a byte 1, the count of its faces and the faces
            // *(seen)*.
            VERTEX => {
                let n = u32_at(b, start + 1)? as usize;
                let faces = (0..n)
                    .map(|i| self.entry_at(b, start + 5 + 8 * i))
                    .collect::<Option<Vec<u32>>>()?;
                Some(Name::Vertex(faces))
            }
            // An edge by its ends and the two faces it lies between, the
            // last two entries after a count 2 *(seen)*.
            EDGE => {
                let ends = [self.entry_at(b, start)?, self.entry_at(b, start + 8)?];
                let n = b.len();
                if u32_at(b, n.checked_sub(20)?)? != 2 {
                    return None;
                }
                let faces = vec![self.entry_at(b, n - 16)?, self.entry_at(b, n - 8)?];
                Some(Name::Edge {
                    ends,
                    faces,
                    own: None,
                })
            }
            // An edge a feature made: its ends, a byte 1, the feature's
            // nodes and -1, the count of the faces it lies between and the
            // faces, each with a byte; then its own tag *(seen)*.
            GENERATED_EDGE | SWEPT_EDGE => {
                let ends = [self.entry_at(b, start)?, self.entry_at(b, start + 8)?];
                let (_, at) = Self::features_at(b, start + 17)?;
                let n = u32_at(b, at)? as usize;
                let faces = (0..n)
                    .map(|i| self.entry_at(b, at + 4 + 9 * i))
                    .collect::<Option<Vec<u32>>>()?;
                // Its own tag: the features, -1, the face it is made from,
                // then the numbers of a generated (swept) tag.
                let own = Self::features_at(b, at + 4 + 9 * n).and_then(|(features, k)| {
                    let face = self.entry_at(b, k)?;
                    Some(TagName {
                        class: if *class == SWEPT_EDGE {
                            TAG_SWEPT
                        } else {
                            TAG_GENERATED
                        },
                        features,
                        context: vec![face],
                        fields: b[k + 8..].to_vec(),
                    })
                });
                Some(Name::Edge { ends, faces, own })
            }
            _ => Some(Name::Other(*class)),
        }
    }

    /// The items under a record (an edge list `ce2d9b29…` → `799c7cfa…` →
    /// a list of items, and the lists in them, at most four references
    /// deep), in the order the lists give them.
    pub fn items_under(&self, root: usize) -> Vec<usize> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        self.collect_items(root, 0, &mut seen, &mut out);
        out
    }

    fn collect_items(
        &self,
        r: usize,
        depth: usize,
        seen: &mut std::collections::HashSet<usize>,
        out: &mut Vec<usize>,
    ) {
        if depth > 4 || !seen.insert(r) {
            return;
        }
        if self.dc.is(r, &ITEM) {
            out.push(r);
            return;
        }
        let b = self.dc.bytes(r);
        let mut at = self.dc.header_len();
        while at + 4 <= b.len() {
            let v = u32_at(b, at).unwrap_or(0);
            match dc::reference(v) {
                Some(c) if v & 0x8000_0000 != 0 && c < self.dc.len() => {
                    self.collect_items(c, depth + 1, seen, out);
                    at += 4;
                }
                _ => at += 1,
            }
        }
    }

    /// The items under a feature record's slots (study).
    pub fn items_of(&self, record: usize) -> Vec<usize> {
        let mut r = self.dc.body(record);
        let Ok(slots) = (|| -> dc::Result<Vec<u32>> {
            r.i32()?;
            r.u32()?;
            Ok(r.list()?.1)
        })() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for v in slots {
            if let Some(s) = dc::reference(v).filter(|&s| s < self.dc.len()) {
                for it in self.items_under(s) {
                    if !out.contains(&it) {
                        out.push(it);
                    }
                }
            }
        }
        out
    }

    /// A name as text, for study: the record type, the feature nodes and
    /// the payload, entries of the table as nested names.
    pub fn text(&self, entry: u32, depth: usize) -> String {
        let Some(&r) = self.entries.get(&entry) else {
            return format!("#{entry}?");
        };
        let t = self.dc.type_of(r).map(dc::type_text).unwrap_or_default();
        let b = self.dc.bytes(r);
        let table_ref = self.table.map(|t| 0x8000_0000 | (t as u32 + 1));
        // The head: u32, u16, u32, a list, five u32, u32, u32 *(seen)*.
        let mut at = 10;
        let Some(count) = u32_at(b, at + 4) else {
            return format!("#{entry} {}?", &t[..8]);
        };
        at += 8;
        if count > 0 {
            at += 8 + 4 * count as usize;
        }
        at += 28;
        let mut out = format!("#{entry} {}(", &t[..8]);
        while at < b.len() {
            let w = u32_at(b, at);
            if w.is_some() && w == table_ref {
                let i = u32_at(b, at + 4).unwrap_or(0);
                if depth > 0 {
                    out.push_str(&format!(" [{}]", self.text(i, depth - 1)));
                } else {
                    out.push_str(&format!(" #{i}"));
                }
                at += 8;
            } else if w == Some(0x3000_0002) {
                let n = u32_at(b, at + 4).unwrap_or(0) as usize;
                let items: Vec<String> = (0..n)
                    .filter_map(|k| u32_at(b, at + 16 + 4 * k))
                    .map(|v| v.to_string())
                    .collect();
                out.push_str(&format!(" f{{{}}}", items.join(",")));
                at += if n > 0 { 16 + 4 * n } else { 8 };
            } else if w == Some(0xffff_ffff) && out.ends_with('}') {
                // A tag's own numbers after its feature nodes, as bytes.
                out.push_str(" -1 :");
                at += 4;
                let mut k = at;
                while k < b.len() {
                    let v = u32_at(b, k);
                    if v.is_some() && (v == table_ref || v == Some(0x3000_0002)) {
                        break;
                    }
                    out.push_str(&format!("{:02x}", b[k]));
                    k += 1;
                }
                at = k;
            } else if w == Some(0xffff_ffff) {
                out.push_str(" -1");
                at += 4;
            } else if (1..4).any(|k| {
                let v = u32_at(b, at + k);
                v.is_some() && (v == table_ref || v == Some(0x3000_0002))
            }) {
                out.push_str(&format!(" x{:02x}", b[at]));
                at += 1;
            } else if let Some(w) = w.filter(|&w| w < 0x0100_0000) {
                out.push_str(&format!(" {w}"));
                at += 4;
            } else {
                out.push_str(&format!(" x{:02x}", b[at]));
                at += 1;
            }
        }
        out.push_str(" )");
        out
    }
}

/// A naming attribute of a B-rep entity (`NMx_…-NamingMatching-attrib`):
/// its class (the first part of the type name), the feature nodes it
/// names and the numbers after them.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    pub class: String,
    pub features: Vec<i64>,
    /// The numbers after the -1 that ends the features.
    pub fields: Vec<i64>,
}

impl Tag {
    /// The attribute's class in both spellings (`NMx_Generated_Brep_tag`,
    /// `INV_NMX_GEN_TAG`): `generated`, `sweep`, `flange`, `bend`, `blend`,
    /// `hole`, `cutxbendrim`, `matched`, … (lower case otherwise).
    pub fn kind(&self) -> String {
        let c = self.class.to_ascii_lowercase();
        let c = c
            .trim_start_matches("inv_nmx_")
            .trim_start_matches("nmx_")
            .trim_end_matches("_tag")
            .trim_end_matches("_attrib")
            .trim_end_matches("tag")
            .trim_end_matches("_brep");
        match c {
            "gen" | "generated" => "generated".to_owned(),
            "sweepgenerated" | "sweep_generated" => "sweep".to_owned(),
            "matched" | "matched_entity" => "matched".to_owned(),
            "cutxbendrimface" => "cutxbendrim".to_owned(),
            other => other.to_owned(),
        }
    }
}

/// A face of a body at a history state: its entity, naming attributes and
/// edges.
#[derive(Clone, Debug)]
pub struct StateFace {
    pub entity: usize,
    pub tags: Vec<Tag>,
    pub edges: Vec<usize>,
}

/// An edge at a history state: its ends (model coordinates, cm) and its
/// naming attributes.
#[derive(Clone, Debug)]
pub struct StateEdge {
    pub ends: [[f64; 3]; 2],
    /// Its curve is a line.
    pub straight: bool,
    pub tags: Vec<Tag>,
}

/// The faces and edges of the bodies of an ASM file at a state of its
/// history, with their naming attributes.
#[derive(Clone, Debug, Default)]
pub struct BrepState {
    pub faces: Vec<StateFace>,
    pub edges: HashMap<usize, StateEdge>,
}

/// The operations to roll back to reach the bodies right after the state
/// `id`: its position in the history, newest first (the ids are not in
/// order: a part's newest state may have the id 0 *(seen)*).
pub fn steps_to(history: &History, id: i64) -> Option<usize> {
    history.states.iter().position(|s| s.id == id)
}

impl BrepState {
    /// The faces (indices into `faces`) one of whose naming attributes is
    /// the tag.
    pub fn faces_named(&self, t: &TagName) -> Vec<usize> {
        (0..self.faces.len())
            .filter(|&i| self.faces[i].tags.iter().any(|a| t.matches(a)))
            .collect()
    }

    /// The faces with an edge that ends at `p` (cm, within 1e-6).
    fn faces_at(&self, p: [f64; 3]) -> Vec<usize> {
        let near = |a: [f64; 3]| (0..3).all(|i| (a[i] - p[i]).abs() <= 1e-6);
        (0..self.faces.len())
            .filter(|&i| {
                self.faces[i].edges.iter().any(|e| {
                    self.edges
                        .get(e)
                        .is_some_and(|e| near(e.ends[0]) || near(e.ends[1]))
                })
            })
            .collect()
    }
}

impl Naming<'_> {
    /// The faces of a state a name of a face names.
    pub fn faces_of(&self, entry: u32, state: &BrepState) -> Result<Vec<usize>, String> {
        match self.name(entry) {
            Some(Name::Tag(t)) => {
                if t.expected().is_none() {
                    return Err(format!(
                        "a face named by a tag of class {} (not decoded)",
                        &dc::type_text(&t.class)[..8]
                    ));
                }
                let faces = state.faces_named(&t);
                if faces.is_empty() {
                    return Err(format!(
                        "no face of the history state has the name {} of feature {:?}",
                        &dc::type_text(&t.class)[..8],
                        t.features
                    ));
                }
                Ok(faces)
            }
            Some(Name::Other(c)) => Err(format!(
                "a face named by a name of class {} (not decoded)",
                &dc::type_text(&c)[..8]
            )),
            Some(_) => Err("a face named by a vertex or an edge".to_owned()),
            None => Err(format!("name {entry} not read")),
        }
    }

    /// The edge of a state (its entity) an edge's name names: the edges the
    /// two faces it lies between share; where they share more than one, the
    /// one whose ends are the vertices its name names (by the faces that
    /// meet there).
    pub fn resolve_edge(&self, entry: u32, state: &BrepState) -> Result<usize, String> {
        let Some(Name::Edge { ends, faces, own }) = self.name(entry) else {
            return Err(match self.name(entry) {
                Some(Name::Other(c)) => format!(
                    "an edge named by a name of class {} (not decoded)",
                    &dc::type_text(&c)[..8]
                ),
                Some(_) => "the name is not an edge's".to_owned(),
                None => format!("name {entry} not read"),
            });
        };
        // An edge a feature made and that is still whole: the one edge with
        // its tag.
        if let Some(own) = &own {
            let tagged: Vec<usize> = state
                .edges
                .iter()
                .filter(|(_, e)| e.tags.iter().any(|t| own.matches(t)))
                .map(|(k, _)| *k)
                .collect();
            if let [e] = tagged.as_slice() {
                return Ok(*e);
            }
        }
        let [a, b] = faces.as_slice() else {
            return Err(format!(
                "an edge between {} faces, its own name on no single edge",
                faces.len()
            ));
        };
        let (fa, fb) = (self.faces_of(*a, state)?, self.faces_of(*b, state)?);
        let mut shared: Vec<usize> = Vec::new();
        for &i in &fa {
            for &j in &fb {
                if i == j {
                    continue;
                }
                for e in &state.faces[i].edges {
                    if state.faces[j].edges.contains(e) && !shared.contains(e) {
                        shared.push(*e);
                    }
                }
            }
        }
        if shared.len() > 1 {
            // The vertices: each a set of faces that must all meet there.
            let vertex = |v: u32| -> Option<Vec<Vec<usize>>> {
                let Some(Name::Vertex(faces)) = self.name(v) else {
                    return None;
                };
                faces
                    .iter()
                    .map(|f| self.faces_of(*f, state).ok())
                    .collect()
            };
            if let (Some(v0), Some(v1)) = (vertex(ends[0]), vertex(ends[1])) {
                let at = |p: [f64; 3], v: &[Vec<usize>]| {
                    let here = state.faces_at(p);
                    v.iter().all(|f| f.iter().any(|f| here.contains(f)))
                };
                shared.retain(|e| {
                    let Some(edge) = state.edges.get(e) else {
                        return false;
                    };
                    let [p, q] = edge.ends;
                    (at(p, &v0) && at(q, &v1)) || (at(p, &v1) && at(q, &v0))
                });
            }
        }
        match shared.as_slice() {
            [e] => Ok(*e),
            [] => Err("its faces share no edge in the history state".to_owned()),
            more => Err(format!(
                "its faces share {} edges in the history state",
                more.len()
            )),
        }
    }
}

/// Reads an ASM file's entities at a state (a history view).
struct At<'a> {
    file: &'a AsmFile,
    view: &'a View,
}

impl At<'_> {
    /// The record holding entity pointer `p`'s data at the state.
    fn rec(&self, p: i64) -> Option<usize> {
        if p < 0 {
            return None;
        }
        let e = self.file.record_of(p)?;
        match self.view.get(&e) {
            Some(r) => *r,
            None => Some(e),
        }
    }

    fn fields(&self, r: usize) -> &[Token] {
        self.file.fields(r)
    }

    fn ptr(&self, r: usize, i: usize) -> Option<i64> {
        match self.fields(r).get(i) {
            Some(Token::Ptr(p)) => Some(*p),
            _ => None,
        }
    }

    /// The `i`th pointer among a record's fields.
    fn nth_ptr(&self, r: usize, i: usize) -> Option<i64> {
        self.fields(r)
            .iter()
            .filter_map(|t| match t {
                Token::Ptr(p) => Some(*p),
                _ => None,
            })
            .nth(i)
    }

    fn type_of(&self, r: usize) -> &str {
        &self.file.records[r].type_name
    }

    /// The naming attributes of an entity's record: its attribute chain
    /// (field 0, then each attribute's field 2).
    fn tags(&self, r: usize) -> Vec<Tag> {
        let mut out = Vec::new();
        let mut at = self.ptr(r, 0).and_then(|p| self.rec(p));
        let mut guard = 0;
        while let Some(a) = at {
            guard += 1;
            if guard > 64 {
                break;
            }
            let t = self.type_of(a);
            // `NMx_…-NamingMatching-attrib`: 43, the count of feature nodes,
            // the nodes up to a -1, then the class's numbers; from ASM 229
            // the same after the class's name as a text and a 1
            // (`ATTRIB_CUSTOM-attrib "INV_NMX_GEN_TAG" 1 44 …`) *(seen)*.
            let (class, skip) = if t.ends_with("NamingMatching-attrib") {
                (Some(t.split('-').next().unwrap_or(t).to_owned()), 0)
            } else if t == "ATTRIB_CUSTOM-attrib" {
                let name = self.fields(a).iter().find_map(|t| match t {
                    Token::Str(s) | Token::Literal(s) => Some(s.clone()),
                    _ => None,
                });
                (name.filter(|n| n.starts_with("INV_NMX_")), 1)
            } else {
                (None, 0)
            };
            if let Some(class) = class {
                let ints: Vec<i64> = self
                    .fields(a)
                    .iter()
                    .skip(5)
                    .filter_map(|t| match t {
                        Token::Int(v) | Token::Enum(v) => Some(*v),
                        _ => None,
                    })
                    .skip(skip)
                    .collect();
                if let Some(end) = ints.iter().skip(2).position(|&v| v == -1) {
                    out.push(Tag {
                        class,
                        features: ints[2..2 + end].to_vec(),
                        fields: ints[3 + end..].to_vec(),
                    });
                }
            }
            at = self.ptr(a, 2).and_then(|p| self.rec(p));
        }
        out
    }

    fn point(&self, vertex: i64) -> Option<[f64; 3]> {
        let v = self.rec(vertex)?;
        let p = self.rec(self.nth_ptr(v, 3)?)?;
        self.fields(p).iter().find_map(|t| match t {
            Token::Pos(x) => Some(*x),
            _ => None,
        })
    }
}

impl BrepState {
    /// The bodies of `file` after rolling back the newest `steps` states of
    /// its history (`history` none: as stored).
    pub fn read(file: &AsmFile, history: Option<&History>, steps: usize) -> BrepState {
        let view = history.map(|h| h.view(steps)).unwrap_or_default();
        let at = At { file, view: &view };
        let mut out = BrepState::default();
        // The top-level bodies at the state, and the bodies a later
        // operation deleted, their copies standing for them.
        // (As the history's bodies are converted: `deleted_bodies`, the
        // `history` command of `mitcad-ipt-inspect`.)
        let mut bodies: Vec<usize> = mitcad_asm::convert::body_records(file)
            .into_iter()
            .filter(|(_, top)| *top)
            .map(|(b, _)| b)
            .collect();
        if let Some(h) = history {
            bodies.extend(
                crate::deleted_bodies(h, file)
                    .into_iter()
                    .filter(|&(_, j)| j < steps)
                    .map(|(copy, _)| copy),
            );
        }
        for entity in bodies {
            let body = match view.get(&entity) {
                Some(None) => continue,
                Some(Some(r)) => *r,
                None => entity,
            };
            if at.type_of(body) != "body" {
                continue;
            }
            // Its transform (field 5): the images of the axes, the
            // translation and a scale, row-vector form (as the converter's).
            let transform = at.ptr(body, 5).and_then(|t| at.rec(t)).and_then(|t| {
                let v: Vec<[f64; 3]> = at
                    .fields(t)
                    .iter()
                    .filter_map(|x| match x {
                        Token::Pos(p) | Token::Vector(p) => Some(*p),
                        _ => None,
                    })
                    .collect();
                let scale = at.fields(t).iter().find_map(|x| match x {
                    Token::Double(s) => Some(*s),
                    _ => None,
                })?;
                (v.len() >= 4).then(|| (v[0], v[1], v[2], v[3], scale))
            });
            let place = |p: [f64; 3]| match transform {
                Some((a, b, d, t, s)) => {
                    [0, 1, 2].map(|i| s * (p[0] * a[i] + p[1] * b[i] + p[2] * d[i]) + t[i])
                }
                None => p,
            };
            // body: attrib, tag, ?, lump; lump: …, next, shell; shell: …,
            // next, subshell, face; face: …, next, loop; loop: …, next,
            // coedge; coedge: …, next, previous, partner, edge.
            let mut lump = at.ptr(body, 3).and_then(|p| at.rec(p));
            let mut guard = 0usize;
            while let Some(l) = lump {
                let mut shell = at.ptr(l, 4).and_then(|p| at.rec(p));
                while let Some(s) = shell {
                    let mut face_p = at.ptr(s, 5);
                    while let Some(fp) = face_p.filter(|&p| p >= 0) {
                        guard += 1;
                        if guard > 1_000_000 {
                            return out;
                        }
                        let Some(f) = at.rec(fp) else { break };
                        let mut edges = Vec::new();
                        let mut lp = at.ptr(f, 4).and_then(|p| at.rec(p));
                        while let Some(lr) = lp {
                            let first = at.ptr(lr, 4);
                            let mut cp = first;
                            let mut n = 0;
                            while let Some(c) = cp.filter(|&p| p >= 0) {
                                let Some(cr) = at.rec(c) else { break };
                                if let Some(ep) = at.ptr(cr, 6)
                                    && let Some(e) = file.record_of(ep)
                                {
                                    edges.push(e);
                                    if let std::collections::hash_map::Entry::Vacant(v) =
                                        out.edges.entry(e)
                                        && let Some(er) = at.rec(ep)
                                    {
                                        let ends = [
                                            at.ptr(er, 3).and_then(|v| at.point(v)),
                                            at.ptr(er, 5).and_then(|v| at.point(v)),
                                        ];
                                        let straight = at
                                            .ptr(er, 8)
                                            .and_then(|c| at.rec(c))
                                            .is_some_and(|c| at.type_of(c) == "straight-curve");
                                        if let [Some(a), Some(b)] = ends {
                                            v.insert(StateEdge {
                                                ends: [place(a), place(b)],
                                                straight,
                                                tags: at.tags(er),
                                            });
                                        }
                                    }
                                }
                                cp = at.ptr(cr, 3);
                                n += 1;
                                if cp == first || n > 100_000 {
                                    break;
                                }
                            }
                            lp = at.ptr(lr, 3).and_then(|p| at.rec(p));
                        }
                        out.faces.push(StateFace {
                            entity: file.record_of(fp).unwrap_or(f),
                            tags: at.tags(f),
                            edges,
                        });
                        face_p = at.ptr(f, 3);
                    }
                    shell = at.ptr(s, 3).and_then(|p| at.rec(p));
                }
                lump = at.ptr(l, 3).and_then(|p| at.rec(p));
            }
        }
        out
    }
}

/// The B-rep record with its ASM history, whose states the names are
/// resolved in; each state read once, when first asked for.
pub struct Brep {
    file: AsmFile,
    history: Option<History>,
    states: std::cell::RefCell<HashMap<i64, std::rc::Rc<BrepState>>>,
}

impl Brep {
    /// The part's B-rep record that has a history (else the first).
    pub fn read(file: &crate::IptFile) -> Option<Brep> {
        let records = file.brep_records().ok()?;
        let mut first = None;
        for r in &records {
            let Ok(asm) = AsmFile::parse(&r.asm) else {
                continue;
            };
            match History::parse(&asm) {
                Ok(Some(h)) => {
                    return Some(Brep {
                        file: asm,
                        history: Some(h),
                        states: Default::default(),
                    });
                }
                _ => {
                    if first.is_none() {
                        first = Some(asm);
                    }
                }
            }
        }
        first.map(|file| Brep {
            file,
            history: None,
            states: Default::default(),
        })
    }

    /// The bodies right after the history state `id`; `None` when the
    /// history has no such state.
    pub fn state(&self, id: i64) -> Option<std::rc::Rc<BrepState>> {
        if let Some(s) = self.states.borrow().get(&id) {
            return Some(s.clone());
        }
        let steps = steps_to(self.history.as_ref()?, id)?;
        let s = std::rc::Rc::new(BrepState::read(&self.file, self.history.as_ref(), steps));
        self.states.borrow_mut().insert(id, s.clone());
        Some(s)
    }
}

/// An edge of a history state as the dump's edge reference (a
/// fingerprint: its ends, middle and length, cm), by which the import
/// finds the edge of its own replay.
pub fn edge_reference(edge: &StateEdge) -> serde_json::Value {
    let [a, b] = edge.ends;
    let mid = [0, 1, 2].map(|i| (a[i] + b[i]) / 2.0);
    let length = ((0..3).map(|i| (b[i] - a[i]).powi(2)).sum::<f64>()).sqrt();
    serde_json::json!({
        "kind": "edge",
        "objectType": "BRepEdge",
        "start_point": a,
        "end_point": b,
        "mid_point": mid,
        "length": length,
    })
}

/// What the decoder resolves features' edges with: the definitions'
/// names, the history state table and the part's B-rep record (read when
/// first asked for).
pub struct Resolver<'a> {
    pub naming: Naming<'a>,
    states: &'a HashMap<u32, Vec<i64>>,
    file: Option<&'a crate::IptFile>,
    brep: std::cell::OnceCell<Option<Brep>>,
}

impl<'a> Resolver<'a> {
    /// `file` none (definitions made by tests): no edge is resolved.
    pub fn new(
        dc: &'a Definitions,
        states: &'a HashMap<u32, Vec<i64>>,
        file: Option<&'a crate::IptFile>,
    ) -> Resolver<'a> {
        Resolver {
            naming: Naming::new(dc),
            states,
            file,
            brep: std::cell::OnceCell::new(),
        }
    }

    /// The history state a feature record starts from (the state table's
    /// first state of its node).
    pub fn before(&self, record: usize) -> Option<i64> {
        let node = self.naming.dc.header(record)?.node;
        self.states.get(&node)?.first().copied()
    }

    /// The edges the items under `root` name, each found in the history
    /// state before `feature` (see [`Naming::edges`]).
    pub fn edges_under(
        &self,
        root: usize,
        feature: usize,
    ) -> Vec<Result<serde_json::Value, String>> {
        let items = self.naming.items_under(root);
        self.edges_of(&items, feature)
    }

    /// The edges the items name, found in the history state before
    /// `feature`.
    pub fn edges_of(
        &self,
        items: &[usize],
        feature: usize,
    ) -> Vec<Result<serde_json::Value, String>> {
        let brep = self
            .brep
            .get_or_init(|| self.file.and_then(Brep::read))
            .as_ref();
        self.naming.edges(items, brep, self.before(feature))
    }
}

impl Naming<'_> {
    /// The edges the items name, in the items' order, each as the dump's
    /// edge reference found in the history state `before` (the state the
    /// feature starts from), or why it was not found.
    pub fn edges(
        &self,
        items: &[usize],
        brep: Option<&Brep>,
        before: Option<i64>,
    ) -> Vec<Result<serde_json::Value, String>> {
        let state = brep.zip(before).and_then(|(b, s)| b.state(s));
        items
            .iter()
            .map(|&item| {
                let entry = self
                    .item_entry(item)
                    .ok_or_else(|| format!("item {item}: its name was not read"))?;
                let state = state
                    .as_ref()
                    .ok_or("the history state before the feature was not read")?;
                let e = self.resolve_edge(entry, state)?;
                let edge = &state.edges[&e];
                if !edge.straight {
                    return Err("its edge is not straight".to_owned());
                }
                Ok(edge_reference(edge))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEATURE: u32 = 175;

    fn u32s(v: &[u32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    /// A record's common header (major 24): value, id, next, flags, the
    /// context and the node.
    fn header(node: u32) -> Vec<u8> {
        let mut b = 0u32.to_le_bytes().to_vec();
        b.extend(7u16.to_le_bytes());
        b.extend(u32s(&[0, 0, 0x8000_0001, node]));
        b
    }

    /// A name record: its head, then `payload`.
    fn name_record(payload: &[u8]) -> Vec<u8> {
        let mut b = 0u32.to_le_bytes().to_vec();
        b.extend(7u16.to_le_bytes());
        b.extend(u32s(&[1, 0x3000_0002, 0, 0, 0, 0, 0, 0, 180, 0]));
        b.extend_from_slice(payload);
        b
    }

    /// The table is record 1 (reference `0x80000002`).
    fn entry(i: u32) -> Vec<u8> {
        u32s(&[0x8000_0002, i])
    }

    fn features() -> Vec<u8> {
        u32s(&[0x3000_0002, 1, 1, 0x1000_0000, FEATURE, 0xffff_ffff])
    }

    /// A generated tag: the feature, no context, then 0, the number, the
    /// kind, 0.
    fn generated(number: u32, kind: u32) -> Vec<u8> {
        let mut b = features();
        b.extend(u32s(&[0, 0, number, kind, 0]));
        name_record(&b)
    }

    /// Definitions of: 0 the root, 1 the table, 2.. the names (entries 1..),
    /// then two reference records and their items.
    fn definitions() -> (Definitions, usize, usize) {
        let mut names = vec![
            generated(8, 20), // 1: a side face
            generated(0, 12), // 2: the cap
            generated(6, 20), // 3: another side face
        ];
        // 4: the edge between 1 and 2 by its faces.
        let mut edge = entry(1);
        edge.extend(entry(1));
        edge.push(1);
        edge.extend(u32s(&[0x3000_0002, 0]));
        edge.push(0);
        edge.extend(u32s(&[2]));
        edge.extend(entry(1));
        edge.extend(entry(2));
        names.push(name_record(&edge));
        // 5: the edge the feature made between 3 and 2, with its own tag.
        let mut made = entry(1);
        made.extend(entry(1));
        made.push(1);
        made.extend(features());
        made.extend(u32s(&[2]));
        made.extend(entry(3));
        made.push(5);
        made.extend(entry(2));
        made.push(3);
        made.extend(features());
        made.extend(entry(3));
        made.extend(u32s(&[0, 6, 10]));
        names.push(name_record(&made));
        let first = 2;
        // The table: u32, u16, u32, the next index, the count, the pairs.
        let mut table = 0u32.to_le_bytes().to_vec();
        table.extend(7u16.to_le_bytes());
        table.extend(u32s(&[0, names.len() as u32 + 1, names.len() as u32]));
        for (k, _) in names.iter().enumerate() {
            table.extend(u32s(&[k as u32 + 1, 0x8000_0000 | (first + k + 1) as u32]));
        }
        let mut records: Vec<([u8; 16], Vec<u8>)> = vec![([0; 16], header(1)), (TABLE, table)];
        let types = [
            TAG_GENERATED,
            TAG_GENERATED,
            TAG_GENERATED,
            EDGE,
            GENERATED_EDGE,
        ];
        for (t, n) in types.into_iter().zip(names) {
            records.push((t, n));
        }
        // References (nodes 900, 901) to entries 4 and 5, and their items.
        let mut items = Vec::new();
        for (node, e) in [(900u32, 4u32), (901, 5)] {
            let mut r = header(node);
            r.extend(u32s(&[0xffff_ffff, 0]));
            r.extend(u32s(&[0x8000_0002, e]));
            records.push((REFERENCE, r));
            let mut it = header(0);
            it.extend(u32s(&[0xffff_ffff, 0x3000_0002, 1, 1, 0x1000_0000, node]));
            it.extend(u32s(&[0xffff_ffff, 0]));
            records.push((ITEM, it));
            items.push(records.len() - 1);
        }
        (Definitions::from_records(24, &records), items[0], items[1])
    }

    fn tag(fields: &[i64]) -> Tag {
        Tag {
            class: "NMx_Generated_Brep_tag".to_owned(),
            features: vec![i64::from(FEATURE)],
            fields: fields.to_vec(),
        }
    }

    /// Three faces of a plate: side 8 and side 6 each meet the cap.
    fn state() -> BrepState {
        let edge = |a: [f64; 3], b: [f64; 3], tags: Vec<Tag>| StateEdge {
            ends: [a, b],
            straight: true,
            tags,
        };
        BrepState {
            faces: vec![
                StateFace {
                    entity: 1,
                    tags: vec![tag(&[8, 0, 20])],
                    edges: vec![11, 13],
                },
                StateFace {
                    entity: 2,
                    tags: vec![tag(&[0, 0, 12])],
                    edges: vec![11, 12],
                },
                StateFace {
                    entity: 3,
                    tags: vec![tag(&[6, 0, 20])],
                    edges: vec![12, 13],
                },
            ],
            edges: HashMap::from([
                (11, edge([0.0; 3], [1.0, 0.0, 0.0], vec![tag(&[8, 0, 10])])),
                (12, edge([0.0; 3], [0.0, 1.0, 0.0], vec![tag(&[6, 0, 10])])),
                (13, edge([0.0; 3], [0.0, 0.0, 1.0], vec![])),
            ]),
        }
    }

    /// An item's name is its reference record's table entry; an edge
    /// between two faces is the edge they share, an edge a feature made is
    /// the edge with its own tag.
    #[test]
    fn items_name_the_edges_of_a_state() {
        let (dc, by_faces, by_tag) = definitions();
        let naming = Naming::new(&dc);
        assert_eq!(naming.item_entry(by_faces), Some(4));
        assert_eq!(naming.item_entry(by_tag), Some(5));
        let Some(Name::Edge { faces, own, .. }) = naming.name(5) else {
            panic!("not an edge");
        };
        assert_eq!(faces, [3, 2]);
        assert_eq!(own.map(|t| t.features), Some(vec![i64::from(FEATURE)]));
        let s = state();
        assert_eq!(naming.resolve_edge(4, &s), Ok(11));
        assert_eq!(naming.resolve_edge(5, &s), Ok(12));
        // Without the side face, no edge.
        let mut lost = state();
        lost.faces.remove(0);
        assert!(naming.resolve_edge(4, &lost).is_err());
        let refs = naming.edges(&[by_faces], None, None);
        assert!(refs[0].is_err(), "no state, no edge");
    }

    /// The attribute classes in both spellings; a flange tag's numbers as
    /// stored.
    #[test]
    fn tags_match_in_both_spellings() {
        for (class, kind) in [
            ("NMx_Generated_Brep_tag", "generated"),
            ("INV_NMX_GEN_TAG", "generated"),
            ("NMx_Sweep_Generated_Brep_tag", "sweep"),
            ("INV_NMX_SWEEPGENERATED_TAG", "sweep"),
            ("NMx_flange_tag", "flange"),
            ("NMx_Hole_Brep_tag", "hole"),
            ("INV_NMX_MATCHED_ATTRIB", "matched"),
        ] {
            let t = Tag {
                class: class.to_owned(),
                features: vec![],
                fields: vec![],
            };
            assert_eq!(t.kind(), kind, "{class}");
        }
        // a, b, c, u32 d, f, e, g
        let mut fields = vec![0, 0, 1];
        fields.extend(273u32.to_le_bytes());
        fields.extend(0u32.to_le_bytes());
        fields.extend(8u32.to_le_bytes());
        fields.extend(0x0400_0004u32.to_le_bytes());
        let name = TagName {
            class: TAG_FLANGE,
            features: vec![1261],
            context: vec![],
            fields,
        };
        let attribute = Tag {
            class: "NMx_flange_tag".to_owned(),
            features: vec![1261],
            fields: vec![0, 0, 1, 273, 8, 0, 0x0400_0004],
        };
        assert!(name.matches(&attribute));
        let other = Tag {
            fields: vec![0, 0, 2, 273, 8, 0, 0x0400_0004],
            ..attribute
        };
        assert!(!name.matches(&other));
    }
}
