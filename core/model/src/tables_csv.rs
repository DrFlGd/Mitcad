// SPDX-License-Identifier: MIT
//! A table of the design as CSV (mitcad#804; `docs/table-lookups.md`, "CSV
//! import and export of a table"): one file per table, as spreadsheets
//! export one sheet per file.
//!
//! The header has one field per column, its name with its unit in
//! brackets: `Size [Text],d [mm],dk [mm],Pitch [mm]`, `[]` for unitless.
//! The file is UTF-8 (a byte order mark is skipped) with RFC 4180 quoting
//! and CRLF, read and written as the parameters CSV is
//! (`parameters_csv.rs`); a header with semicolons and no comma is read
//! with `;` between fields.
//!
//! - **Cells.** A number cell is read as a constant expression in its
//!   column's unit: `5,5` (a decimal comma) is 5.5, a plain `5` in an `mm`
//!   column is 5 mm, and `0.25 in` there is stored as 6.35. A cell that is
//!   not a number of the column's kind (`abc` or `5 deg` in an `mm`
//!   column, a name) is refused with the reason. A `Text` column takes its
//!   cells as they are. White space around a cell is dropped; an empty cell
//!   is empty.
//! - **Inferred columns.** A header without brackets is a `Text` column
//!   when any of its cells is not a number; else a column of the unit of
//!   its first cell with a unit (`12 mm` makes an `mm` column, in the
//!   design's default units), and a unitless one when none has a unit.
//! - **Names.** A column name the model cannot use (`Head diameter`, `2nd`,
//!   a second `d`) gets a free one (`Head_diameter`, `c_2nd`, `d_1`), as
//!   the parameters CSV renames. Column names may be reserved words (`in`,
//!   `Max`): a column is only ever written after `Table.`.
//! - **Export** writes every unit in brackets (`[Text]` too) and numbers
//!   with decimal points, the shortest decimal that gives the value back,
//!   so export and import give the same table.
//!
//! What an import does with the table (the check of the parameters that
//! use it, the undo step) is `document/tables_csv.rs`.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Serialize, Serializer};

use crate::expr::{
    Dims, EvalContext, MAX_TEXT_BYTES, ParameterTable, Unit, Value, format_number_exact,
    with_decimal_points,
};
use crate::parameters::unit_for;
use crate::parameters_csv::{self, CsvError, write_record};
// The sizes and column names of the design's tables (mitcad#803).
pub use crate::tables::{MAX_COLUMNS, MAX_ROWS, is_column_name};

/// A cell as the CSV reads and writes it: a number in its column's unit, a
/// text, or empty. As JSON a number, a string or null, as the design file
/// keeps a table's cells.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum CsvCell {
    Number(f64),
    Text(String),
    Empty,
}

/// A column of a table read from CSV.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CsvColumn {
    /// The name in the file (without its unit).
    pub name: String,
    /// The name in the table, when the file's cannot be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renamed: Option<String>,
    #[serde(serialize_with = "unit_text")]
    pub unit: Unit,
    /// The header had no unit: the unit comes from the cells.
    pub inferred: bool,
}

impl CsvColumn {
    /// The column's name in the table.
    pub fn table_name(&self) -> &str {
        self.renamed.as_deref().unwrap_or(&self.name)
    }
}

/// A unit as the `parameters` query gives it: `mm`, `""`, `Text`.
fn unit_text<S: Serializer>(unit: &Unit, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&unit.to_string())
}

/// A cell the import cannot take: it is left empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefusedCell {
    /// The line of the file the cell's row starts on (the header is line
    /// 1).
    pub line: usize,
    /// The column's name in the table; empty for a cell beyond the last
    /// column.
    pub column: String,
    /// The cell as the file has it.
    pub text: String,
    pub reason: String,
}

/// A table read from CSV: its columns, its rows (each with a cell per
/// column, a refused cell empty) and the cells refused.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableCsv {
    pub columns: Vec<CsvColumn>,
    pub rows: Vec<Vec<CsvCell>>,
    /// The line each row starts on.
    pub lines: Vec<usize>,
    pub refused: Vec<RefusedCell>,
}

/// Why a text is not a table CSV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableCsvError {
    /// Nothing but white space, or no column.
    Empty,
    /// A quoted field that is not closed, from this line on.
    OpenQuote {
        line: usize,
    },
    /// A unit in brackets that is not one.
    Unit {
        column: String,
        unit: String,
        reason: String,
    },
    TooManyColumns(usize),
    TooManyRows(usize),
}

impl fmt::Display for TableCsvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("the CSV file is empty"),
            Self::OpenQuote { line } => {
                write!(f, "line {line}: a quoted field is not closed")
            }
            Self::Unit {
                column,
                unit,
                reason,
            } => write!(f, "column {column}: the unit [{unit}]: {reason}"),
            Self::TooManyColumns(n) => write!(
                f,
                "the CSV file has {n} columns; a table has at most {MAX_COLUMNS}"
            ),
            Self::TooManyRows(n) => write!(
                f,
                "the CSV file has {n} rows; a table has at most {MAX_ROWS}"
            ),
        }
    }
}

impl std::error::Error for TableCsvError {}

/// A header field as its name and the unit in its brackets (None without
/// brackets): `d [mm]` is `d` and `mm`, `n []` is `n` and `""`.
fn split_header(field: &str) -> (String, Option<String>) {
    let field = field.trim();
    if let Some(open) = field.rfind('[')
        && field.ends_with(']')
    {
        let unit = field[open + 1..field.len() - 1].trim();
        return (field[..open].trim().to_owned(), Some(unit.to_owned()));
    }
    (field.to_owned(), None)
}

/// The base of a free name for a column the model cannot use: other
/// characters as `_`, `c_` before a name that does not start with a letter,
/// `column_<n>` (its place in the file, from 1) for an empty one.
fn column_base(name: &str, place: usize) -> String {
    let base: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if base.is_empty() {
        format!("column_{place}")
    } else if base.starts_with(|c: char| c.is_alphabetic() || c == '_') {
        base
    } else {
        format!("c_{base}")
    }
}

/// The table names of columns named `names` in the file (with their places
/// in it): None where the file's name is used, else a free one that no
/// other column has or can use.
fn column_names(names: &[(String, usize)]) -> Vec<Option<String>> {
    let taken: BTreeSet<&str> = names
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| is_column_name(name))
        .collect();
    let mut used: BTreeSet<String> = BTreeSet::new();
    names
        .iter()
        .map(|(name, place)| {
            if is_column_name(name) && used.insert(name.clone()) {
                return None;
            }
            // (A base and its numbered forms are column names.)
            let base = column_base(name, *place);
            let free = |c: &str| !taken.contains(c) && !used.contains(c);
            let new = if free(&base) {
                base
            } else {
                (1u32..)
                    .map(|n| format!("{base}_{n}"))
                    .find(|candidate| free(candidate))
                    .expect("a free name exists")
            };
            used.insert(new.clone());
            Some(new)
        })
        .collect()
}

/// A cell that is a plain number (digits, a sign, a decimal point or
/// comma, an exponent): its value as it is, so that a number read in its
/// unit is not converted there and back.
fn plain_number(text: &str) -> Option<f64> {
    let text = with_decimal_points(text);
    let plain = |c: char| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E');
    if !text.chars().all(plain) {
        return None;
    }
    text.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The dimensions of a cell that is a number (a constant expression), None
/// for any other cell.
fn number_dims(text: &str, constants: &ParameterTable) -> Option<Dims> {
    if plain_number(text).is_some() {
        return Some(Dims::NONE);
    }
    match constants.evaluate_value(text) {
        Ok(Value::Number(q)) if q.value.is_finite() => Some(q.dims),
        _ => None,
    }
}

/// The unit of a column whose header has none, from its cells (not empty):
/// `Text` when one is not a number, else the unit of the first with a unit
/// in the default units of `context`, else unitless.
fn inferred_unit<'a>(
    cells: impl Iterator<Item = &'a str>,
    constants: &ParameterTable,
    context: &EvalContext,
) -> Unit {
    let mut dims = None;
    for text in cells {
        match number_dims(text, constants) {
            None => return Unit::TEXT,
            Some(found) if dims.is_none() && !found.is_dimensionless() => dims = Some(found),
            Some(_) => {}
        }
    }
    dims.and_then(|dims| unit_for(dims, context))
        .unwrap_or(Unit::NONE)
}

/// A cell (trimmed) of a column of `unit`; the reason when it is refused.
fn read_cell(text: &str, unit: Unit, constants: &ParameterTable) -> Result<CsvCell, String> {
    if text.is_empty() {
        return Ok(CsvCell::Empty);
    }
    if unit.is_text() {
        if text.len() > MAX_TEXT_BYTES {
            return Err(format!(
                "the text has {} bytes; a text has at most {MAX_TEXT_BYTES}",
                text.len()
            ));
        }
        return Ok(CsvCell::Text(text.to_owned()));
    }
    if let Some(value) = plain_number(text) {
        return Ok(CsvCell::Number(value));
    }
    // A constant expression: a cell names no parameter.
    let quantity = constants.evaluate(text, unit).map_err(|e| e.to_string())?;
    let value = unit.canonical_to_unit(quantity.value);
    if value.is_finite() {
        Ok(CsvCell::Number(value))
    } else {
        Err("the value is not a finite number".to_owned())
    }
}

/// The cell `i` of a record, trimmed; empty where the record is shorter.
fn cell(cells: &[String], i: usize) -> &str {
    cells.get(i).map_or("", |c| c.trim())
}

/// Reads a table CSV; `context` gives the default units of inferred
/// columns and of unitless values in expressions.
pub fn parse(text: &str, context: EvalContext) -> Result<TableCsv, TableCsvError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let separator = parameters_csv::separator(text);
    let records = parameters_csv::records(text, separator).map_err(|e| match e {
        CsvError::OpenQuote { line } => TableCsvError::OpenQuote { line },
        _ => TableCsvError::Empty,
    })?;
    let mut records = records.into_iter();
    let Some((_, header)) = records.next() else {
        return Err(TableCsvError::Empty);
    };
    let mut body: Vec<(usize, Vec<String>)> = records.collect();
    // Rows of empty cells at the end are a spreadsheet's formatting.
    let empty = |cells: &[String]| cells.iter().all(|c| c.trim().is_empty());
    while body.last().is_some_and(|(_, cells)| empty(cells)) {
        body.pop();
    }
    // A column without a name and without cells is a separator too many.
    let has_cells = |i: usize| body.iter().any(|(_, cells)| !cell(cells, i).is_empty());
    let kept: Vec<usize> = (0..header.len())
        .filter(|&i| !header[i].trim().is_empty() || has_cells(i))
        .collect();
    if kept.is_empty() {
        return Err(TableCsvError::Empty);
    }
    if kept.len() > MAX_COLUMNS {
        return Err(TableCsvError::TooManyColumns(kept.len()));
    }
    if body.len() > MAX_ROWS {
        return Err(TableCsvError::TooManyRows(body.len()));
    }
    let heads: Vec<(String, Option<String>)> =
        kept.iter().map(|&i| split_header(&header[i])).collect();
    let places: Vec<(String, usize)> = heads
        .iter()
        .zip(&kept)
        .map(|((name, _), &i)| (name.clone(), i + 1))
        .collect();
    let renamed = column_names(&places);
    // Cells are constants: they are evaluated against no parameters.
    let constants = ParameterTable::new(context);
    let mut columns = Vec::with_capacity(kept.len());
    for ((&i, (name, unit)), renamed) in kept.iter().zip(&heads).zip(renamed) {
        let (unit, inferred) = match unit {
            Some(text) => {
                let unit = Unit::parse(text).map_err(|e| TableCsvError::Unit {
                    column: name.clone(),
                    unit: text.clone(),
                    reason: e.to_string(),
                })?;
                (unit, false)
            }
            None => {
                let cells = body
                    .iter()
                    .map(|(_, cells)| cell(cells, i))
                    .filter(|c| !c.is_empty());
                (inferred_unit(cells, &constants, &context), true)
            }
        };
        columns.push(CsvColumn {
            name: name.clone(),
            renamed,
            unit,
            inferred,
        });
    }
    let mut table = TableCsv {
        columns,
        ..TableCsv::default()
    };
    for (line, cells) in &body {
        let mut row = Vec::with_capacity(kept.len());
        for (column, &i) in table.columns.iter().zip(&kept) {
            let text = cell(cells, i);
            match read_cell(text, column.unit, &constants) {
                Ok(read) => row.push(read),
                Err(reason) => {
                    table.refused.push(RefusedCell {
                        line: *line,
                        column: column.table_name().to_owned(),
                        text: text.to_owned(),
                        reason,
                    });
                    row.push(CsvCell::Empty);
                }
            }
        }
        for text in cells.iter().skip(header.len()).map(|c| c.trim()) {
            if !text.is_empty() {
                table.refused.push(RefusedCell {
                    line: *line,
                    column: String::new(),
                    text: text.to_owned(),
                    reason: "the row has more cells than the header has columns".to_owned(),
                });
            }
        }
        table.rows.push(row);
        table.lines.push(*line);
    }
    Ok(table)
}

/// Appends one record (`write_record` of fields kept as strings).
fn record(out: &mut String, fields: &[String]) {
    let fields: Vec<&str> = fields.iter().map(String::as_str).collect();
    write_record(out, &fields);
}

/// A table as CSV: the header with every unit in brackets, then the rows
/// (a missing cell empty).
pub fn write(columns: &[(String, Unit)], rows: &[Vec<CsvCell>]) -> String {
    let mut out = String::new();
    let header: Vec<String> = columns
        .iter()
        .map(|(name, unit)| format!("{name} [{unit}]"))
        .collect();
    record(&mut out, &header);
    for row in rows {
        let fields: Vec<String> = (0..columns.len())
            .map(|i| match row.get(i) {
                Some(CsvCell::Number(value)) => format_number_exact(*value),
                Some(CsvCell::Text(text)) => text.clone(),
                Some(CsvCell::Empty) | None => String::new(),
            })
            .collect();
        if fields.iter().all(String::is_empty) {
            // An empty row stays a row: a lone empty line would be skipped.
            out.push_str("\"\"");
            out.push_str(&",".repeat(columns.len().saturating_sub(1)));
            out.push_str("\r\n");
            continue;
        }
        record(&mut out, &fields);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> TableCsv {
        parse(text, EvalContext::default()).unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    fn number(cell: &CsvCell) -> f64 {
        match cell {
            CsvCell::Number(value) => *value,
            other => panic!("not a number: {other:?}"),
        }
    }

    fn text(value: &str) -> CsvCell {
        CsvCell::Text(value.to_owned())
    }

    fn units(table: &TableCsv) -> Vec<String> {
        table.columns.iter().map(|c| c.unit.to_string()).collect()
    }

    #[test]
    fn the_header_has_names_and_units() {
        let table = read(
            "\u{feff}Size [Text],d [mm],Head diameter [mm],d [in],2nd [],in [deg],[mm^2]\r\n\
             M5,5,8.5,0.2,3,90,12\r\n",
        );
        let names: Vec<&str> = table.columns.iter().map(CsvColumn::table_name).collect();
        assert_eq!(
            names,
            [
                "Size",
                "d",
                "Head_diameter",
                "d_1",
                "c_2nd",
                "in",
                "column_7"
            ]
        );
        assert_eq!(table.columns[2].name, "Head diameter");
        assert_eq!(table.columns[2].renamed.as_deref(), Some("Head_diameter"));
        // A reserved word names a column as it is.
        assert_eq!(table.columns[5].renamed, None);
        assert_eq!(units(&table), ["Text", "mm", "mm", "in", "", "deg", "mm^2"]);
        assert!(table.columns.iter().all(|c| !c.inferred));
        assert_eq!(table.rows[0][0], text("M5"));
        assert_eq!(table.rows[0][3], CsvCell::Number(0.2));
        assert_eq!(table.lines, [2]);
        assert!(table.refused.is_empty());
        let column = serde_json::to_value(&table.columns[2]).unwrap();
        assert_eq!(
            column,
            serde_json::json!({"name": "Head diameter", "renamed": "Head_diameter",
                               "unit": "mm", "inferred": false})
        );
        assert_eq!(
            serde_json::to_value(&table.rows[0][..3]).unwrap(),
            serde_json::json!(["M5", 5.0, 8.5])
        );
    }

    #[test]
    fn semicolons_and_decimal_commas() {
        let table = read(
            "Size;d [mm];Pitch [mm];k\r\n\
             M3; 3 ;0,5;\"3,0\"\r\n\
             M4;4;0,7;4\r\n\
             M5;5;\"0,8\";5\r\n",
        );
        assert_eq!(units(&table), ["Text", "mm", "mm", ""]);
        assert_eq!(table.rows[0][1], CsvCell::Number(3.0));
        assert_eq!(table.rows[0][2], CsvCell::Number(0.5));
        assert_eq!(table.rows[0][3], CsvCell::Number(3.0));
        assert_eq!(table.rows[1][2], CsvCell::Number(0.7));
        assert_eq!(table.rows[2][2], CsvCell::Number(0.8));
        assert!(table.refused.is_empty(), "{:?}", table.refused);
        // A quoted decimal comma in a comma-separated file too.
        let table = read("d [mm]\n\"5,5\"\n");
        assert_eq!(table.rows[0][0], CsvCell::Number(5.5));
    }

    #[test]
    fn units_in_cells_are_read_in_the_column_unit() {
        let table = read(
            "d [mm],a [deg],w [in],n []\n\
             0.25 in,0.5 rad,25.4 mm,2 * 3\n\
             5,30,1,\n\
             3 mm + 1 cm,90 deg / 2,\"1,5\",1e-3\n",
        );
        assert!((number(&table.rows[0][0]) - 6.35).abs() < 1e-12);
        assert!((number(&table.rows[0][1]) - 28.647_889_756_541_16).abs() < 1e-9);
        assert!((number(&table.rows[0][2]) - 1.0).abs() < 1e-12);
        assert_eq!(table.rows[0][3], CsvCell::Number(6.0));
        // A number without a unit is in the column's.
        assert_eq!(table.rows[1][0], CsvCell::Number(5.0));
        assert_eq!(table.rows[1][1], CsvCell::Number(30.0));
        assert_eq!(table.rows[1][2], CsvCell::Number(1.0));
        assert_eq!(table.rows[1][3], CsvCell::Empty);
        assert!((number(&table.rows[2][0]) - 13.0).abs() < 1e-12);
        assert!((number(&table.rows[2][1]) - 45.0).abs() < 1e-9);
        assert_eq!(table.rows[2][2], CsvCell::Number(1.5));
        assert_eq!(table.rows[2][3], CsvCell::Number(0.001));
        assert!(table.refused.is_empty(), "{:?}", table.refused);
    }

    #[test]
    fn columns_without_units_are_inferred() {
        let table = read(
            "Size,d,Len,Angle,Code,Blank\n\
             M3,3,10 mm,90 deg,10,\n\
             M4,4,12,45,A7,\n\
             ,,,,,\n\
             M5,5,0.5 in,,8,\n",
        );
        assert_eq!(units(&table), ["Text", "", "mm", "deg", "Text", ""]);
        assert!(table.columns.iter().all(|c| c.inferred));
        // The unit of the cells with one; a number without one is in it.
        assert_eq!(table.rows[0][2], CsvCell::Number(10.0));
        assert_eq!(table.rows[1][2], CsvCell::Number(12.0));
        assert!((number(&table.rows[3][2]) - 12.7).abs() < 1e-12);
        assert_eq!(table.rows[1][3], CsvCell::Number(45.0));
        // A text column takes numbers as texts.
        assert_eq!(table.rows[0][4], text("10"));
        // An empty row in the middle stays.
        assert_eq!(table.rows.len(), 4);
        assert!(table.rows[2].iter().all(|c| *c == CsvCell::Empty));
        assert!(table.refused.is_empty(), "{:?}", table.refused);
        // In inches by default, the design's default unit.
        let inches = EvalContext::new(
            crate::expr::LengthUnit::Inch,
            crate::expr::AngleUnit::Degree,
        );
        let table = parse("Len\n1 in\n2\n", inches).unwrap();
        assert_eq!(units(&table), ["in"]);
        assert_eq!(table.rows[1][0], CsvCell::Number(2.0));
    }

    #[test]
    fn wrong_cells_are_refused_with_the_reason() {
        let table = read(
            "d [mm],k [Text],n []\r\n\
             abc,x,1\r\n\
             5 deg,y,2 mm\r\n\
             'M5',z,3,extra\r\n\
             Width * 2,,4,\r\n",
        );
        let refused: Vec<(usize, &str, &str)> = table
            .refused
            .iter()
            .map(|r| (r.line, r.column.as_str(), r.text.as_str()))
            .collect();
        assert_eq!(
            refused,
            [
                (2, "d", "abc"),
                (3, "d", "5 deg"),
                (3, "n", "2 mm"),
                (4, "d", "'M5'"),
                (4, "", "extra"),
                (5, "d", "Width * 2"),
            ]
        );
        assert!(
            table.refused.iter().all(|r| !r.reason.is_empty()),
            "{:?}",
            table.refused
        );
        assert!(table.refused[4].reason.contains("more cells"));
        // A refused cell is left empty; the others are read.
        assert_eq!(
            table.rows[0],
            [CsvCell::Empty, text("x"), CsvCell::Number(1.0)]
        );
        assert_eq!(table.rows[3][1], CsvCell::Empty);
        assert_eq!(table.rows[3][2], CsvCell::Number(4.0));
    }

    #[test]
    fn files_that_are_no_table_are_refused() {
        let error = |text: &str| parse(text, EvalContext::default()).unwrap_err();
        assert_eq!(error(""), TableCsvError::Empty);
        assert_eq!(error(" \r\n"), TableCsvError::Empty);
        assert_eq!(error(",,\n"), TableCsvError::Empty);
        assert_eq!(
            error("a [mm]\n\"open\n"),
            TableCsvError::OpenQuote { line: 2 }
        );
        let unit = error("Size [Text],d [furlong]\n");
        assert!(
            matches!(&unit, TableCsvError::Unit { column, unit, .. }
                if column == "d" && unit == "furlong"),
            "{unit:?}"
        );
        assert!(
            unit.to_string()
                .starts_with("column d: the unit [furlong]: ")
        );
        let wide: Vec<String> = (0..=MAX_COLUMNS).map(|i| format!("c{i}")).collect();
        assert_eq!(
            error(&wide.join(",")),
            TableCsvError::TooManyColumns(MAX_COLUMNS + 1)
        );
        let long = format!("n []\r\n{}", "1\r\n".repeat(MAX_ROWS + 1));
        assert_eq!(error(&long), TableCsvError::TooManyRows(MAX_ROWS + 1));
    }

    #[test]
    fn export_and_import_give_the_same_table() {
        let columns: Vec<(String, Unit)> = [
            ("Size", "Text"),
            ("n", ""),
            ("d", "mm"),
            ("w", "in"),
            ("a", "deg"),
            ("r", "rad"),
            ("A", "mm^2"),
            ("Max", "cm"),
        ]
        .into_iter()
        .map(|(name, unit)| (name.to_owned(), Unit::parse(unit).unwrap()))
        .collect();
        let rows = vec![
            vec![
                text("M3"),
                CsvCell::Number(3.0),
                CsvCell::Number(5.5),
                CsvCell::Number(0.1),
                CsvCell::Number(30.0),
                CsvCell::Number(std::f64::consts::FRAC_PI_4),
                CsvCell::Number(1e-12),
                CsvCell::Number(-2.5),
            ],
            vec![
                text("with, comma and \"quotes\""),
                CsvCell::Empty,
                CsvCell::Number(1.0 / 3.0),
                CsvCell::Number(12_345_678.9),
                CsvCell::Number(-0.000_001),
                CsvCell::Empty,
                CsvCell::Number(2.5e17),
                CsvCell::Number(0.0),
            ],
            vec![CsvCell::Empty; 8],
            vec![
                text("8"),
                CsvCell::Number(1e300),
                CsvCell::Empty,
                CsvCell::Empty,
                CsvCell::Empty,
                CsvCell::Empty,
                CsvCell::Empty,
                CsvCell::Empty,
            ],
        ];
        let csv = write(&columns, &rows);
        assert!(
            csv.starts_with(
                "Size [Text],n [],d [mm],w [in],a [deg],r [rad],A [mm^2],Max [cm]\r\n\
                 M3,3,5.5,0.1,30,"
            ),
            "{csv}"
        );
        assert!(
            csv.contains("\r\n\"with, comma and \"\"quotes\"\"\",,"),
            "{csv}"
        );
        let table = read(&csv);
        let read_columns: Vec<(String, Unit)> = table
            .columns
            .iter()
            .map(|c| (c.table_name().to_owned(), c.unit))
            .collect();
        assert_eq!(read_columns, columns);
        assert_eq!(table.rows, rows);
        assert!(table.refused.is_empty());
        // A one-column table keeps its empty rows.
        let single = vec![("k".to_owned(), Unit::TEXT)];
        let rows = vec![vec![text("a")], vec![CsvCell::Empty], vec![text("b")]];
        let csv = write(&single, &rows);
        assert_eq!(csv, "k [Text]\r\na\r\n\"\"\r\nb\r\n");
        assert_eq!(read(&csv).rows, rows);
    }

    #[test]
    fn free_column_names() {
        let names = |names: &[&str]| {
            column_names(
                &names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| ((*n).to_owned(), i + 1))
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            names(&["a b", "a_b", "x", "x", "", "3"]),
            [
                Some("a_b_1".to_owned()),
                None,
                None,
                Some("x_1".to_owned()),
                Some("column_5".to_owned()),
                Some("c_3".to_owned()),
            ]
        );
        assert!(is_column_name("in") && is_column_name("_k") && !is_column_name("2a"));
    }
}
