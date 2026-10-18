// SPDX-License-Identifier: MIT
//! Sketch texts (mitcad#414): the text record of a sketch, its frame, the
//! glyph contours the file keeps with it, and the part's font table.
//!
//! - A text (`013cbb1f…`, in the sketch's list as an entity): the header,
//!   the prefix, its anchor (x, y, z: f64, cm), its angle (f64, radians),
//!   its alignment (u32: the low two bits 1 left, 2 centre, 3 right; the
//!   rest 2 for the frame's top *(seen: 9 and 10, the anchor the frame's
//!   top left corner or the middle of its top side, in the text's own
//!   axes)*), a u32, 21 bytes, an f64 (0.065 in every text seen), the
//!   string (a text whose font changes are codes `\F<n>;` naming entries
//!   of the font table; lines break at `\n`), the font table entry the
//!   text starts with (u32), three u32, an empty list, the list of its
//!   glyph contours, its frame's box in its own axes (two points x, y, z:
//!   the anchor's x to the anchor's x plus the frame's width, the
//!   anchor's y less the frame's height to the anchor's y), a u32, three
//!   bytes, the sketch, an empty list and the string again *(seen: all 9
//!   texts of the corpus's 4 parts with texts)*.
//! - Its frame (`7f341025…`, in the sketch's list as a constraint): the
//!   header, the prefix, the corner point of the frame's construction
//!   rectangle at the text's top left (in its own axes) and the text
//!   *(seen: every text in a frame)*.
//! - A glyph contour (`cb63f719…`): a header of 22 bytes whose last u32 is
//!   the count of its pieces, the pieces, the sketch and the pen position
//!   of its character (x, y, z: model coordinates, cm, on the baseline).
//!   A piece is a u32 kind and its curve: 0x2a a B-spline (two u32 0, its
//!   NURBS data as a sketch spline's, with points x, y, z, then an f64,
//!   two u32 and its parameter range), 0x17 a line (its start and its
//!   vector to the end, x, y, z) *(seen: the quadratic pieces of the
//!   font's outlines, each contour closed)*.
//! - The font table (`01691837…`, in the application segment): u32, u16,
//!   a list (kind 2) of entries: the index (u32), a u16, the weight (u32,
//!   400 regular, 700 bold), the size (f32, cm), an f32, the family (a
//!   text) and 11 bytes *(seen: 17 to 28 entries a part)*.

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use crate::IptFile;
use crate::dc::{self, Definitions, Reader, type_id};

pub const TEXT: [u8; 16] = type_id("013cbb1fd2114d68a0009e9a3c3aa076");
pub const FRAME: [u8; 16] = type_id("7f341025ae4a79961d9047869d5f4e48");
pub const CONTOUR: [u8; 16] = type_id("cb63f7192d4e0e7c751b79aaf2ded1d7");
pub const FONT_TABLE: [u8; 16] = type_id("01691837bd45b82ba3cffb9bf865faf1");

/// An entry of the part's font table.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    pub index: u32,
    pub family: String,
    /// 400 regular, 700 bold.
    pub weight: u16,
    pub italic: bool,
    /// The text's height, cm: the height of the capital letters
    /// *(verified on the glyph contours the file keeps)*.
    pub size: f64,
    /// The characters' width factor (1, and 0 in some entries, for the
    /// font's own width).
    pub width: f64,
}

/// An f32 as the shortest decimal that gives it back (0.15, not
/// 0.15000000596).
fn f32_value(r: &mut Reader) -> Option<f64> {
    let v = f32::from_le_bytes(r.take(4).ok()?.try_into().ok()?);
    v.to_string().parse().ok()
}

/// The part's font table (empty when the file has none).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fonts(pub Vec<Font>);

impl Fonts {
    /// The font table of a part's application segment.
    pub fn read(file: &IptFile) -> Fonts {
        let Some(segment) = file.segments.iter().find(|s| s.name == "PmAppSegment") else {
            return Fonts::default();
        };
        let Ok(content) = file.segment_data(segment) else {
            return Fonts::default();
        };
        let Ok(records) = &content.records else {
            return Fonts::default();
        };
        records
            .iter()
            .find(|r| content.tables.types[r.type_index()].id == FONT_TABLE)
            .and_then(|r| Fonts::parse(&content.data[r.range.clone()]))
            .unwrap_or_default()
    }

    /// A font table record's entries; None when it is not laid out so.
    pub fn parse(bytes: &[u8]) -> Option<Fonts> {
        let mut r = Reader::new(bytes);
        r.u32().ok()?;
        r.u16().ok()?;
        let (kind, tag, n) = (r.u16().ok()?, r.u16().ok()?, r.u32().ok()?);
        if kind != 2 || tag != 0x3000 || n > 1000 {
            return None;
        }
        if n > 0 {
            r.skip(8).ok()?;
        }
        let mut fonts = Vec::new();
        for _ in 0..n {
            let index = r.u32().ok()?;
            r.u16().ok()?;
            let weight = r.u16().ok()?;
            let italic = r.u8().ok()? != 0;
            r.u8().ok()?;
            let size = f32_value(&mut r)?;
            let width = f32_value(&mut r)?;
            let family = r.text().ok()?;
            r.skip(11).ok()?;
            if !(1..=1000).contains(&weight) || !(size.is_finite() && size > 0.0) {
                return None;
            }
            fonts.push(Font {
                index,
                family,
                weight,
                italic,
                size,
                width,
            });
        }
        Some(Fonts(fonts))
    }

    pub fn get(&self, index: u32) -> Option<&Font> {
        self.0.iter().find(|f| f.index == index)
    }
}

/// A piece of a glyph contour, in model coordinates (cm).
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Spline {
        degree: u32,
        knots: Vec<f64>,
        poles: Vec<[f64; 3]>,
    },
    Line {
        start: [f64; 3],
        end: [f64; 3],
    },
}

impl Piece {
    pub fn start(&self) -> [f64; 3] {
        match self {
            Piece::Spline { poles, .. } => poles[0],
            Piece::Line { start, .. } => *start,
        }
    }

    pub fn end(&self) -> [f64; 3] {
        match self {
            Piece::Spline { poles, .. } => poles[poles.len() - 1],
            Piece::Line { end, .. } => *end,
        }
    }

    /// Its control points (a line's two ends).
    pub fn points(&self) -> Vec<[f64; 3]> {
        match self {
            Piece::Spline { poles, .. } => poles.clone(),
            Piece::Line { start, end } => vec![*start, *end],
        }
    }
}

/// A closed contour of a character's glyph, as the file keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Contour {
    pub pieces: Vec<Piece>,
    /// The pen position of its character, on the baseline (model, cm).
    pub pen: [f64; 3],
}

/// A sketch text as stored.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    /// The point the alignment refers to (sketch, cm).
    pub anchor: [f64; 2],
    pub angle: f64,
    pub alignment: u32,
    /// The string with its font codes.
    pub content: String,
    /// The font table entry it starts with.
    pub font: u32,
    pub contours: Vec<usize>,
    /// The frame's width and height (cm).
    pub size: [f64; 2],
}

/// A text record.
pub fn read(dc: &Definitions, record: usize) -> Result<Text, String> {
    let mut r = dc.fields(record);
    (|| -> dc::Result<Text> {
        let (x, y) = (r.f64()?, r.f64()?);
        r.f64()?;
        let angle = r.f64()?;
        r.u32()?;
        let alignment = r.u32()?;
        r.u32()?;
        r.skip(21)?;
        r.f64()?;
        let content = r.text()?;
        let font = r.u32()?;
        r.skip(12)?;
        r.list()?;
        let contours = r.references()?;
        let lo = [r.f64()?, r.f64()?, r.f64()?];
        let hi = [r.f64()?, r.f64()?, r.f64()?];
        Ok(Text {
            anchor: [x, y],
            angle,
            alignment,
            content,
            font,
            contours,
            size: [hi[0] - lo[0], hi[1] - lo[1]],
        })
    })()
    .map_err(|e| format!("text {record}: {e}"))
    .and_then(|t| {
        if t.anchor.iter().chain(&t.size).all(|v| v.is_finite()) && t.angle.is_finite() {
            Ok(t)
        } else {
            Err(format!("text {record}: not a finite placement"))
        }
    })
}

/// A glyph contour record.
pub fn contour(dc: &Definitions, record: usize) -> Result<Contour, String> {
    let bytes = dc.bytes(record);
    (|| -> dc::Result<Contour> {
        let head = dc.header_len();
        let mut r = Reader::at(bytes, head - 4);
        let n = r.u32()? as usize;
        if n > r.left() / 52 {
            return Err(dc::DcError(format!("{n} pieces")));
        }
        let point = |r: &mut Reader| -> dc::Result<[f64; 3]> { Ok([r.f64()?, r.f64()?, r.f64()?]) };
        let mut pieces = Vec::with_capacity(n);
        for _ in 0..n {
            let at = r.left();
            match r.u32()? {
                0x2a => {
                    r.u32()?;
                    r.u32()?;
                    let degree = r.u32()?;
                    r.f64()?;
                    let nk = r.u32()? as usize;
                    r.u32()?;
                    r.u32()?;
                    if nk > r.left() / 8 {
                        return Err(dc::DcError(format!("{nk} knots")));
                    }
                    let knots = (0..nk).map(|_| r.f64()).collect::<dc::Result<Vec<_>>>()?;
                    r.skip(8)?;
                    r.u32()?;
                    let np = r.u32()? as usize;
                    r.u32()?;
                    r.u32()?;
                    if np < 2 || np > r.left() / 24 || np + degree as usize + 1 != nk {
                        return Err(dc::DcError(format!(
                            "a spline of degree {degree}, {np} poles and {nk} knots"
                        )));
                    }
                    let poles = (0..np)
                        .map(|_| point(&mut r))
                        .collect::<dc::Result<Vec<_>>>()?;
                    r.f64()?;
                    r.u32()?;
                    r.u32()?;
                    r.f64()?;
                    r.f64()?;
                    pieces.push(Piece::Spline {
                        degree,
                        knots,
                        poles,
                    });
                }
                0x17 => {
                    let start = point(&mut r)?;
                    let v = point(&mut r)?;
                    let end = [start[0] + v[0], start[1] + v[1], start[2] + v[2]];
                    pieces.push(Piece::Line { start, end });
                }
                k => {
                    return Err(dc::DcError(format!(
                        "a piece of kind {k:#x} at {}",
                        bytes.len() - at
                    )));
                }
            }
        }
        r.u32()?;
        let pen = point(&mut r)?;
        Ok(Contour { pieces, pen })
    })()
    .map_err(|e| format!("glyph contour {record}: {e}"))
}

/// A model point (cm) in the coordinates of a sketch whose transform is
/// `m` (sketch to model, a rotation and a translation).
pub fn to_sketch(m: &[[f64; 4]; 4], p: [f64; 3]) -> [f64; 2] {
    let d = [p[0] - m[0][3], p[1] - m[1][3], p[2] - m[2][3]];
    let dot = |c: usize| d[0] * m[0][c] + d[1] * m[1][c] + d[2] * m[2][c];
    [dot(0), dot(1)]
}

/// What [`sketch_texts`] made of a sketch's texts.
#[derive(Debug, Default)]
pub struct SketchTexts {
    /// The dump's texts (SCHEMA.md `Text`).
    pub texts: Vec<Value>,
    /// The text and frame records they came from.
    pub done: HashSet<usize>,
    /// Texts that were left out, with why.
    pub dropped: Vec<(usize, String)>,
}

/// The texts of a sketch whose transform is `m`, as the dump's texts: the
/// string, the font table entry's family, weight and size (the capitals'
/// height), the frame (its construction rectangle through the frame's
/// corner point, `ids` and `curves` the sketch's decoded points and
/// curves), the alignment in it, and the spacing of the lines from the
/// baselines of the glyph contours the file keeps, and the characters'
/// glyphs from those contours ([`char_glyphs`], mitcad#416). Lines align by
/// their pens and the frame's top meets the first line's highest outline
/// point *(seen: the glyph contours of the corpus's texts lie so)*. A text in
/// several fonts, or without a frame, is left out *(neither seen with
/// glyph contours)*.
pub fn sketch_texts(
    dc: &Definitions,
    entities: &[usize],
    ids: &HashMap<usize, String>,
    curves: &[Value],
    m: &[[f64; 4]; 4],
    fonts: &Fonts,
) -> SketchTexts {
    let mut out = SketchTexts::default();
    // Frames by their text: (frame record, corner point).
    let mut frames: HashMap<usize, (usize, usize)> = HashMap::new();
    for &e in entities.iter().filter(|&&e| dc.is(e, &FRAME)) {
        let mut r = dc.fields(e);
        if let (Ok(Some(corner)), Ok(Some(text))) = (r.reference(), r.reference()) {
            frames.insert(text, (e, corner));
        }
    }
    for &e in entities.iter().filter(|&&e| dc.is(e, &TEXT)) {
        let made = (|| -> Result<(Value, Option<usize>), String> {
            let t = read(dc, e)?;
            let (string, used) = plain(&t.content, t.font);
            let font = match used[..] {
                [f] => fonts
                    .get(f)
                    .ok_or_else(|| format!("text {e}: font {f} is not in the font table"))?,
                [] => fonts
                    .get(t.font)
                    .ok_or_else(|| format!("text {e}: font {} is not in the font table", t.font))?,
                _ => return Err(format!("text {e}: in {} fonts", used.len())),
            };
            // Mitcad's texts keep the font's widths.
            if font.width != 0.0 && font.width != 1.0 {
                return Err(format!(
                    "text {e}: its font's width factor is {}",
                    font.width
                ));
            }
            let (frame, corner) = frames
                .get(&e)
                .copied()
                .ok_or_else(|| format!("text {e}: without a frame"))?;
            let corner = ids
                .get(&corner)
                .ok_or_else(|| format!("text {e}: its frame's corner was not read"))?;
            let lines = rectangle_through(corner, curves)
                .ok_or_else(|| format!("text {e}: no frame rectangle through its corner"))?;
            let mut v = json!({
                "text": string.clone(),
                "height": font.size,
                "position": [t.anchor[0], t.anchor[1], 0.0],
                "angle": t.angle,
                "fontName": font.family,
                "textStyle": i32::from(font.weight >= 600) | (2 * i32::from(font.italic)),
                "penAligned": true,
                "inkTop": true,
                "definition": {
                    "_type": "MultiLineTextDefinition",
                    "rectangleLines": lines.iter()
                        .map(|c| json!({"kind": "sketch_entity", "id": c}))
                        .collect::<Vec<_>>(),
                },
                "_ipt": {"record": e, "alignment": t.alignment},
            });
            // The alignment's low bits: left, centre, right; the rest the
            // frame's top (2) *(seen: 9 and 10)*.
            if let h @ 1..=3 = t.alignment & 3 {
                v["definition"]["horizontalAlignment"] = json!(h - 1);
            }
            if t.alignment >> 2 == 2 {
                v["definition"]["verticalAlignment"] = json!(0);
            }
            // The glyph contours the file keeps, in the sketch: the lines'
            // spacing from their baselines.
            let contours: Vec<Contour> = t
                .contours
                .iter()
                .map(|&c| contour(dc, c))
                .collect::<Result<_, _>>()?;
            let spacing = line_spacing(&contours, t.angle, m);
            if let Some(s) = spacing {
                v["lineSpacing"] = json!(s);
            }
            // The characters' glyphs for a computer without the font
            // (mitcad#416).
            match char_glyphs(&string, &t, &contours, m, spacing) {
                Ok(glyphs) => v["glyphs"] = glyphs.into(),
                Err(why) => v["_ipt"]["glyphsLeftOut"] = json!(why),
            }
            v["_ipt"]["glyphs"] = contours
                .iter()
                .map(|k| contour_json(k, m))
                .collect::<Vec<_>>()
                .into();
            Ok((v, Some(frame)))
        })();
        match made {
            Ok((mut v, frame)) => {
                v["id"] = json!(format!("t{}", out.texts.len()));
                out.texts.push(v);
                out.done.insert(e);
                out.done.extend(frame);
            }
            Err(why) => out.dropped.push((e, why)),
        }
    }
    out
}

/// The four construction lines of the rectangle a point is a corner of
/// (the dump's curve ids).
fn rectangle_through(corner: &str, curves: &[Value]) -> Option<Vec<String>> {
    let lines: Vec<(&str, &str, &str)> = curves
        .iter()
        .filter(|c| c["type"] == "SketchLine" && c["isConstruction"] == true)
        .filter_map(|c| {
            Some((
                c["id"].as_str()?,
                c["startSketchPoint"].as_str()?,
                c["endSketchPoint"].as_str()?,
            ))
        })
        .collect();
    let other = |l: &(&str, &str, &str), p: &str| -> Option<String> {
        match (l.1 == p, l.2 == p) {
            (true, false) => Some(l.2.to_owned()),
            (false, true) => Some(l.1.to_owned()),
            _ => None,
        }
    };
    let at_corner: Vec<_> = lines
        .iter()
        .filter_map(|l| Some((l, other(l, corner)?)))
        .collect();
    for (i, (la, a)) in at_corner.iter().enumerate() {
        for (lb, b) in at_corner.iter().skip(i + 1) {
            if a == b {
                continue;
            }
            for lc in &lines {
                let Some(d) = other(lc, a) else { continue };
                if d == corner || &d == b {
                    continue;
                }
                if let Some(ld) = lines
                    .iter()
                    .find(|l| other(l, b).as_deref() == Some(d.as_str()))
                {
                    return Some(vec![
                        la.0.to_owned(),
                        lc.0.to_owned(),
                        ld.0.to_owned(),
                        lb.0.to_owned(),
                    ]);
                }
            }
        }
    }
    None
}

/// The distance between the baselines of a text's lines (cm), from the
/// pens of its glyph contours (None for one line, or lines not evenly
/// spaced).
fn line_spacing(contours: &[Contour], angle: f64, m: &[[f64; 4]; 4]) -> Option<f64> {
    let (s, c) = angle.sin_cos();
    let mut baselines: Vec<f64> = contours
        .iter()
        .map(|k| {
            let p = to_sketch(m, k.pen);
            -p[0] * s + p[1] * c
        })
        .collect();
    baselines.sort_by(|a, b| b.total_cmp(a));
    baselines.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    let gaps: Vec<f64> = baselines.windows(2).map(|w| w[0] - w[1]).collect();
    let first = *gaps.first()?;
    gaps.iter()
        .all(|g| (g - first).abs() < 1e-6)
        .then_some(first)
}

/// A clamped B-spline as Bézier pieces (their control points): every
/// interior knot inserted until it is `degree` times there (exact). None
/// for a spline that is not clamped or not laid out so.
fn bezier_pieces(degree: usize, knots: &[f64], poles: &[[f64; 2]]) -> Option<Vec<Vec<[f64; 2]>>> {
    let p = degree;
    if p == 0 || knots.len() != poles.len() + p + 1 {
        return None;
    }
    let (lo, hi) = (knots[0], knots[knots.len() - 1]);
    if knots[..=p].iter().any(|k| *k != lo)
        || knots[knots.len() - p - 1..].iter().any(|k| *k != hi)
        || knots.windows(2).any(|w| w[1] < w[0])
        || hi <= lo
    {
        return None;
    }
    let (mut u, mut q) = (knots.to_vec(), poles.to_vec());
    let mut at = p + 1;
    while at < u.len() - p - 1 {
        let t = u[at];
        let mult = u.iter().filter(|k| **k == t).count();
        if mult >= p {
            at += mult;
            continue;
        }
        // One insertion of t (Boehm): s the last knot index at t.
        let s = at + mult - 1;
        let mut next = Vec::with_capacity(q.len() + 1);
        for i in 0..=q.len() {
            next.push(if i + p <= s {
                q[i]
            } else if i > s {
                q[i - 1]
            } else {
                let a = (t - u[i]) / (u[i + p] - u[i]);
                [
                    (1.0 - a) * q[i - 1][0] + a * q[i][0],
                    (1.0 - a) * q[i - 1][1] + a * q[i][1],
                ]
            });
        }
        u.insert(s + 1, t);
        q = next;
    }
    (q.len() % p == 1).then(|| q.windows(p + 1).step_by(p).map(<[_]>::to_vec).collect())
}

/// The highest point of a Bézier piece (its control points): its ends and
/// where a quadratic's height stops rising; a cubic's by its control
/// points' hull.
fn highest(points: &[[f64; 2]]) -> f64 {
    let ends = points[0][1].max(points[points.len() - 1][1]);
    match points {
        [a, b, c] => {
            let d = a[1] - 2.0 * b[1] + c[1];
            let t = if d.abs() > 1e-15 {
                (a[1] - b[1]) / d
            } else {
                -1.0
            };
            if (0.0..=1.0).contains(&t) {
                let s = 1.0 - t;
                ends.max(s * s * a[1] + 2.0 * s * t * b[1] + t * t * c[1])
            } else {
                ends
            }
        }
        _ => points.iter().map(|p| p[1]).fold(ends, f64::max),
    }
}

/// How far the layout of the stored glyphs may miss the file's pens and
/// frame top (cm): 0.1 µm.
const GLYPH_TOLERANCE: f64 = 1e-5;

/// A glyph contour as Bézier pieces, each its control points.
type GlyphContour = Vec<Vec<[f64; 2]>>;

/// The glyphs of a text's characters from the glyph contours the file
/// keeps (mitcad#416), for a computer without its font: one per character
/// of `string` (newlines included), `{"advance": cm, "contours": [[piece,
/// …], …]}` with each piece its Bézier control points (two for a line,
/// three for a quadratic), in the text's own axes from the character's pen
/// (cm). The contours go to the characters that are not white space, in
/// order, a character's contours sharing its pen; a character's advance
/// runs to the next pen of its line (the white space between included);
/// the last of a line ends where the frame's alignment ends the line
/// (centred or right: about the anchor; left: the line starts at the
/// anchor, and its last character ends at its outline, which nothing
/// places by). Err with why when the contours do not lie as the text's
/// layout (pen-aligned lines `spacing` apart, the frame's top at the first
/// line's highest outline point) would put them, or a piece is not one the
/// layout takes.
fn char_glyphs(
    string: &str,
    t: &Text,
    contours: &[Contour],
    m: &[[f64; 4]; 4],
    spacing: Option<f64>,
) -> Result<Vec<Value>, String> {
    let (s, c) = t.angle.sin_cos();
    // Sketch to the text's own axes about its anchor.
    let own = |p: [f64; 2]| {
        let d = [p[0] - t.anchor[0], p[1] - t.anchor[1]];
        [d[0] * c + d[1] * s, -d[0] * s + d[1] * c]
    };
    // Each contour: its pen and its pieces from the pen.
    let mut placed: Vec<([f64; 2], GlyphContour)> = Vec::new();
    for k in contours {
        let pen = own(to_sketch(m, k.pen));
        let rel = |q: [f64; 3]| {
            let o = own(to_sketch(m, q));
            [o[0] - pen[0], o[1] - pen[1]]
        };
        let mut pieces: Vec<Vec<[f64; 2]>> = Vec::new();
        for piece in &k.pieces {
            match piece {
                Piece::Line { start, end } => pieces.push(vec![rel(*start), rel(*end)]),
                Piece::Spline {
                    degree,
                    knots,
                    poles,
                } => {
                    let poles: Vec<[f64; 2]> = poles.iter().map(|q| rel(*q)).collect();
                    let parts = bezier_pieces(*degree as usize, knots, &poles)
                        .filter(|_| (1..=3).contains(degree))
                        .ok_or_else(|| format!("a glyph piece of degree {degree}"))?;
                    pieces.extend(parts);
                }
            }
        }
        // Each piece from where the one before it ends, back to the first.
        let n = pieces.len();
        for i in 0..n {
            let end = *pieces[i].last().expect("pieces have points");
            let start = &mut pieces[(i + 1) % n][0];
            if (end[0] - start[0]).hypot(end[1] - start[1]) > 1e-9 {
                return Err("a glyph contour is not closed".to_owned());
            }
            *start = end;
        }
        if n > 0 {
            placed.push((pen, pieces));
        }
    }
    // The characters' contours: runs of one pen.
    let mut groups: Vec<([f64; 2], Vec<GlyphContour>)> = Vec::new();
    for (pen, pieces) in placed {
        match groups.last_mut() {
            Some((p, list)) if (p[0] - pen[0]).hypot(p[1] - pen[1]) < 1e-9 => list.push(pieces),
            _ => groups.push((pen, vec![pieces])),
        }
    }
    let chars: Vec<char> = string.chars().collect();
    let inked: Vec<usize> = (0..chars.len())
        .filter(|&k| !chars[k].is_whitespace())
        .collect();
    if inked.len() != groups.len() {
        return Err(format!(
            "{} glyphs for {} characters",
            groups.len(),
            inked.len()
        ));
    }
    let pen_of: HashMap<usize, usize> = inked.iter().enumerate().map(|(g, &k)| (k, g)).collect();
    let mut advances = vec![0.0; chars.len()];
    let mut first_baseline: Option<f64> = None;
    let (mut start, mut line) = (0, 0);
    for end in (0..=chars.len()).filter(|&k| k == chars.len() || chars[k] == '\n') {
        let ks: Vec<usize> = (start..end).filter(|k| pen_of.contains_key(k)).collect();
        if let (Some(&k0), Some(&kn)) = (ks.first(), ks.last()) {
            let pen = |k: usize| groups[pen_of[&k]].0;
            // Its baseline, `spacing` below the line before it.
            let baseline = pen(k0)[1];
            let down = spacing.unwrap_or(0.0) * line as f64;
            let first = *first_baseline.get_or_insert(baseline + down);
            if ks
                .iter()
                .any(|&k| (pen(k)[1] - baseline).abs() > GLYPH_TOLERANCE)
                || (baseline - (first - down)).abs() > GLYPH_TOLERANCE
            {
                return Err(format!("line {}'s pens are not on its baseline", line + 1));
            }
            for w in ks.windows(2) {
                advances[w[0]] = pen(w[1])[0] - pen(w[0])[0];
                if advances[w[0]] <= 0.0 {
                    return Err(format!("line {}'s pens go back", line + 1));
                }
            }
            // The line's ends by the alignment: the anchor at its start,
            // its middle or its end.
            let (x0, xn) = (pen(k0)[0], pen(kn)[0]);
            let ink_right = groups[pen_of[&kn]]
                .1
                .iter()
                .flatten()
                .flat_map(|piece| [piece[0][0], piece[piece.len() - 1][0]])
                .fold(0.0, f64::max);
            let (from, to) = match t.alignment & 3 {
                2 => (x0, -x0),
                3 => (x0, 0.0),
                _ => (0.0, xn + ink_right),
            };
            if k0 > start {
                advances[start] = x0 - from;
            } else if (x0 - from).abs() > GLYPH_TOLERANCE {
                return Err(format!("line {} does not start at its anchor", line + 1));
            }
            advances[kn] = to - xn;
            if advances[kn] <= 0.0 || to - from > t.size[0] + GLYPH_TOLERANCE {
                return Err(format!("line {} does not fit its frame", line + 1));
            }
        }
        start = end + 1;
        line += 1;
    }
    // The frame's top (the anchor's height) meets the first line's highest
    // outline point.
    if t.alignment >> 2 != 2 {
        return Err(format!("the vertical alignment {}", t.alignment >> 2));
    }
    let first_line: Vec<usize> = inked
        .iter()
        .copied()
        .take_while(|&k| !chars[..k].contains(&'\n'))
        .collect();
    let top = first_line
        .iter()
        .flat_map(|k| groups[pen_of[k]].1.iter().flatten())
        .map(|piece| highest(piece))
        .reduce(f64::max)
        .ok_or("the first line has no outline")?;
    let baseline = groups[pen_of[&first_line[0]]].0[1];
    if (baseline + top).abs() > GLYPH_TOLERANCE {
        return Err(format!(
            "the first line's top is {:.6} mm off the frame's",
            10.0 * (baseline + top)
        ));
    }
    Ok((0..chars.len())
        .map(|k| {
            let mut g = json!({"advance": advances[k]});
            if let Some(&i) = pen_of.get(&k) {
                g["contours"] = json!(groups[i].1);
            }
            g
        })
        .collect())
}

/// A glyph contour in the sketch: its pen and its pieces (control points,
/// cm), for the record.
fn contour_json(k: &Contour, m: &[[f64; 4]; 4]) -> Value {
    let xy = |p: [f64; 3]| {
        let q = to_sketch(m, p);
        json!([q[0], q[1]])
    };
    let pieces: Vec<Value> = k
        .pieces
        .iter()
        .map(|p| match p {
            Piece::Spline {
                degree,
                knots,
                poles,
            } => json!({"degree": degree, "knots": knots,
                        "poles": poles.iter().map(|q| xy(*q)).collect::<Vec<_>>()}),
            Piece::Line { start, end } => json!({"line": [xy(*start), xy(*end)]}),
        })
        .collect();
    json!({"pen": xy(k.pen), "pieces": pieces})
}

/// The text's string without its font codes, and the font table entries
/// it uses: the one it starts with, then each `\F<n>;` that some
/// character follows.
pub fn plain(content: &str, first: u32) -> (String, Vec<u32>) {
    let mut out = String::new();
    let mut fonts: Vec<u32> = Vec::new();
    let mut current = first;
    let mut rest = content;
    while let Some(c) = rest.chars().next() {
        if let Some(code) = rest.strip_prefix("\\F")
            && let Some(end) = code.find(';')
            && let Ok(n) = code[..end].parse::<u32>()
        {
            current = n;
            rest = &code[end + 1..];
            continue;
        }
        if c != '\n' && !fonts.contains(&current) {
            fonts.push(current);
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    (out, fonts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKETCH_RECORD: usize = 9;

    fn reference(record: usize) -> [u8; 4] {
        (0x8000_0001 + record as u32).to_le_bytes()
    }

    fn text_bytes(s: &str) -> Vec<u8> {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut out = (units.len() as u32).to_le_bytes().to_vec();
        for u in units {
            out.extend(u.to_le_bytes());
        }
        out
    }

    fn f64s(out: &mut Vec<u8>, values: &[f64]) {
        for v in values {
            out.extend(v.to_le_bytes());
        }
    }

    /// A major-24 record's header and prefix.
    fn head() -> Vec<u8> {
        let mut out = vec![0u8; 22];
        out.extend((-1i32).to_le_bytes());
        out
    }

    /// A text record laid out as the files' (see the module docs).
    fn text_record(content: &str, font: u32, anchor: [f64; 2], contours: &[usize]) -> Vec<u8> {
        let mut out = head();
        f64s(&mut out, &[anchor[0], anchor[1], 0.0, 0.0]);
        for v in [0u32, 10, 1] {
            out.extend(v.to_le_bytes());
        }
        out.extend(1.0f32.to_le_bytes());
        out.extend([0u8; 17]);
        f64s(&mut out, &[0.065]);
        out.extend(text_bytes(content));
        out.extend(font.to_le_bytes());
        out.extend([3u8, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
        out.extend([2u8, 0, 0, 0x30, 0, 0, 0, 0]);
        out.extend([2u8, 0, 0, 0x30]);
        let n = contours.len() as u32;
        if n > 0 {
            for v in [n, n, 0] {
                out.extend(v.to_le_bytes());
            }
        } else {
            out.extend(0u32.to_le_bytes());
        }
        for &c in contours {
            out.extend(reference(c));
        }
        // The frame's box in the text's axes: 2 cm wide, 1 cm high.
        f64s(
            &mut out,
            &[
                anchor[0],
                anchor[1] - 1.0,
                0.0,
                anchor[0] + 2.0,
                anchor[1],
                0.0,
            ],
        );
        out.extend(0x30u32.to_le_bytes());
        out.extend([1u8, 0, 0]);
        out.extend(reference(SKETCH_RECORD));
        out
    }

    /// A glyph contour: a square of a line and a quadratic spline per
    /// side pair, its pen at `pen` (model = sketch coordinates here).
    fn contour_record(pen: [f64; 2]) -> Vec<u8> {
        let mut out = vec![0u8; 18];
        out.extend(2u32.to_le_bytes());
        let (x, y) = (pen[0] + 0.1, pen[1]);
        // A quadratic spline of two spans from (x, y) up to (x, y + 0.5).
        for v in [0x2au32, 0, 0, 2] {
            out.extend(v.to_le_bytes());
        }
        f64s(&mut out, &[1e-9]);
        for v in [7u32, 7, 8] {
            out.extend(v.to_le_bytes());
        }
        f64s(&mut out, &[0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]);
        out.extend([0u8; 8]);
        for v in [8u32, 4, 4, 8] {
            out.extend(v.to_le_bytes());
        }
        for p in [[x, y], [x - 0.1, y + 0.1], [x - 0.1, y + 0.4], [x, y + 0.5]] {
            f64s(&mut out, &[p[0], p[1], 0.0]);
        }
        f64s(&mut out, &[1e-12]);
        for v in [1u32, 1] {
            out.extend(v.to_le_bytes());
        }
        f64s(&mut out, &[0.0, 2.0]);
        // The line back down.
        out.extend(0x17u32.to_le_bytes());
        f64s(&mut out, &[x, y + 0.5, 0.0, 0.0, -0.5, 0.0]);
        out.extend(reference(SKETCH_RECORD));
        f64s(&mut out, &[pen[0], pen[1], 0.0]);
        out
    }

    fn frame_record(corner: usize, text: usize) -> Vec<u8> {
        let mut out = head();
        out.extend(reference(corner));
        out.extend(reference(text));
        out.extend([0u8; 8]);
        out
    }

    fn font_table() -> Vec<u8> {
        let mut out = vec![0u8; 6];
        out.extend([2u8, 0, 0, 0x30]);
        for v in [3u32, 3, 0x10f] {
            out.extend(v.to_le_bytes());
        }
        for (index, weight, italic, size, width, family) in [
            (1u32, 400u16, 0u8, 0.35f32, 1.0f32, "Tahoma"),
            (2, 700, 1, 0.15, 1.0, "Arial"),
            (3, 400, 0, 0.3, 0.77, "Arial"),
        ] {
            out.extend(index.to_le_bytes());
            out.extend(4u16.to_le_bytes());
            out.extend(weight.to_le_bytes());
            out.extend([italic, 0]);
            out.extend(size.to_le_bytes());
            out.extend(width.to_le_bytes());
            out.extend(text_bytes(family));
            out.extend([0u8, 0, 0, 0, 0, 0, 0x80, 0x3f, 0, 0, 0]);
        }
        out
    }

    #[test]
    fn a_text_in_its_frame_with_its_glyph_contours_is_a_dump_text() {
        let fonts = Fonts::parse(&font_table()).expect("a font table");
        assert_eq!(fonts.0.len(), 3);
        let bold = fonts.get(2).unwrap();
        assert_eq!(
            (bold.family.as_str(), bold.weight, bold.italic, bold.size),
            ("Arial", 700, true, 0.15)
        );
        // Records: 0 a text in its frame, 1 its frame, 2-4 its glyph
        // contours (two lines, 0.25 cm apart), 5 a text in two fonts, 6
        // its frame, 7 a text in a font of a width factor, 8 its frame.
        let records = [
            (
                TEXT,
                text_record("\\F2;AB\nC\\F1;", 1, [1.0, 1.0], &[2, 3, 4]),
            ),
            (FRAME, frame_record(10, 0)),
            (CONTOUR, contour_record([0.0, 0.5])),
            (CONTOUR, contour_record([0.6, 0.5])),
            (CONTOUR, contour_record([0.3, 0.25])),
            (TEXT, text_record("A\\F2;B", 1, [1.0, 1.0], &[])),
            (FRAME, frame_record(10, 5)),
            (TEXT, text_record("A", 3, [1.0, 1.0], &[])),
            (FRAME, frame_record(10, 7)),
            // 9 a text of two characters with one glyph, 10 its frame.
            (TEXT, text_record("AB", 1, [1.0, 1.0], &[2])),
            (FRAME, frame_record(10, 9)),
        ];
        let dc = Definitions::from_records(24, &records);
        let k = contour(&dc, 2).unwrap();
        assert_eq!(k.pieces.len(), 2);
        assert_eq!(k.pieces[0].start(), [0.1, 0.5, 0.0]);
        assert_eq!(k.pieces[1].end(), [0.1, 0.5, 0.0]);
        assert!(matches!(&k.pieces[0], Piece::Spline { degree: 2, poles, .. } if poles.len() == 4));
        let ids: HashMap<usize, String> = [(10, "p0".to_owned())].into();
        let line = |id: &str, a: &str, b: &str| {
            json!({"id": id, "type": "SketchLine", "startSketchPoint": a,
                   "endSketchPoint": b, "isConstruction": true})
        };
        let curves = [
            line("c0", "p1", "p2"),
            line("c1", "p0", "p1"),
            line("c2", "p2", "p3"),
            line("c3", "p3", "p0"),
            json!({"id": "c4", "type": "SketchLine", "startSketchPoint": "p0",
                   "endSketchPoint": "p2"}),
        ];
        let m = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let out = sketch_texts(&dc, &[0, 1, 5, 6, 7, 8, 9, 10], &ids, &curves, &m, &fonts);
        assert_eq!(out.texts.len(), 2, "{:?}", out.dropped);
        // Without a glyph for each character, the text has none.
        assert_eq!(out.texts[1]["glyphs"], Value::Null);
        assert_eq!(
            out.texts[1]["_ipt"]["glyphsLeftOut"],
            "1 glyphs for 2 characters"
        );
        let t = &out.texts[0];
        // The characters' glyphs (mitcad#416): from their pens in the
        // text's axes, the spline in its two quadratic pieces; each advance
        // to the next pen, a line's last to the end the centring gives
        // (the anchor the line's middle).
        let glyphs = t["glyphs"].as_array().unwrap();
        let advances: Vec<f64> = glyphs
            .iter()
            .map(|g| g["advance"].as_f64().unwrap())
            .collect();
        let expected = [0.6, 1.4, 0.0, 1.4];
        assert!(
            advances
                .iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 1e-12),
            "{advances:?}"
        );
        assert_eq!(glyphs[2].get("contours"), None);
        let pieces: Vec<Vec<[f64; 2]>> =
            serde_json::from_value(glyphs[0]["contours"][0].clone()).unwrap();
        let expected = [
            vec![[0.1, 0.0], [0.0, 0.1], [0.0, 0.25]],
            vec![[0.0, 0.25], [0.0, 0.4], [0.1, 0.5]],
            vec![[0.1, 0.5], [0.1, 0.0]],
        ];
        assert_eq!(pieces.len(), 3);
        for (piece, want) in pieces.iter().zip(&expected) {
            assert!(
                piece.len() == want.len()
                    && piece
                        .iter()
                        .zip(want)
                        .all(|(p, q)| (p[0] - q[0]).abs() < 1e-12 && (p[1] - q[1]).abs() < 1e-12),
                "{pieces:?}"
            );
        }
        assert_eq!(t["text"], "AB\nC");
        assert_eq!(
            (
                t["fontName"].clone(),
                t["height"].clone(),
                t["textStyle"].clone()
            ),
            (json!("Arial"), json!(0.15), json!(3))
        );
        assert_eq!(
            (t["penAligned"].clone(), t["inkTop"].clone()),
            (json!(true), json!(true))
        );
        assert!((t["lineSpacing"].as_f64().unwrap() - 0.25).abs() < 1e-12);
        let d = &t["definition"];
        assert_eq!(
            (
                d["horizontalAlignment"].clone(),
                d["verticalAlignment"].clone()
            ),
            (json!(1), json!(0))
        );
        // The rectangle around through its corner, construction lines only.
        let lines: Vec<&str> = d["rectangleLines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["id"].as_str().unwrap())
            .collect();
        assert_eq!(lines, ["c1", "c0", "c2", "c3"]);
        assert_eq!(t["_ipt"]["glyphs"].as_array().unwrap().len(), 3);
        assert_eq!(out.done, [0, 1, 9, 10].into());
        let dropped: Vec<&str> = out.dropped.iter().map(|(_, why)| why.as_str()).collect();
        assert_eq!(
            dropped,
            [
                "text 5: in 2 fonts",
                "text 7: its font's width factor is 0.77"
            ]
        );
    }

    #[test]
    fn a_clamped_spline_is_split_into_bezier_pieces_exactly() {
        // A cubic of two spans and a knot of multiplicity two: three
        // pieces, each ending where the next starts on the spline.
        let poles = [
            [0.0, 0.0],
            [1.0, 2.0],
            [2.0, 2.0],
            [3.0, 0.0],
            [4.0, 1.0],
            [5.0, 0.0],
            [6.0, 2.0],
        ];
        let knots = [0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0];
        let pieces = bezier_pieces(3, &knots, &poles).expect("pieces");
        assert_eq!(pieces.len(), 3);
        assert!(pieces.iter().all(|p| p.len() == 4));
        assert_eq!(pieces[0][0], poles[0]);
        assert_eq!(pieces[2][3], poles[6]);
        for w in pieces.windows(2) {
            assert_eq!(w[0][3], w[1][0]);
        }
        // At the simple knot 1 the spline is C1: the first piece's last two
        // control points and the second's second are on one line.
        let (a, b, c) = (pieces[0][2], pieces[0][3], pieces[1][1]);
        assert!(((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() < 1e-12);
        // Not clamped: none.
        let open: Vec<f64> = (0..11).map(f64::from).collect();
        assert_eq!(bezier_pieces(3, &open, &poles), None);
    }

    #[test]
    fn font_codes_are_left_out_of_the_string() {
        assert_eq!(
            plain("\\F16;16x5,08\n\\F16;XL\\F6;", 6),
            ("16x5,08\nXL".to_owned(), vec![16])
        );
        assert_eq!(plain("a\\F2;b", 1), ("ab".to_owned(), vec![1, 2]));
    }
}
