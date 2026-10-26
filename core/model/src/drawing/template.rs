// SPDX-License-Identifier: MIT
//! Sheet templates and revisions (mitcad#707 phase 4).
//!
//! A **sheet template** is data, not code: the frame's margins, the title
//! block's size, its lines and its fields, where ISO 5456-2's projection
//! symbol goes, and whether a revision table sits above the block. A field
//! is a cell with a small label and a text whose names in braces are filled
//! in from the design when the sheet is drawn (`{title}`, `{material}`,
//! `{mass}`, `{Set/Key}`: `api/drawing_templates.rs`), so the block follows
//! the design.
//!
//! Templates come from three places: the built-in ones (ISO and ANSI, the
//! JSON files under `templates/`), the user's (`drawing-templates/` in the
//! user's library folder, beside its materials and print profiles,
//! [`crate::print_library::user_library`]) and a project's (the same
//! folder in the project's library, [`crate::print_library::project_library`]).
//! A sheet keeps a complete copy of the template it was given, so that a
//! drawing travels with its design and a library change never changes it.
//!
//! A drawing's **revisions** are rows of its revision table: a letter, a
//! description, a date and who made it; the title block's `{revision}` is
//! the latest one's letter.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::compose::{fitted, projection_symbol};
use super::sheet::{Anchor, Group, Pen};
use super::{P2, Projection, SheetSize};

/// The `format` of a template's file.
pub const TEMPLATE_FORMAT: &str = "mitcad-drawing-template";
/// The folder of templates in a user's or a project's library.
pub const TEMPLATES_DIR: &str = "drawing-templates";
/// The largest template file read.
const MAX_FILE: u64 = 1 << 20;

/// The built-in templates' data.
const BUILT_IN: [&str; 2] = [
    include_str!("templates/iso.json"),
    include_str!("templates/ansi.json"),
];

/// The height of a field's label and of a value without its own, mm.
const LABEL_TEXT: f64 = 2.0;
const VALUE_TEXT: f64 = 3.5;
/// A revision table's rows and texts, mm.
const REVISION_ROW: f64 = 5.0;
const REVISION_TEXT: f64 = 2.5;

fn default_margins() -> [f64; 4] {
    [20.0, 10.0, 10.0, 10.0]
}

fn default_rows() -> usize {
    4
}

/// A sheet template: its frame, title block and revision table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetTemplate {
    /// [`TEMPLATE_FORMAT`] in a library's file; left out in a sheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Unique in its library: its file's name without `.json`.
    pub id: String,
    pub name: String,
    /// Where it came from: `builtin`, `user` or `project` (given when it
    /// is read, left out of a library's file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<String>,
    /// The sheet size a drawing made with it starts with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<SheetSize>,
    /// The projection method a drawing made with it starts with (ISO: first
    /// angle, ANSI: third angle; mitcad#725).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<Projection>,
    /// The frame's distances from the sheet's edges: left, bottom, right,
    /// top, mm (ISO 5457: 20 on the left for filing, 10 elsewhere).
    #[serde(default = "default_margins")]
    pub margins: [f64; 4],
    pub title_block: BlockLayout,
    /// A revision table above the title block, drawn once the drawing has
    /// a revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_table: Option<RevisionLayout>,
}

/// A title block in the frame's lower right corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockLayout {
    /// mm.
    pub width: f64,
    pub height: f64,
    /// Thin lines in it, `[x0, y0, x1, y1]` in mm from its lower left
    /// corner; its outline is drawn wide.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<TemplateField>,
    /// Where the projection symbol starts (its left end, on its axis), mm
    /// from the block's lower left corner; no symbol when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_symbol: Option<P2>,
}

/// A cell of a title block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateField {
    /// Written small in its upper left corner (`TITLE`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Its value: text with names in braces filled in from the design.
    pub text: String,
    /// Its lower left corner, mm from the block's lower left corner.
    pub at: P2,
    /// Its width and height, mm.
    pub size: P2,
    /// The value's height of capitals, mm (3.5 when left out); a value too
    /// wide for the cell is made smaller, then cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
}

/// A revision table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionLayout {
    /// How many of the latest revisions it shows.
    #[serde(default = "default_rows")]
    pub rows: usize,
}

/// A row of a drawing's revision table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    /// `A`, `B`, …
    pub letter: String,
    pub description: String,
    /// YYYY-MM-DD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

impl Revision {
    pub fn check(&self) -> Result<(), String> {
        if self.letter.trim().is_empty() || self.letter.chars().any(char::is_whitespace) {
            return Err("a revision is named by a letter".to_owned());
        }
        if self.description.trim().is_empty() {
            return Err(format!("revision {} needs a description", self.letter));
        }
        Ok(())
    }
}

/// The letter of the revision after these: the one after the latest in
/// A, B, … Z, AA, AB, … (I, O and Q left out, as they read as digits), A
/// for the first; the first one none has after a letter of another kind.
pub fn next_revision_letter(revisions: &[Revision]) -> String {
    let letters: Vec<char> = super::def::LETTERS.chars().collect();
    let sequence: Vec<String> = letters
        .iter()
        .map(|c| c.to_string())
        .chain(
            letters
                .iter()
                .flat_map(|a| letters.iter().map(move |b| format!("{a}{b}"))),
        )
        .collect();
    let used = |l: &str| revisions.iter().any(|r| r.letter == l);
    let Some(last) = revisions.last() else {
        return "A".to_owned();
    };
    let after = sequence
        .iter()
        .position(|l| *l == last.letter)
        .map_or(sequence.len(), |i| i + 1);
    sequence[after..]
        .iter()
        .find(|l| !used(l))
        .or_else(|| sequence.iter().find(|l| !used(l)))
        .cloned()
        .unwrap_or_else(|| format!("{}1", last.letter))
}

impl SheetTemplate {
    /// Why it cannot be used, if it cannot.
    pub fn check(&self) -> Result<(), String> {
        let at = format!("template '{}'", self.id);
        if let Some(format) = &self.format
            && format != TEMPLATE_FORMAT
        {
            return Err(format!(
                "{at}: its format is {TEMPLATE_FORMAT}, not {format}"
            ));
        }
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "{at}: a template's id is letters, digits, '-' and '_'"
            ));
        }
        if self.name.trim().is_empty() {
            return Err(format!("{at}: a template needs a name"));
        }
        if !self.margins.iter().all(|m| m.is_finite() && *m >= 0.0) {
            return Err(format!("{at}: its margins are not negative"));
        }
        let block = &self.title_block;
        if !(block.width.is_finite() && block.width > 0.0)
            || !(block.height.is_finite() && block.height > 0.0)
        {
            return Err(format!("{at}: its title block has a size"));
        }
        if !block.lines.iter().flatten().all(|v| v.is_finite())
            || !block
                .projection_symbol
                .iter()
                .flatten()
                .all(|v| v.is_finite())
        {
            return Err(format!("{at}: its lines and symbol are points"));
        }
        for field in &block.fields {
            let finite = field.at.iter().chain(&field.size).all(|v| v.is_finite());
            if !finite || field.size[0] <= 0.0 || field.size[1] <= 0.0 {
                return Err(format!("{at}: the field '{}' has no cell", field.text));
            }
            if let Some(h) = field.height
                && !(h.is_finite() && h > 0.0)
            {
                return Err(format!("{at}: the field '{}' has no height", field.text));
            }
        }
        if let Some(table) = &self.revision_table
            && !(1..=50).contains(&table.rows)
        {
            return Err(format!("{at}: a revision table shows 1 to 50 rows"));
        }
        // Its own sheet size, as a drawing made with it starts.
        if let Some(size) = self.size {
            let (width, height) = size.landscape();
            self.fits(width, height)?;
        }
        Ok(())
    }

    /// Why it does not fit a sheet of this size, if it does not
    /// (mitcad#722: large margins on A4 gave a frame of negative width):
    /// its margins leave a frame, and its title block with the revision
    /// table's room fits in the frame.
    pub fn fits(&self, width: f64, height: f64) -> Result<(), String> {
        let at = format!("template '{}'", self.id);
        let (min, max) = self.frame(width, height);
        let (w, h) = (max[0] - min[0], max[1] - min[1]);
        if !(w > 0.0 && h > 0.0) {
            return Err(format!(
                "{at}: its margins leave no frame on a {width} x {height} mm sheet"
            ));
        }
        let block = &self.title_block;
        let high = block.height + self.revision_room();
        if block.width > w + 1e-6 || high > h + 1e-6 {
            return Err(format!(
                "{at}: its title block and revision table ({} x {high} mm) do not fit in its \
                 frame ({w} x {h} mm) on a {width} x {height} mm sheet",
                block.width
            ));
        }
        Ok(())
    }

    /// The frame's corners on a sheet of this size.
    pub fn frame(&self, width: f64, height: f64) -> (P2, P2) {
        let [left, bottom, right, top] = self.margins;
        ([left, bottom], [width - right, height - top])
    }

    /// The title block's box on a sheet of this size: in the frame's lower
    /// right corner, no wider than the frame.
    pub fn title_box(&self, width: f64, height: f64) -> (P2, P2) {
        let (min, max) = self.frame(width, height);
        let w = self.title_block.width.min(max[0] - min[0]);
        (
            [max[0] - w, min[1]],
            [max[0], min[1] + self.title_block.height],
        )
    }

    /// The height its revision table takes above the title block when it
    /// shows all its rows and its header, mm; 0 without one. Views and the
    /// hole table keep clear of it from the start, so that the first
    /// revision overlaps nothing (mitcad#722).
    pub fn revision_room(&self) -> f64 {
        self.revision_table
            .as_ref()
            .map_or(0.0, |t| REVISION_ROW * (t.rows.max(1) + 1) as f64)
    }

    /// The names its fields fill in, each once, in order.
    pub fn keys(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for field in &self.title_block.fields {
            for key in keys(&field.text) {
                if !out.contains(&key) {
                    out.push(key);
                }
            }
        }
        out
    }

    /// What a drawing's sheet keeps of it: without its file's format.
    pub fn for_sheet(mut self) -> Self {
        self.format = None;
        self
    }

    /// Its choice in commands and lists: a built-in one's id (`iso`), else
    /// `user:<id>` or `project:<id>`.
    pub fn choice(&self) -> String {
        match self.library.as_deref() {
            None | Some("builtin") => self.id.clone(),
            Some(library) => format!("{library}:{}", self.id),
        }
    }
}

/// A text with the names in its braces filled in by `value`; a brace that
/// is not closed stays as it is.
pub fn fill(text: &str, value: &mut dyn FnMut(&str) -> String) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                out.push_str(&value(after[..close].trim()));
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The names in a text's braces.
pub fn keys(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    fill(text, &mut |key| {
        out.push(key.to_owned());
        String::new()
    });
    out
}

/// A template with its fields filled in, as a sheet draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct Filled {
    pub template: SheetTemplate,
    /// Each field's value, in the template's order.
    pub values: Vec<String>,
    /// The drawing's revisions, the latest last.
    pub revisions: Vec<Revision>,
    pub projection: Projection,
}

/// The groups a filled template draws on a sheet of this size: `frame`,
/// `title-block` and, once the drawing has a revision, `revision-table`.
pub fn draw(filled: &Filled, width: f64, height: f64) -> Vec<Group> {
    let t = &filled.template;
    let (frame_min, frame_max) = t.frame(width, height);
    let mut frame = Group::new("frame");
    frame.rect(Pen::Frame, frame_min, frame_max);
    let (min, max) = t.title_box(width, height);
    let mut block = Group::new("title-block");
    block.rect(Pen::Frame, min, max);
    let at = |p: P2| [min[0] + p[0], min[1] + p[1]];
    for l in &t.title_block.lines {
        block.line(Pen::Thin, at([l[0], l[1]]), at([l[2], l[3]]));
    }
    for (field, value) in t.title_block.fields.iter().zip(&filled.values) {
        let lo = at(field.at);
        let top = lo[1] + field.size[1];
        if !field.label.is_empty() {
            block.text(
                [lo[0] + 1.5, top - 1.5 - LABEL_TEXT],
                LABEL_TEXT,
                0.0,
                Anchor::Start,
                field.label.clone(),
            );
        }
        if !value.trim().is_empty() {
            let letters = field.height.unwrap_or(VALUE_TEXT);
            let (text, h) = fitted(value, letters, field.size[0] - 3.0);
            block.text([lo[0] + 1.5, lo[1] + 2.0], h, 0.0, Anchor::Start, text);
        }
    }
    if let Some(p) = t.title_block.projection_symbol {
        projection_symbol(&mut block, filled.projection, at(p));
    }
    let mut out = vec![frame, block];
    if let Some(layout) = &t.revision_table
        && !filled.revisions.is_empty()
    {
        out.push(revision_table(
            &filled.revisions,
            layout.rows,
            [min[0], max[1]],
            max[0],
        ));
    }
    out
}

/// A revision table from `corner` (its lower left corner) to `right`: its
/// header (REV, DESCRIPTION, DATE, BY) at the bottom, the latest `rows`
/// revisions above it, the latest on top.
fn revision_table(revisions: &[Revision], rows: usize, corner: P2, right: f64) -> Group {
    let [left, bottom] = corner;
    let width = right - left;
    let columns = [12.0, (width - 64.0).max(10.0), 24.0, 28.0];
    let shown = &revisions[revisions.len().saturating_sub(rows.max(1))..];
    let lines = shown.len() + 1;
    let top = bottom + REVISION_ROW * lines as f64;
    let mut g = Group::new("revision-table");
    g.rect(Pen::Thin, [left, bottom], [right, top]);
    let mut x = left;
    for w in &columns[..3] {
        x += w;
        g.line(Pen::Thin, [x, bottom], [x, top]);
    }
    for i in 1..lines {
        let y = bottom + REVISION_ROW * i as f64;
        g.line(Pen::Thin, [left, y], [right, y]);
    }
    let mut row = |y: f64, cells: [&str; 4]| {
        let mut x = left;
        for (cell, w) in cells.iter().zip(columns) {
            if !cell.trim().is_empty() {
                let (text, h) = fitted(cell, REVISION_TEXT, w - 3.0);
                g.text([x + 1.5, y + 1.5], h, 0.0, Anchor::Start, text);
            }
            x += w;
        }
    };
    row(bottom, ["REV", "DESCRIPTION", "DATE", "BY"]);
    for (i, r) in shown.iter().enumerate() {
        row(
            bottom + REVISION_ROW * (i + 1) as f64,
            [
                r.letter.as_str(),
                r.description.as_str(),
                r.date.as_deref().unwrap_or(""),
                r.author.as_deref().unwrap_or(""),
            ],
        );
    }
    g
}

// --- Where templates come from ----------------------------------------------

/// The built-in templates: ISO (A sizes) and ANSI.
pub fn built_in() -> Vec<SheetTemplate> {
    BUILT_IN
        .iter()
        .map(|text| {
            let mut t: SheetTemplate = serde_json::from_str(text).expect("a built-in template");
            t.format = None;
            t.library = Some("builtin".to_owned());
            t
        })
        .collect()
}

/// The folder of a library's templates: the user's (`user`) or the
/// project's (`project`, given its folder).
pub fn library_folder(library: &str, project: Option<&Path>) -> Result<PathBuf, String> {
    match library {
        "user" => crate::print_library::user_library()
            .map(|folder| folder.join(TEMPLATES_DIR))
            .ok_or_else(|| "no user library folder is set".to_owned()),
        "project" => project
            .map(|p| crate::print_library::project_library(p).join(TEMPLATES_DIR))
            .ok_or_else(|| "the design is in no project".to_owned()),
        other => Err(format!(
            "templates are saved to the 'user' or the 'project' library, not '{other}'"
        )),
    }
}

/// A template file's data read, its id from its file's name.
pub fn parse_file(path: &Path, data: &[u8], library: &str) -> Result<SheetTemplate, String> {
    let name = path.display();
    let mut t: SheetTemplate =
        serde_json::from_slice(data).map_err(|e| format!("{name}: not a sheet template: {e}"))?;
    if t.format.as_deref() != Some(TEMPLATE_FORMAT) {
        return Err(format!("{name}: its format is not {TEMPLATE_FORMAT}"));
    }
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        t.id = stem.to_owned();
    }
    t.check().map_err(|e| format!("{name}: {e}"))?;
    t.format = None;
    t.library = Some(library.to_owned());
    Ok(t)
}

/// The templates of a library's folder, sorted by name; what could not be
/// read goes to `problems`.
fn read_folder(folder: &Path, library: &str, problems: &mut Vec<String>) -> Vec<SheetTemplate> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| e.metadata().is_ok_and(|m| m.len() <= MAX_FILE))
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    for path in paths {
        match std::fs::read(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|data| parse_file(&path, &data, library))
        {
            Ok(t) => out.push(t),
            Err(problem) => problems.push(problem),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    out
}

/// Every template a design can choose from: the built-in ones, the
/// user's, the project's (its folder given); and what could not be read.
pub fn list(project: Option<&Path>) -> (Vec<SheetTemplate>, Vec<String>) {
    let user = library_folder("user", None).ok();
    let project = library_folder("project", project).ok();
    list_in(user.as_deref(), project.as_deref())
}

/// [`list`] from these folders of the user's and the project's templates
/// (a test's own, mitcad#722).
fn list_in(user: Option<&Path>, project: Option<&Path>) -> (Vec<SheetTemplate>, Vec<String>) {
    let mut problems = Vec::new();
    let mut out = built_in();
    for (library, folder) in [("user", user), ("project", project)] {
        if let Some(folder) = folder {
            out.extend(read_folder(folder, library, &mut problems));
        }
    }
    (out, problems)
}

/// Writes a template to a library (`user` or `project`) as
/// `drawing-templates/<id>.json`, replacing one of its id; returns the
/// file's path.
pub fn save(
    template: &SheetTemplate,
    library: &str,
    project: Option<&Path>,
) -> Result<PathBuf, String> {
    template.check()?;
    let folder = library_folder(library, project)?;
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut file = template.clone();
    file.format = Some(TEMPLATE_FORMAT.to_owned());
    file.library = None;
    let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    let path = folder.join(format!("{}.json", template.id));
    std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::sheet::Item;

    #[test]
    fn the_built_in_templates_read_and_check() {
        let all = built_in();
        let ids: Vec<&str> = all.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["iso", "ansi"]);
        for t in &all {
            t.check().unwrap();
            assert_eq!(t.choice(), t.id);
            // Every field inside its block.
            let b = &t.title_block;
            for f in &b.fields {
                assert!(
                    f.at[0] >= 0.0
                        && f.at[1] >= 0.0
                        && f.at[0] + f.size[0] <= b.width + 1e-9
                        && f.at[1] + f.size[1] <= b.height + 1e-9,
                    "{}: {f:?}",
                    t.id
                );
            }
        }
        let iso = &all[0];
        for key in [
            "title", "number", "material", "mass", "scale", "revision", "sheet",
        ] {
            assert!(iso.keys().iter().any(|k| k == key), "{key}");
        }
        assert_eq!(iso.size, Some(SheetSize::A3));
        assert_eq!(all[1].size, Some(SheetSize::AnsiB));
        // Their projection methods (mitcad#725).
        assert_eq!(iso.projection, Some(Projection::FirstAngle));
        assert_eq!(all[1].projection, Some(Projection::ThirdAngle));
    }

    #[test]
    fn fields_are_filled_and_names_found() {
        let mut value = |key: &str| match key {
            "title" => "Bracket".to_owned(),
            "sheet_number" => "1".to_owned(),
            "sheets" => "2".to_owned(),
            other => format!("<{other}>"),
        };
        assert_eq!(fill("{title}", &mut value), "Bracket");
        assert_eq!(fill("{sheet_number} OF {sheets}", &mut value), "1 OF 2");
        assert_eq!(fill("{ Shop/Supplier }", &mut value), "<Shop/Supplier>");
        assert_eq!(fill("open { brace", &mut value), "open { brace");
        assert_eq!(fill("plain", &mut value), "plain");
        assert_eq!(keys("{a} and {b}"), ["a", "b"]);
    }

    #[test]
    fn revision_letters_follow_the_latest() {
        let rev = |letter: &str| Revision {
            letter: letter.to_owned(),
            description: "x".to_owned(),
            date: None,
            author: None,
        };
        assert_eq!(next_revision_letter(&[]), "A");
        assert_eq!(next_revision_letter(&[rev("A")]), "B");
        assert_eq!(next_revision_letter(&[rev("A"), rev("H")]), "J");
        assert_eq!(next_revision_letter(&[rev("Z")]), "AA");
        assert_eq!(next_revision_letter(&[rev("B"), rev("1")]), "A");
        assert!(rev(" ").check().is_err());
        assert!(
            Revision {
                description: " ".to_owned(),
                ..rev("A")
            }
            .check()
            .is_err()
        );
    }

    #[test]
    fn a_filled_template_draws_its_frame_block_and_revisions() {
        let iso = built_in().remove(0);
        let values: Vec<String> = iso
            .title_block
            .fields
            .iter()
            .map(|f| fill(&f.text, &mut |k| k.to_uppercase()))
            .collect();
        let mut filled = Filled {
            template: iso,
            values,
            revisions: Vec::new(),
            projection: Projection::ThirdAngle,
        };
        let texts = |groups: &[Group]| -> Vec<String> {
            groups
                .iter()
                .flat_map(|g| &g.items)
                .filter_map(|i| match i {
                    Item::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect()
        };
        let groups = draw(&filled, 420.0, 297.0);
        let classes: Vec<&str> = groups.iter().map(|g| g.class).collect();
        assert_eq!(classes, ["frame", "title-block"]);
        let all = texts(&groups);
        for text in ["TITLE", "TITLE", "MATERIAL", "MASS", "REVISION"] {
            assert!(all.iter().any(|t| t == text), "{text}: {all:?}");
        }
        // The frame inside the margins; the block in its lower right corner.
        let (min, max) = filled.template.title_box(420.0, 297.0);
        assert_eq!(min, [230.0, 10.0]);
        assert_eq!(max, [410.0, 46.0]);
        // With revisions, the latest four above the block.
        filled.revisions = (0..6)
            .map(|i| Revision {
                letter: ["A", "B", "C", "D", "E", "F"][i].to_owned(),
                description: format!("change {i}"),
                date: Some("2026-10-26".to_owned()),
                author: Some("JD".to_owned()),
            })
            .collect();
        let groups = draw(&filled, 420.0, 297.0);
        let table = groups
            .iter()
            .find(|g| g.class == "revision-table")
            .expect("a revision table");
        let all = texts(std::slice::from_ref(table));
        assert!(all.iter().any(|t| t == "DESCRIPTION"));
        assert!(all.iter().any(|t| t == "change 5"));
        assert!(!all.iter().any(|t| t == "change 1"), "{all:?}");
    }

    #[test]
    fn a_template_must_fit_its_sheet() {
        // mitcad#722: large margins on A4 gave a frame of negative width.
        let iso = built_in().remove(0);
        // Its block fits A4 upright, just.
        iso.fits(210.0, 297.0).unwrap();
        let mut wide = iso.clone();
        wide.margins = [150.0, 10.0, 150.0, 10.0];
        let error = wide.fits(297.0, 210.0).unwrap_err();
        assert!(error.contains("leave no frame"), "{error}");
        // Checked against its own size.
        wide.size = Some(SheetSize::A4);
        assert!(wide.check().unwrap_err().contains("leave no frame"));
        // A block (with the revision table's room) higher than the frame.
        let mut high = iso.clone();
        high.title_block.height = 260.0;
        let error = high.fits(420.0, 297.0).unwrap_err();
        assert!(error.contains("do not fit"), "{error}");
        high.revision_table = None;
        high.fits(420.0, 297.0).unwrap();
        // A block wider than the frame.
        let mut broad = iso;
        broad.title_block.width = 300.0;
        broad.size = Some(SheetSize::A4);
        assert!(broad.check().unwrap_err().contains("do not fit"));
    }

    #[test]
    fn templates_are_checked_and_saved_to_a_library() {
        let mut t = built_in().remove(0);
        t.id = "bad id".to_owned();
        assert!(t.check().unwrap_err().contains("id is letters"));
        t.id = "company-a3".to_owned();
        t.title_block.width = 0.0;
        assert!(t.check().unwrap_err().contains("has a size"));
        t.title_block.width = 180.0;
        t.revision_table = Some(RevisionLayout { rows: 0 });
        assert!(t.check().unwrap_err().contains("1 to 50"));
        t.revision_table = None;
        t.check().unwrap();
        // Saved to a project's library and read back from it; the user's
        // library a folder of the test's own, so that a developer's own
        // templates (`MITCAD_PRINT_LIBRARY_DIR`) play no part (mitcad#722).
        let own = own_folder("templates");
        let project = own.join("project");
        let user = own.join("user");
        std::fs::create_dir_all(&user).unwrap();
        let path = save(&t, "project", Some(project.as_path())).unwrap();
        assert!(
            path.ends_with("drawing-templates/company-a3.json"),
            "{path:?}"
        );
        let folder = library_folder("project", Some(project.as_path())).unwrap();
        let (all, problems) = list_in(Some(user.as_path()), Some(folder.as_path()));
        assert!(problems.is_empty(), "{problems:?}");
        let read = all.iter().find(|r| r.id == "company-a3").expect("listed");
        assert_eq!(read.library.as_deref(), Some("project"));
        assert_eq!(read.choice(), "project:company-a3");
        assert_eq!(read.title_block, t.title_block);
        // A file that is no template is a problem, not a template.
        std::fs::write(path.with_file_name("broken.json"), "{}").unwrap();
        let (_, problems) = list_in(Some(user.as_path()), Some(folder.as_path()));
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            save(&t, "builtin", Some(project.as_path()))
                .unwrap_err()
                .contains("'user' or the 'project'")
        );
        std::fs::remove_dir_all(&own).ok();
    }

    /// A fresh folder of a test's own beside the test program in the build
    /// tree, not in the system's temporary folder.
    fn own_folder(name: &str) -> PathBuf {
        let exe = std::env::current_exe().expect("the test program's path");
        let dir = exe
            .parent()
            .expect("the test program's folder")
            .join("test-folders")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
