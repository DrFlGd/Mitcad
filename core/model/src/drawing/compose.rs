// SPDX-License-Identifier: MIT
//! The layout of a sheet: the views in their places after the projection
//! method at a scale that fits (or the one given), their overall
//! dimensions, the diameters of round holes with leaders, the hole table
//! and the title block (ISO 5457 frame, ISO 7200 title block, ISO 129
//! dimensions, ISO 128 lines).

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_1_SQRT_2, PI, TAU};

use super::sheet::{Anchor, Group, Item, Pen, Sheet, text_width};
use super::{
    CutPlane, DrawingOptions, Orientation, P2, Projection, RoundHole, ScaleChoice, Units,
    ViewCurve, ViewEdgeKind, ViewName, ViewProjection, ViewSpec, cross, dot, scale_text, unit,
};
use crate::datum::Vec3;

/// A view to lay out.
#[derive(Debug, Clone)]
pub struct ViewInput {
    /// None: the section view.
    pub name: Option<ViewName>,
    pub spec: ViewSpec,
    pub projection: ViewProjection,
}

/// A round hole with its thread's designation (the `threads` query).
#[derive(Debug, Clone)]
pub struct HoleInput {
    pub hole: RoundHole,
    pub thread: Option<String>,
}

/// A dimension of a sketch, in model coordinates, drawn in a view that
/// looks along its sketch's normal.
#[derive(Debug, Clone, PartialEq)]
pub enum SketchDimension {
    /// Between two points, measured along `along` (a sketch axis) or
    /// aligned with them.
    Linear {
        a: Vec3,
        b: Vec3,
        along: Option<Vec3>,
        text: Option<Vec3>,
        value: f64,
        normal: Vec3,
    },
    /// A circle's or arc's radius or diameter.
    Radial {
        center: Vec3,
        radius: f64,
        diameter: bool,
        text: Option<Vec3>,
        value: f64,
        normal: Vec3,
    },
}

#[derive(Debug, Clone, Default)]
pub struct SheetInput {
    /// The design's name.
    pub title: String,
    pub bodies: Vec<String>,
    pub date: String,
    pub views: Vec<ViewInput>,
    pub holes: Vec<HoleInput>,
    pub sketch_dimensions: Vec<SketchDimension>,
}

/// A laid out sheet and what it holds.
#[derive(Debug, Clone)]
pub struct Composed {
    pub sheet: Sheet,
    pub scale: f64,
    pub views: usize,
    pub dimensions: usize,
    pub holes: usize,
    pub hole_rows: usize,
    pub polygonal: bool,
    pub warnings: Vec<String>,
}

// Sheet lengths, mm.
const MARGIN_LEFT: f64 = 20.0; // ISO 5457: the filing margin
const MARGIN: f64 = 10.0;
const TITLE_WIDTH: f64 = 180.0;
const TITLE_HEIGHT: f64 = 32.0;
const DIMENSION_TEXT: f64 = 3.5;
const CALLOUT_TEXT: f64 = 3.0;
const SMALL_TEXT: f64 = 2.5;
const LABEL_TEXT: f64 = 2.0;
const ARROW_LENGTH: f64 = 3.0;
// ISO 129-1: a closed filled arrowhead of 30 degrees.
const ARROW_HALF_WIDTH: f64 = 0.8;
const EXTENSION_GAP: f64 = 1.0;
const EXTENSION_BEYOND: f64 = 2.0;
const DIMENSION_OFFSET: f64 = 10.0;
// The room around a view for its dimensions (left, below) and leaders.
const ROOM_LEFT: f64 = 16.0;
const ROOM_BELOW: f64 = 16.0;
const ROOM_RIGHT: f64 = 14.0;
const ROOM_ABOVE: f64 = 14.0;
const HATCH_SPACING: f64 = 2.5;
const ROW_HEIGHT: f64 = 5.0;
/// Under the hole table: where its X and Y are measured from.
const TABLE_NOTE: [&str; 2] = ["X, Y FROM THE LOWER LEFT", "CORNER OF THE VIEW"];

/// ISO 5455's scales, largest first.
const SCALES: [f64; 15] = [
    50.0, 20.0, 10.0, 5.0, 2.0, 1.0, 0.5, 0.2, 0.1, 0.05, 0.02, 0.01, 0.005, 0.002, 0.001,
];

/// An axis-aligned box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: P2,
    pub max: P2,
}

impl Bounds {
    fn empty() -> Self {
        Self {
            min: [f64::INFINITY; 2],
            max: [f64::NEG_INFINITY; 2],
        }
    }
    fn add(&mut self, p: P2) {
        for (i, v) in p.into_iter().enumerate() {
            self.min[i] = self.min[i].min(v);
            self.max[i] = self.max[i].max(v);
        }
    }
    fn is_empty(&self) -> bool {
        !(self.min[0] <= self.max[0] && self.min[1] <= self.max[1])
    }
    pub fn width(&self) -> f64 {
        self.max[0] - self.min[0]
    }
    pub fn height(&self) -> f64 {
        self.max[1] - self.min[1]
    }
    fn center(&self) -> P2 {
        [
            (self.min[0] + self.max[0]) / 2.0,
            (self.min[1] + self.max[1]) / 2.0,
        ]
    }
}

/// Whether the angle lies in the counter-clockwise range from `start` to
/// `end`.
fn in_sweep(angle: f64, start: f64, end: f64) -> bool {
    let t = (angle - start).rem_euclid(TAU);
    t <= end - start + 1e-12
}

/// The box of a view's edges and cut faces, in view coordinates.
pub fn view_bounds(projection: &ViewProjection) -> Bounds {
    let mut b = Bounds::empty();
    for edge in &projection.edges {
        match &edge.curve {
            ViewCurve::Line(p, q) => {
                b.add(*p);
                b.add(*q);
            }
            ViewCurve::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let at = |a: f64| [center[0] + radius * a.cos(), center[1] + radius * a.sin()];
                b.add(at(*start));
                b.add(at(*end));
                for k in 0..4 {
                    let a = f64::from(k) * PI / 2.0;
                    if in_sweep(a, *start, *end) {
                        b.add(at(a));
                    }
                }
            }
            ViewCurve::Polyline(points) => {
                for p in points {
                    b.add(*p);
                }
            }
        }
    }
    for face in &projection.cuts {
        for lp in face {
            for p in lp {
                b.add(*p);
            }
        }
    }
    b
}

/// Where a view goes: the sheet point of view coordinates.
#[derive(Debug, Clone, Copy)]
struct Placement {
    scale: f64,
    /// The view's box centre in view coordinates and on the sheet.
    model_center: P2,
    sheet_center: P2,
}

impl Placement {
    fn at(&self, p: P2) -> P2 {
        [
            self.sheet_center[0] + self.scale * (p[0] - self.model_center[0]),
            self.sheet_center[1] + self.scale * (p[1] - self.model_center[1]),
        ]
    }
}

fn add2(a: P2, b: P2) -> P2 {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub2(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}
fn mul2(a: P2, s: f64) -> P2 {
    [a[0] * s, a[1] * s]
}
fn len2(a: P2) -> f64 {
    a[0].hypot(a[1])
}
fn unit2(a: P2) -> P2 {
    let n = len2(a);
    if n > 1e-12 {
        mul2(a, 1.0 / n)
    } else {
        [1.0, 0.0]
    }
}
fn perp(a: P2) -> P2 {
    [-a[1], a[0]]
}

/// A filled arrowhead whose tip is at `tip`, pointing along `direction`.
fn arrow(group: &mut Group, tip: P2, direction: P2) {
    let d = unit2(direction);
    let base = sub2(tip, mul2(d, ARROW_LENGTH));
    let side = mul2(perp(d), ARROW_HALF_WIDTH);
    group.items.push(Item::Fill {
        points: vec![tip, add2(base, side), sub2(base, side)],
    });
}

/// A text's baseline direction that reads from below or from the right
/// (ISO 129-1), and the side its text goes to.
fn upright(direction: P2) -> f64 {
    let mut angle = direction[1].atan2(direction[0]);
    if angle > PI / 2.0 + 1e-9 {
        angle -= PI;
    } else if angle <= -PI / 2.0 + 1e-9 {
        angle += PI;
    }
    angle
}

/// A linear dimension between `a` and `b` (sheet points) measured along
/// `direction`, its dimension line `offset` away on the side of
/// `perp(direction)` (negative: the other side).
fn linear_dimension(a: P2, b: P2, direction: P2, offset: f64, text: &str) -> Group {
    let mut g = Group::new("dimension");
    let d = unit2(direction);
    let n = perp(d);
    // The feet on the dimension line: both points moved across to it.
    let base = add2(a, mul2(n, offset));
    let along = dot2(sub2(b, a), d);
    let fa = base;
    let fb = add2(base, mul2(d, along));
    let side = if offset >= 0.0 { 1.0 } else { -1.0 };
    for (from, foot) in [(a, fa), (b, fb)] {
        let reach = dot2(sub2(foot, from), n) * side;
        if reach > EXTENSION_GAP {
            g.line(
                Pen::Thin,
                add2(from, mul2(n, side * EXTENSION_GAP)),
                add2(foot, mul2(n, side * EXTENSION_BEYOND)),
            );
        }
    }
    g.line(Pen::Thin, fa, fb);
    let length = along.abs();
    let towards = unit2(sub2(fb, fa));
    if length >= 2.0 * ARROW_LENGTH + 1.0 {
        arrow(&mut g, fa, mul2(towards, -1.0));
        arrow(&mut g, fb, towards);
    } else {
        // Too short: the arrows outside, pointing in.
        g.line(Pen::Thin, fa, sub2(fa, mul2(towards, ARROW_LENGTH + 2.0)));
        g.line(Pen::Thin, fb, add2(fb, mul2(towards, ARROW_LENGTH + 2.0)));
        arrow(&mut g, fa, towards);
        arrow(&mut g, fb, mul2(towards, -1.0));
    }
    let angle = upright(towards);
    let up = [-angle.sin(), angle.cos()];
    let middle = mul2(add2(fa, fb), 0.5);
    g.text(
        add2(middle, mul2(up, 1.0)),
        DIMENSION_TEXT,
        angle,
        Anchor::Middle,
        text,
    );
    g
}

fn dot2(a: P2, b: P2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// The height of a callout's text lines.
fn callout_height(lines: usize) -> f64 {
    lines as f64 * CALLOUT_TEXT * 1.6
}

/// A leader from a circle to text lines on a shoulder at `knee`, the arrow
/// on the circle; the shoulder and the text run to the right when `side`
/// is positive, else to the left.
fn leader(center: P2, radius: f64, knee: P2, side: f64, lines: &[String]) -> Group {
    let mut g = Group::new("dimension");
    let d = unit2(sub2(knee, center));
    let on = add2(center, mul2(d, radius));
    let width = lines
        .iter()
        .map(|l| text_width(l, CALLOUT_TEXT))
        .fold(0.0, f64::max);
    g.line(Pen::Thin, on, knee);
    g.line(Pen::Thin, knee, add2(knee, [side * (width + 2.0), 0.0]));
    arrow(&mut g, on, mul2(d, -1.0));
    let (x, anchor) = if side > 0.0 {
        (knee[0] + 1.0, Anchor::Start)
    } else {
        (knee[0] - 1.0, Anchor::End)
    };
    for (i, line) in lines.iter().rev().enumerate() {
        let y = knee[1] + 1.0 + i as f64 * CALLOUT_TEXT * 1.6;
        g.text([x, y], CALLOUT_TEXT, 0.0, anchor, line.clone());
    }
    g
}

/// Hatching at 45 degrees of a face's loops (sheet points), even-odd.
fn hatch(group: &mut Group, loops: &[Vec<P2>]) {
    // Turned by -45 degrees the hatch lines are horizontal.
    let (c, s) = (FRAC_1_SQRT_2, -FRAC_1_SQRT_2);
    let turn = |p: P2| [p[0] * c - p[1] * s, p[0] * s + p[1] * c];
    let back = |p: P2| [p[0] * c + p[1] * s, -p[0] * s + p[1] * c];
    let turned: Vec<Vec<P2>> = loops
        .iter()
        .map(|l| l.iter().map(|p| turn(*p)).collect())
        .collect();
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for p in turned.iter().flatten() {
        low = low.min(p[1]);
        high = high.max(p[1]);
    }
    if high.partial_cmp(&low) != Some(std::cmp::Ordering::Greater) {
        return;
    }
    let mut y = (low / HATCH_SPACING).floor() * HATCH_SPACING + HATCH_SPACING / 2.0;
    while y < high {
        let mut xs = Vec::new();
        for lp in &turned {
            for i in 0..lp.len() {
                let p = lp[i];
                let q = lp[(i + 1) % lp.len()];
                if (p[1] <= y) != (q[1] <= y) {
                    xs.push(p[0] + (y - p[1]) / (q[1] - p[1]) * (q[0] - p[0]));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.as_chunks::<2>().0 {
            group.line(Pen::Hatch, back([pair[0], y]), back([pair[1], y]));
        }
        y += HATCH_SPACING;
    }
}

/// A view's place in the grid of views: (row from the top, column).
fn cell(name: Option<ViewName>, projection: super::Projection) -> (usize, usize) {
    use super::Projection::{FirstAngle, ThirdAngle};
    match (projection, name) {
        (ThirdAngle, Some(ViewName::Top)) => (0, 0),
        (ThirdAngle, Some(ViewName::Iso)) => (0, 1),
        (ThirdAngle, Some(ViewName::Front)) => (1, 0),
        (ThirdAngle, Some(ViewName::Right)) => (1, 1),
        (ThirdAngle, None) => (1, 2),
        (FirstAngle, Some(ViewName::Right)) => (0, 0),
        (FirstAngle, Some(ViewName::Front)) => (0, 1),
        (FirstAngle, None) => (0, 2),
        (FirstAngle, Some(ViewName::Iso)) => (1, 0),
        (FirstAngle, Some(ViewName::Top)) => (1, 1),
    }
}

/// The text lines of a hole's callout (without the count) and its row of
/// the hole table.
struct HoleText {
    size: String,
    depth: String,
    extra: Option<String>,
    thread: Option<String>,
}

fn hole_text(h: &HoleInput, units: Units) -> HoleText {
    let hole = &h.hole;
    let extra = match (hole.counterbore, hole.countersink) {
        (Some((d, depth)), _) => Some(format!(
            "CBORE Ø{} DEPTH {}",
            units.length(d),
            units.length(depth)
        )),
        (None, Some((d, angle))) => Some(format!(
            "CSK Ø{} x {}°",
            units.length(d),
            super::trim_number(angle.to_degrees(), 1)
        )),
        (None, None) => None,
    };
    HoleText {
        size: format!("Ø{}", units.length(hole.diameter)),
        depth: if hole.through {
            "THRU".to_owned()
        } else {
            units.length(hole.depth)
        },
        extra,
        thread: h.thread.clone(),
    }
}

/// A callout of a hole size in a view.
struct Callout {
    view: usize,
    lines: Vec<String>,
    /// View coordinates of the hole it points at, and its radius.
    at: P2,
    radius: f64,
}

/// A hole as one view shows it.
struct ViewHole<'a> {
    input: &'a HoleInput,
    /// View coordinates of the axis.
    at: P2,
    tag: String,
}

pub fn compose(options: &DrawingOptions, units: Units, input: &SheetInput) -> Composed {
    let (long, short) = options.sheet.landscape();
    let (width, height) = match options.orientation {
        Orientation::Landscape => (long, short),
        Orientation::Portrait => (short, long),
    };
    let mut warnings = Vec::new();
    let frame_min = [MARGIN_LEFT, MARGIN];
    let frame_max = [width - MARGIN, height - MARGIN];
    let mut groups = Vec::new();

    // The views with something to show, and their boxes.
    let views: Vec<(&ViewInput, Bounds)> = input
        .views
        .iter()
        .filter_map(|v| {
            let b = view_bounds(&v.projection);
            if b.is_empty() {
                warnings.push(format!(
                    "the {} view shows nothing",
                    v.name.map_or("section", ViewName::as_str)
                ));
                None
            } else {
                Some((v, b))
            }
        })
        .collect();

    // The holes each orthographic view shows end on, tagged per view.
    let mut tagged: Vec<(usize, Vec<ViewHole<'_>>)> = Vec::new();
    let mut letter = b'A';
    for (index, (view, _)) in views.iter().enumerate() {
        if view.name.is_none() || view.name == Some(ViewName::Iso) {
            continue;
        }
        let eye = view.spec.toward_eye;
        let mut shown: Vec<ViewHole<'_>> = input
            .holes
            .iter()
            .filter(|h| {
                let along = dot(h.hole.direction, eye);
                along.abs() > 1.0 - 1e-6 && (h.hole.through || along < 0.0)
            })
            .map(|h| ViewHole {
                input: h,
                at: view.spec.project(h.hole.opening),
                tag: String::new(),
            })
            .collect();
        if shown.is_empty() {
            continue;
        }
        shown.sort_by(|a, b| {
            (a.at[0], a.at[1])
                .partial_cmp(&(b.at[0], b.at[1]))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (k, h) in shown.iter_mut().enumerate() {
            h.tag = format!("{}{}", letter as char, k + 1);
        }
        letter = letter.saturating_add(1);
        tagged.push((index, shown));
    }
    let hole_count: usize = tagged.iter().map(|(_, h)| h.len()).sum();

    // The diameters of round holes, once per size ("4x Ø6.6 THRU"), each
    // on a leader from the first hole of its size in the first view that
    // shows it end on.
    let mut callouts: Vec<Callout> = Vec::new();
    if options.dimensions {
        let mut labelled: Vec<String> = Vec::new();
        for (index, holes) in &tagged {
            let mut sizes: Vec<(String, Vec<&ViewHole<'_>>)> = Vec::new();
            for h in holes {
                let text = hole_text(h.input, units);
                // A tapped hole by its thread, others by their diameter.
                let first = text.thread.unwrap_or(text.size);
                let mut lines = vec![format!("{first} {}", text.depth)];
                lines.extend(text.extra);
                let key = lines.join("\n");
                match sizes.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, members)) => members.push(h),
                    None => sizes.push((key, vec![h])),
                }
            }
            for (key, members) in sizes {
                if labelled.contains(&key) {
                    continue;
                }
                let mut lines: Vec<String> = key.split('\n').map(str::to_owned).collect();
                if members.len() > 1 {
                    lines[0] = format!("{}x {}", members.len(), lines[0]);
                }
                callouts.push(Callout {
                    view: *index,
                    lines,
                    at: members[0].at,
                    radius: members[0].input.hole.diameter / 2.0,
                });
                labelled.push(key);
            }
        }
    }
    // The room above a view for its callouts.
    let above = |view: usize| {
        let needed: f64 = callouts
            .iter()
            .filter(|c| c.view == view)
            .map(|c| callout_height(c.lines.len()) + 2.0)
            .sum();
        if needed > 0.0 {
            ROOM_ABOVE.max(needed + 6.0)
        } else {
            ROOM_ABOVE
        }
    };

    // The hole table's columns and rows.
    let table_rows: Vec<[String; 6]> = if options.hole_table {
        tagged
            .iter()
            .flat_map(|(index, holes)| {
                let bounds = views[*index].1;
                holes.iter().map(move |h| {
                    let text = hole_text(h.input, units);
                    let mut size = text.size.clone();
                    if let Some(extra) = &text.extra {
                        size = format!("{size} {extra}");
                    }
                    [
                        h.tag.clone(),
                        units.length(h.at[0] - bounds.min[0]),
                        units.length(h.at[1] - bounds.min[1]),
                        size,
                        text.depth.clone(),
                        text.thread.clone().unwrap_or_else(|| "-".to_owned()),
                    ]
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let header = ["TAG", "X", "Y", "SIZE", "DEPTH", "THREAD"].map(str::to_owned);
    let mut columns = [0.0f64; 6];
    for row in std::iter::once(&header).chain(table_rows.iter()) {
        for (i, cell) in row.iter().enumerate() {
            columns[i] = columns[i].max(text_width(cell, SMALL_TEXT) + 3.0);
        }
    }
    // As wide as its note at least.
    let note_width = text_width(TABLE_NOTE[0], SMALL_TEXT) + 1.0;
    let sum: f64 = columns.iter().sum();
    if sum < note_width {
        columns[3] += note_width - sum;
    }
    let table_width: f64 = if table_rows.is_empty() {
        0.0
    } else {
        columns.iter().sum()
    };
    // Rows that fit beside the views, above the title block.
    let room_rows =
        ((frame_max[1] - frame_min[1] - TITLE_HEIGHT - 12.0) / ROW_HEIGHT).floor() as usize;
    let shown_rows = table_rows.len().min(room_rows.saturating_sub(3));
    if shown_rows < table_rows.len() {
        warnings.push(format!(
            "the hole table shows {shown_rows} of {} holes: the sheet has no room for more",
            table_rows.len()
        ));
    }

    // The area of the views.
    let area_min = [frame_min[0] + 4.0, frame_min[1] + TITLE_HEIGHT + 4.0];
    let area_max = [
        frame_max[0]
            - 4.0
            - if table_width > 0.0 {
                table_width + 6.0
            } else {
                0.0
            },
        frame_max[1] - 4.0,
    ];

    // The grid: rows and columns of the views present.
    let mut cells: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (i, (view, _)) in views.iter().enumerate() {
        cells.insert(cell(view.name, options.projection), i);
    }
    let mut rows: Vec<usize> = cells.keys().map(|(r, _)| *r).collect();
    rows.sort_unstable();
    rows.dedup();
    let mut cols: Vec<usize> = cells.keys().map(|(_, c)| *c).collect();
    cols.sort_unstable();
    cols.dedup();
    let col_width: Vec<f64> = cols
        .iter()
        .map(|c| {
            cells
                .iter()
                .filter(|((_, cc), _)| cc == c)
                .map(|(_, &i)| views[i].1.width())
                .fold(0.0, f64::max)
        })
        .collect();
    let row_height: Vec<f64> = rows
        .iter()
        .map(|r| {
            cells
                .iter()
                .filter(|((rr, _), _)| rr == r)
                .map(|(_, &i)| views[i].1.height())
                .fold(0.0, f64::max)
        })
        .collect();
    let row_above: Vec<f64> = rows
        .iter()
        .map(|r| {
            cells
                .iter()
                .filter(|((rr, _), _)| rr == r)
                .map(|(_, &i)| above(i))
                .fold(ROOM_ABOVE, f64::max)
        })
        .collect();
    let room_x = (ROOM_LEFT + ROOM_RIGHT) * cols.len() as f64;
    let room_y = ROOM_BELOW * rows.len() as f64 + row_above.iter().sum::<f64>();
    let sum_w: f64 = col_width.iter().sum();
    let sum_h: f64 = row_height.iter().sum();
    let avail_w = area_max[0] - area_min[0];
    let avail_h = area_max[1] - area_min[1];
    let fit = {
        let sx = if sum_w > 1e-9 {
            (avail_w - room_x) / sum_w
        } else {
            f64::INFINITY
        };
        let sy = if sum_h > 1e-9 {
            (avail_h - room_y) / sum_h
        } else {
            f64::INFINITY
        };
        sx.min(sy)
    };
    let scale = match options.scale {
        ScaleChoice::Fixed(s) => {
            if s > fit * (1.0 + 1e-9) && !views.is_empty() {
                warnings.push(format!(
                    "at {} the views do not fit on the sheet",
                    scale_text(s)
                ));
            }
            s
        }
        ScaleChoice::Fit => match SCALES.iter().find(|&&s| s <= fit * (1.0 + 1e-9)) {
            Some(&s) => s,
            None => {
                let s = SCALES[SCALES.len() - 1];
                if !views.is_empty() {
                    warnings.push(format!(
                        "at {} the views do not fit on the sheet",
                        scale_text(s)
                    ));
                }
                s
            }
        },
    };

    // Each view's place: centred in its cell, the grid centred in the area.
    let col_sheet: Vec<f64> = col_width
        .iter()
        .map(|w| scale * w + ROOM_LEFT + ROOM_RIGHT)
        .collect();
    let row_sheet: Vec<f64> = row_height
        .iter()
        .zip(&row_above)
        .map(|(h, a)| scale * h + ROOM_BELOW + a)
        .collect();
    let total_w: f64 = col_sheet.iter().sum();
    let total_h: f64 = row_sheet.iter().sum();
    let left = area_min[0] + ((avail_w - total_w) / 2.0).max(0.0);
    let top = area_max[1] - ((avail_h - total_h) / 2.0).max(0.0);
    let mut placements: Vec<Option<Placement>> = vec![None; views.len()];
    for (&(r, c), &i) in &cells {
        let ci = cols.iter().position(|x| *x == c).unwrap_or(0);
        let ri = rows.iter().position(|x| *x == r).unwrap_or(0);
        let x =
            left + col_sheet[..ci].iter().sum::<f64>() + ROOM_LEFT + scale * col_width[ci] / 2.0;
        let y = top
            - row_sheet[..ri].iter().sum::<f64>()
            - row_above[ri]
            - scale * row_height[ri] / 2.0;
        placements[i] = Some(Placement {
            scale,
            model_center: views[i].1.center(),
            sheet_center: [x, y],
        });
    }

    // The views.
    let mut polygonal = false;
    let mut dimensions = 0;
    let section_letter = "A";
    for (i, (view, bounds)) in views.iter().enumerate() {
        let Some(place) = placements[i] else { continue };
        polygonal |= view.projection.polygonal;
        let mut g = Group::new("view");
        g.id = Some(format!(
            "view-{}",
            view.name.map_or("section", ViewName::as_str)
        ));
        for face in &view.projection.cuts {
            let loops: Vec<Vec<P2>> = face
                .iter()
                .map(|l| l.iter().map(|p| place.at(*p)).collect())
                .collect();
            hatch(&mut g, &loops);
        }
        for edge in &view.projection.edges {
            let pen = match (edge.visible, edge.kind) {
                (false, _) => Pen::Hidden,
                (true, ViewEdgeKind::Smooth) => Pen::Thin,
                (true, _) => Pen::Thick,
            };
            g.items.push(match &edge.curve {
                ViewCurve::Line(p, q) => Item::Line {
                    pen,
                    a: place.at(*p),
                    b: place.at(*q),
                },
                ViewCurve::Arc {
                    center,
                    radius,
                    start,
                    end,
                } => Item::Arc {
                    pen,
                    center: place.at(*center),
                    radius: radius * scale,
                    start: *start,
                    end: *end,
                },
                ViewCurve::Polyline(points) => Item::Polyline {
                    pen,
                    points: points.iter().map(|p| place.at(*p)).collect(),
                    closed: false,
                },
            });
        }
        groups.push(g);
        let lo = place.at(bounds.min);
        let hi = place.at(bounds.max);
        if view.name.is_none() {
            let mut label = Group::new("view-label");
            label.text(
                [(lo[0] + hi[0]) / 2.0, lo[1] - 9.0],
                5.0,
                0.0,
                Anchor::Middle,
                format!("SECTION {section_letter}-{section_letter}"),
            );
            groups.push(label);
        }
        // Overall dimensions of the orthographic views.
        let orthographic = view.name.is_some_and(|n| n != ViewName::Iso);
        if options.dimensions && orthographic {
            groups.push(linear_dimension(
                lo,
                [hi[0], lo[1]],
                [1.0, 0.0],
                -DIMENSION_OFFSET,
                &units.length(bounds.width()),
            ));
            groups.push(linear_dimension(
                lo,
                [lo[0], hi[1]],
                [0.0, 1.0],
                DIMENSION_OFFSET,
                &units.length(bounds.height()),
            ));
            dimensions += 2;
        }
    }

    // The section's cutting plane in a view that sees it edge on.
    if let Some(section) = views
        .iter()
        .find(|(v, _)| v.name.is_none())
        .and_then(|(v, _)| v.spec.section)
        && let Some(g) = section_line(&views, &placements, &section, section_letter)
    {
        groups.push(g);
    }

    // The callouts above their views, stacked, each shoulder pointing away
    // from the view's middle.
    let mut stacked: BTreeMap<usize, f64> = BTreeMap::new();
    for callout in &callouts {
        let Some(place) = placements[callout.view] else {
            continue;
        };
        let lo = place.at(views[callout.view].1.min);
        let hi = place.at(views[callout.view].1.max);
        let rise = stacked.entry(callout.view).or_insert(4.0);
        let center = place.at(callout.at);
        let knee_y = hi[1] + *rise;
        let width = callout
            .lines
            .iter()
            .map(|l| text_width(l, CALLOUT_TEXT))
            .fold(0.0, f64::max)
            + 2.0;
        let knee_x = |side: f64| {
            (center[0] + side * (0.25 * (knee_y - center[1])).max(3.0))
                .clamp(lo[0] - 4.0, hi[0] + 4.0)
        };
        // Away from the view's middle, unless the text would leave the frame.
        let mut side = if center[0] >= place.sheet_center[0] {
            1.0
        } else {
            -1.0
        };
        if (side < 0.0 && knee_x(side) - width < frame_min[0] + 2.0)
            || (side > 0.0 && knee_x(side) + width > frame_max[0] - 2.0)
        {
            side = -side;
        }
        let knee = [knee_x(side), knee_y];
        *rise += callout_height(callout.lines.len()) + 2.0;
        groups.push(leader(
            center,
            callout.radius * scale,
            knee,
            side,
            &callout.lines,
        ));
        dimensions += 1;
    }

    // The holes' tags, below right of each, and the origin of the table's
    // X and Y: the view's lower left corner.
    for (index, holes) in &tagged {
        let Some(place) = placements[*index] else {
            continue;
        };
        if !options.hole_table {
            continue;
        }
        for h in holes {
            let at = place.at(h.at);
            let r = h.input.hole.diameter / 2.0 * scale;
            let mut tag = Group::new("hole-tag");
            tag.id = Some(h.tag.clone());
            tag.text(
                [
                    at[0] + r * FRAC_1_SQRT_2 + 0.5,
                    at[1] - r * FRAC_1_SQRT_2 - 0.5 - SMALL_TEXT,
                ],
                SMALL_TEXT,
                0.0,
                Anchor::Start,
                h.tag.clone(),
            );
            groups.push(tag);
        }
        // The origin of the table's X and Y: the view's lower left corner.
        if options.hole_table {
            let corner = place.at(views[*index].1.min);
            let mut datum = Group::new("hole-datum");
            datum.items.push(Item::Arc {
                pen: Pen::Thin,
                center: corner,
                radius: 1.2,
                start: 0.0,
                end: TAU,
            });
            datum.line(
                Pen::Thin,
                add2(corner, [-2.0, 0.0]),
                add2(corner, [2.0, 0.0]),
            );
            datum.line(
                Pen::Thin,
                add2(corner, [0.0, -2.0]),
                add2(corner, [0.0, 2.0]),
            );
            groups.push(datum);
        }
    }

    // Sketch dimensions in the views that look along their sketches.
    if options.sketch_dimensions {
        for dim in &input.sketch_dimensions {
            let normal = match dim {
                SketchDimension::Linear { normal, .. } | SketchDimension::Radial { normal, .. } => {
                    *normal
                }
            };
            let target = views.iter().enumerate().find(|(_, (v, _))| {
                v.name.is_some_and(|n| n != ViewName::Iso)
                    && dot(v.spec.toward_eye, normal).abs() > 1.0 - 1e-9
            });
            let Some((i, (view, _))) = target else {
                continue;
            };
            let Some(place) = placements[i] else { continue };
            if let Some(g) = sketch_dimension(dim, &view.spec, &place, units) {
                groups.push(g);
                dimensions += 1;
            }
        }
    }

    // The hole table, top right.
    let mut hole_rows = 0;
    if shown_rows > 0 {
        let x0 = frame_max[0] - 4.0 - table_width;
        let mut y = frame_max[1] - 4.0;
        let mut table = Group::new("hole-table");
        table.text(
            [x0, y - 3.0],
            SMALL_TEXT,
            0.0,
            Anchor::Start,
            format!("HOLES ({})", units.symbol),
        );
        y -= 5.0;
        let bottom = y - ROW_HEIGHT * (shown_rows + 1) as f64;
        table.rect(Pen::Thin, [x0, bottom], [x0 + table_width, y]);
        let mut x = x0;
        for (i, w) in columns.iter().enumerate() {
            if i > 0 {
                table.line(Pen::Thin, [x, bottom], [x, y]);
            }
            table.text(
                [x + 1.5, y - ROW_HEIGHT + 1.5],
                SMALL_TEXT,
                0.0,
                Anchor::Start,
                header[i].clone(),
            );
            x += w;
        }
        groups.push(table);
        for row in table_rows.iter().take(shown_rows) {
            y -= ROW_HEIGHT;
            let mut g = Group::new("hole-row");
            g.id = Some(row[0].clone());
            g.line(Pen::Thin, [x0, y], [x0 + table_width, y]);
            let mut x = x0;
            for (i, w) in columns.iter().enumerate() {
                g.text(
                    [x + 1.5, y - ROW_HEIGHT + 1.5],
                    SMALL_TEXT,
                    0.0,
                    Anchor::Start,
                    row[i].clone(),
                );
                x += w;
            }
            groups.push(g);
            hole_rows += 1;
        }
        let mut note = Group::new("hole-table");
        for (i, line) in TABLE_NOTE.iter().enumerate() {
            note.text(
                [x0, y - ROW_HEIGHT - 3.5 - i as f64 * SMALL_TEXT * 1.5],
                SMALL_TEXT,
                0.0,
                Anchor::Start,
                *line,
            );
        }
        groups.push(note);
    }

    // The frame and the title block.
    let mut frame = Group::new("frame");
    frame.rect(Pen::Frame, frame_min, frame_max);
    groups.push(frame);
    groups.push(title_block(
        options,
        units,
        input,
        scale,
        [
            frame_max[0] - TITLE_WIDTH.min(frame_max[0] - frame_min[0]),
            frame_min[1],
        ],
        frame_max[0],
    ));

    Composed {
        sheet: Sheet {
            width,
            height,
            title: input.title.clone(),
            groups,
        },
        scale,
        views: views.len(),
        dimensions,
        holes: hole_count,
        hole_rows,
        polygonal,
        warnings,
    }
}

/// The cutting plane of the section as a chain line across the first
/// orthographic view that sees it edge on, thick at its ends, with arrows
/// in the direction of sight and the section's letter.
fn section_line(
    views: &[(&ViewInput, Bounds)],
    placements: &[Option<Placement>],
    section: &CutPlane,
    letter: &str,
) -> Option<Group> {
    let (i, (view, bounds)) = views.iter().enumerate().find(|(_, (v, _))| {
        v.name.is_some_and(|n| n != ViewName::Iso)
            && dot(v.spec.toward_eye, section.normal).abs() < 1e-9
    })?;
    let place = placements[i]?;
    let along3 = cross(section.normal, view.spec.toward_eye);
    let along = unit2([
        dot(along3, view.spec.x_axis),
        dot(along3, view.spec.y_axis()),
    ]);
    let through = view.spec.project(section.origin);
    // The line's run across the view's box.
    let corners = [
        bounds.min,
        [bounds.max[0], bounds.min[1]],
        bounds.max,
        [bounds.min[0], bounds.max[1]],
    ];
    let ts: Vec<f64> = corners
        .iter()
        .map(|c| dot2(sub2(*c, through), along))
        .collect();
    let t0 = ts.iter().copied().fold(f64::INFINITY, f64::min);
    let t1 = ts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let a = place.at(add2(through, mul2(along, t0)));
    let b = place.at(add2(through, mul2(along, t1)));
    let a = sub2(a, mul2(along, 6.0));
    let b = add2(b, mul2(along, 6.0));
    let mut g = Group::new("section-line");
    g.line(Pen::Chain, a, b);
    g.line(Pen::Thick, a, add2(a, mul2(along, 5.0)));
    g.line(Pen::Thick, b, sub2(b, mul2(along, 5.0)));
    // The direction of sight: against the normal.
    let sight3 = section.normal.map(|v| -v);
    let sight = unit2([
        dot(sight3, view.spec.x_axis),
        dot(sight3, view.spec.y_axis()),
    ]);
    for end in [a, b] {
        let start = sub2(end, mul2(sight, 8.0));
        g.line(Pen::Thin, start, sub2(end, mul2(sight, 0.5)));
        arrow(&mut g, sub2(end, mul2(sight, 0.5)), sight);
        let label = sub2(start, mul2(sight, 1.5));
        g.text(
            [label[0], label[1] - DIMENSION_TEXT / 2.0],
            5.0,
            0.0,
            Anchor::Middle,
            letter,
        );
    }
    Some(g)
}

/// A dimension of a sketch in a view that looks along its normal.
fn sketch_dimension(
    dim: &SketchDimension,
    spec: &ViewSpec,
    place: &Placement,
    units: Units,
) -> Option<Group> {
    let to_sheet = |p: Vec3| place.at(spec.project(p));
    match dim {
        SketchDimension::Linear {
            a,
            b,
            along,
            text,
            value,
            ..
        } => {
            let pa = to_sheet(*a);
            let pb = to_sheet(*b);
            let direction = match along {
                Some(v) => [dot(*v, spec.x_axis), dot(*v, spec.y_axis())],
                None => sub2(pb, pa),
            };
            if len2(direction) < 1e-9 {
                return None;
            }
            let d = unit2(direction);
            let n = perp(d);
            // The dimension line through the text, or 8 mm off.
            let mut offset = match text {
                Some(t) => dot2(sub2(to_sheet(*t), pa), n),
                None => 8.0,
            };
            if offset.abs() < 6.0 {
                offset = if offset < 0.0 { -6.0 } else { 6.0 };
            }
            let mut g = linear_dimension(pa, pb, d, offset, &units.length(value.abs()));
            g.class = "sketch-dimension";
            Some(g)
        }
        SketchDimension::Radial {
            center,
            radius,
            diameter,
            text,
            value,
            ..
        } => {
            let c = to_sheet(*center);
            let r = radius * place.scale;
            let direction = match text {
                Some(t) => unit2(sub2(to_sheet(*t), c)),
                None => [FRAC_1_SQRT_2, FRAC_1_SQRT_2],
            };
            let on = add2(c, mul2(direction, r));
            let end = add2(on, mul2(direction, 8.0));
            let mut g = Group::new("sketch-dimension");
            g.line(Pen::Thin, on, end);
            arrow(&mut g, on, mul2(direction, -1.0));
            let label = if *diameter {
                format!("Ø{}", units.length(*value))
            } else {
                format!("R{}", units.length(*value))
            };
            let anchor = if direction[0] >= 0.0 {
                Anchor::Start
            } else {
                Anchor::End
            };
            let shift = if direction[0] >= 0.0 { 1.0 } else { -1.0 };
            g.text(
                [end[0] + shift, end[1] - DIMENSION_TEXT / 2.0],
                DIMENSION_TEXT,
                0.0,
                anchor,
                label,
            );
            Some(g)
        }
    }
}

/// A text fitted into a cell's width: smaller down to 2 mm, then cut.
fn fitted(text: &str, height: f64, width: f64) -> (String, f64) {
    let w = text_width(text, height);
    if w <= width {
        return (text.to_owned(), height);
    }
    let smaller = height * width / w;
    if smaller >= 2.0 {
        return (text.to_owned(), smaller);
    }
    let mut cut: String = text.to_owned();
    while !cut.is_empty() && text_width(&format!("{cut}..."), 2.0) > width {
        cut.pop();
    }
    (format!("{cut}..."), 2.0)
}

/// ISO 7200's title block: the design, the bodies, the scale, the units,
/// the date, the sheet, the projection method and where it was made.
fn title_block(
    options: &DrawingOptions,
    units: Units,
    input: &SheetInput,
    scale: f64,
    min: P2,
    right: f64,
) -> Group {
    let mut g = Group::new("title-block");
    let x0 = min[0];
    let y0 = min[1];
    let w = right - x0;
    let top = y0 + TITLE_HEIGHT;
    g.rect(Pen::Frame, [x0, y0], [right, top]);
    let split = x0 + w * 2.0 / 3.0;
    let row1 = top - 12.0;
    let row2 = row1 - 10.0;
    g.line(Pen::Thin, [x0, row1], [right, row1]);
    g.line(Pen::Thin, [x0, row2], [right, row2]);
    g.line(Pen::Thin, [split, row2], [split, top]);
    let third = [x0 + w * 0.28, x0 + w * 0.5, x0 + w * 0.78];
    for x in third {
        g.line(Pen::Thin, [x, y0], [x, row2]);
    }
    let mut cell =
        |label: &str, value: &str, from: f64, to: f64, bottom: f64, cell_top: f64, size: f64| {
            g.text(
                [from + 1.5, cell_top - 1.5 - LABEL_TEXT],
                LABEL_TEXT,
                0.0,
                Anchor::Start,
                label,
            );
            let (text, h) = fitted(value, size, to - from - 3.0);
            g.text([from + 1.5, bottom + 2.0], h, 0.0, Anchor::Start, text);
        };
    let bodies = input.bodies.join(", ");
    cell("TITLE", &input.title, x0, split, row1, top, 5.0);
    cell("SCALE", &scale_text(scale), split, right, row1, top, 5.0);
    cell("BODY", &bodies, x0, split, row2, row1, DIMENSION_TEXT);
    cell(
        "UNITS",
        units.symbol,
        split,
        right,
        row2,
        row1,
        DIMENSION_TEXT,
    );
    cell("DATE", &input.date, x0, third[0], y0, row2, DIMENSION_TEXT);
    cell(
        "SHEET",
        options.sheet.name(),
        third[0],
        third[1],
        y0,
        row2,
        DIMENSION_TEXT,
    );
    let method = match options.projection {
        Projection::ThirdAngle => "THIRD ANGLE",
        Projection::FirstAngle => "FIRST ANGLE",
    };
    cell(
        &format!("PROJECTION: {method}"),
        "",
        third[1],
        third[2],
        y0,
        row2,
        DIMENSION_TEXT,
    );
    cell(
        "MADE WITH",
        "Mitcad",
        third[2],
        right,
        y0,
        row2,
        DIMENSION_TEXT,
    );
    // ISO 5456-2's symbol: a truncated cone's side view and its end view
    // (the end view to the right of the side view in third angle, to the
    // left in first angle).
    let cy = y0 + 3.5;
    let (cone_x, circles_x) = match options.projection {
        Projection::ThirdAngle => (third[1] + 3.0, third[1] + 12.0),
        Projection::FirstAngle => (third[1] + 11.0, third[1] + 3.0),
    };
    let r_big = 2.2;
    let r_small = 1.2;
    g.items.push(Item::Polyline {
        pen: Pen::Thin,
        points: vec![
            [cone_x, cy - r_big],
            [cone_x + 6.0, cy - r_small],
            [cone_x + 6.0, cy + r_small],
            [cone_x, cy + r_big],
        ],
        closed: true,
    });
    for r in [r_big, r_small] {
        g.items.push(Item::Arc {
            pen: Pen::Thin,
            center: [circles_x + r_big, cy],
            radius: r,
            start: 0.0,
            end: TAU,
        });
    }
    g
}

/// The axes of a section view of the plane.
pub fn section_spec(plane: &CutPlane, hidden: bool, smooth: bool, exact: bool) -> ViewSpec {
    let (toward_eye, x_axis) = super::section_axes(plane.normal);
    ViewSpec {
        toward_eye,
        x_axis,
        hidden,
        smooth,
        exact,
        section: Some(*plane),
        deflection: 0.01,
    }
}

/// The unit normal of a plane given by any normal.
pub fn cut_plane(origin: Vec3, normal: Vec3) -> Option<CutPlane> {
    unit(normal).map(|normal| CutPlane { origin, normal })
}
