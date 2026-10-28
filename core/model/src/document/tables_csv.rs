// SPDX-License-Identifier: MIT
//! A table of the design exported to and imported from CSV (mitcad#804;
//! the format is `tables_csv.rs`, the design `docs/table-lookups.md`).
//!
//! An import creates the table or replaces it whole; a table it replaces
//! keeps its comment, which the file does not have. It goes through the
//! same atomic check as a table edit (`DocState::set_table`): the
//! parameters that name the table, and their dependents, are evaluated
//! again with the new one in one step.
//!
//! - The preview lists the parameters whose values change (`changed`, from
//!   and to), and those that would fail (`failing`, with their errors: a
//!   key no longer found, a column gone or of another kind).
//! - A parameter that would fail refuses the import, and so does a cell of
//!   the file that is refused (text in a number column, a wrong unit): the
//!   preview shows them all, and nothing changes until the file is right.
//! - The import is one undo step (`Import Table <name>`), none when the
//!   table is already as the file has it. Features recompute only when a
//!   parameter's value changed.

use serde::Serialize;

use super::{DocState, Document, ModelError, invalid};
use crate::expr::{ParamLimits, TableError, Unit, text_literal, value_to_expression};
use crate::kernel::Kernel;
use crate::parameters::{ChangeSet, ParamId, ParameterError};
use crate::tables::{Table, TableCell, TableColumn};
use crate::tables_csv::{self, CsvCell, CsvColumn, RefusedCell, TableCsv};

// The tables of mitcad#803 (`tables.rs`, `document/tables.rs`): the only
// place in this module that names their types, fields and methods
// (`stored_table`, `to_table`, `put_table`, `put_error` and the import's
// `set_table_as`).

/// A table as this module reads and writes it.
#[derive(Debug, Clone, PartialEq)]
struct StoredTable {
    name: String,
    comment: String,
    columns: Vec<(String, Unit)>,
    rows: Vec<Vec<CsvCell>>,
}

/// The table `name` of the state, None when it has none.
fn stored_table(state: &DocState, name: &str) -> Option<StoredTable> {
    let table = state.tables.get(name)?;
    let cell = |cell: &TableCell| match cell {
        TableCell::Number(n) => CsvCell::Number(*n),
        TableCell::Text(text) => CsvCell::Text(text.clone()),
        TableCell::Empty => CsvCell::Empty,
    };
    Some(StoredTable {
        name: table.name.clone(),
        comment: table.comment.clone(),
        columns: table
            .columns
            .iter()
            .map(|c| (c.name.clone(), c.unit))
            .collect(),
        rows: table
            .rows
            .iter()
            .map(|row| row.iter().map(cell).collect())
            .collect(),
    })
}

/// Why a table cannot be put in the state.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PutError {
    /// The table itself: its name, a column, a cell.
    Table(String),
    /// A parameter that names the table would fail with it.
    Parameter { name: String, error: String },
}

/// The table of mitcad#803 that `table` is.
fn to_table(table: &StoredTable) -> Table {
    let cell = |cell: &CsvCell| match cell {
        CsvCell::Number(n) => TableCell::Number(*n),
        CsvCell::Text(text) => TableCell::Text(text.clone()),
        CsvCell::Empty => TableCell::Empty,
    };
    Table {
        name: table.name.clone(),
        comment: table.comment.clone(),
        columns: table
            .columns
            .iter()
            .map(|(name, unit)| TableColumn::new(name, *unit))
            .collect(),
        rows: table
            .rows
            .iter()
            .map(|row| row.iter().map(cell).collect())
            .collect(),
    }
}

/// Puts `table` in the state as a table edit does (`DocState::set_table`:
/// in place of the one of its name or after the others, the parameters
/// that name it and their dependents evaluated again in one atomic step;
/// on an error nothing changes). A column a parameter uses that the table
/// lacks fails that parameter. Returns the parameters whose values
/// changed.
fn put_table(state: &mut DocState, table: &StoredTable) -> Result<ChangeSet, PutError> {
    let new = to_table(table);
    let name = new.name.clone();
    for id in state.parameters.table_users(&name) {
        let p = state.parameters.get(id).expect("users exist");
        for (t, column) in p.table_references() {
            if t == name && new.column(column).is_none() {
                return Err(PutError::Parameter {
                    name: p.name().to_owned(),
                    error: format!("the table has no column {name}.{column} any more"),
                });
            }
        }
    }
    state.set_table(new).map_err(put_error)
}

/// A refused table edit as the parameter that fails, or the table's own
/// problem.
fn put_error(error: ModelError) -> PutError {
    let failing = |name: String, error: String| PutError::Parameter { name, error };
    match error {
        ModelError::Parameter(ParameterError::Table(TableError::Eval { parameter, error })) => {
            failing(parameter, error.to_string())
        }
        ModelError::Parameter(ParameterError::Table(TableError::Parse { parameter, error })) => {
            failing(parameter, error.to_string())
        }
        ModelError::Parameter(ParameterError::Table(TableError::Limit { parameter, error })) => {
            failing(parameter, error.to_string())
        }
        other => PutError::Table(other.to_string()),
    }
}

// (End of the names of mitcad#803.)

/// A parameter whose value the import changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TableCsvChange {
    pub name: String,
    /// The value before and after, as the parameters query shows it
    /// (`8.5 mm`, a text as it is).
    pub from: String,
    pub to: String,
}

/// A parameter that would fail with the imported table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TableCsvFailure {
    pub name: String,
    pub error: String,
}

/// What importing a table CSV does (the preview) or did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TableCsvImport {
    /// The table's name.
    pub name: String,
    /// The import makes the table (else it replaces the one of its name).
    pub created: bool,
    /// The table is already as the file has it: the import changes nothing.
    pub unchanged: bool,
    /// The columns in the file's order.
    pub columns: Vec<CsvColumn>,
    /// The rows as the table keeps them: numbers in their column's unit,
    /// texts, null for an empty or refused cell.
    pub rows: Vec<Vec<CsvCell>>,
    pub refused: Vec<RefusedCell>,
    /// In creation order.
    pub changed: Vec<TableCsvChange>,
    pub failing: Vec<TableCsvFailure>,
}

/// `1 row`, `3 rows`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl TableCsvImport {
    /// `new table Screws: 5 columns, 3 rows, 1 renamed, 2 inferred, 1 cell
    /// refused, 2 parameters change, 1 parameter fails`, leaving out the
    /// nought counts after the rows; `Screws replaced: …` or `Screws
    /// unchanged: …` for a table that exists.
    pub fn summary(&self) -> String {
        let renamed = self.columns.iter().filter(|c| c.renamed.is_some()).count();
        let inferred = self.columns.iter().filter(|c| c.inferred).count();
        let mut parts = vec![
            count(self.columns.len(), "column", "columns"),
            count(self.rows.len(), "row", "rows"),
        ];
        let optional = [
            (renamed, "renamed", "renamed"),
            (inferred, "inferred", "inferred"),
            (self.refused.len(), "cell refused", "cells refused"),
            (self.changed.len(), "parameter changes", "parameters change"),
            (self.failing.len(), "parameter fails", "parameters fail"),
        ];
        for (n, one, many) in optional {
            if n > 0 {
                parts.push(count(n, one, many));
            }
        }
        let what = if self.created {
            format!("new table {}", self.name)
        } else if self.unchanged {
            format!("{} unchanged", self.name)
        } else {
            format!("{} replaced", self.name)
        };
        format!("{what}: {}", parts.join(", "))
    }

    /// Why the import is refused, None when it can be done: the first
    /// refused cell, or the parameters that would fail.
    pub fn refusal(&self) -> Option<String> {
        if let Some(first) = self.refused.first() {
            let column = if first.column.is_empty() {
                "beyond the last column".to_owned()
            } else {
                format!("column {}", first.column)
            };
            return Some(format!(
                "{} of the file refused, the first on line {}, {column}: {}; nothing is imported",
                count(self.refused.len(), "cell", "cells"),
                first.line,
                first.reason
            ));
        }
        let first = self.failing.first()?;
        let names: Vec<&str> = self.failing.iter().map(|f| f.name.as_str()).collect();
        Some(format!(
            "the table would make {} fail ({}: {}); nothing is imported",
            names.join(", "),
            first.name,
            first.error
        ))
    }
}

/// The parameters of `ids` with their values in `before` and `after`.
fn changes(before: &DocState, after: &DocState, ids: &ChangeSet) -> Vec<TableCsvChange> {
    let shown = |state: &DocState, id: ParamId| {
        state
            .parameters
            .get(id)
            .map(|p| p.formatted_value(crate::expr::DEFAULT_DECIMALS))
            .unwrap_or_default()
    };
    ids.iter()
        .map(|&id| TableCsvChange {
            name: after.parameters.name(id),
            from: shown(before, id),
            to: shown(after, id),
        })
        .collect()
}

/// Holds a parameter at its value, without limits, so that it names no
/// table any more; false when it cannot be.
fn hold(state: &mut DocState, name: &str) -> bool {
    let Some(id) = state.parameters.find(name) else {
        return false;
    };
    let Some(p) = state.parameters.get(id) else {
        return false;
    };
    let unit = p.unit();
    let expression = match p.text() {
        Some(text) => text_literal(text),
        None => value_to_expression(p.value(), unit),
    };
    state
        .parameters
        .redefine(id, &expression, unit, Some(ParamLimits::default()))
        .is_ok()
}

/// Every parameter that would fail with `table`, with its error: on a
/// copy, each failing one is held at its value and the table put again,
/// until it goes in.
fn failing_parameters(state: &DocState, table: &StoredTable) -> Vec<TableCsvFailure> {
    let mut trial = state.clone();
    let mut failing: Vec<TableCsvFailure> = Vec::new();
    while let Err(PutError::Parameter { name, error }) = put_table(&mut trial, table) {
        if failing.iter().any(|f| f.name == name) {
            break;
        }
        let held = hold(&mut trial, &name);
        failing.push(TableCsvFailure { name, error });
        if !held {
            break;
        }
    }
    failing
}

/// What importing `read` as the table `name` does to `state`, and the table
/// it puts in.
fn preview(
    state: &DocState,
    name: &str,
    read: TableCsv,
) -> Result<(TableCsvImport, StoredTable), ModelError> {
    let before = stored_table(state, name);
    let table = StoredTable {
        name: name.to_owned(),
        comment: before
            .as_ref()
            .map(|t| t.comment.clone())
            .unwrap_or_default(),
        columns: read
            .columns
            .iter()
            .map(|c| (c.table_name().to_owned(), c.unit))
            .collect(),
        rows: read.rows,
    };
    let mut trial = state.clone();
    let (changed, failing) = match put_table(&mut trial, &table) {
        Ok(ids) => (changes(state, &trial, &ids), Vec::new()),
        Err(PutError::Table(reason)) => return Err(invalid(reason)),
        Err(PutError::Parameter { .. }) => (Vec::new(), failing_parameters(state, &table)),
    };
    let outcome = TableCsvImport {
        name: name.to_owned(),
        created: before.is_none(),
        unchanged: before.as_ref() == Some(&table),
        columns: read.columns,
        rows: table.rows.clone(),
        refused: read.refused,
        changed,
        failing,
    };
    Ok((outcome, table))
}

impl<K: Kernel> Document<K> {
    /// The table `name` as CSV, and its number of rows.
    pub fn table_csv(&self, name: &str) -> Result<(String, usize), ModelError> {
        let table =
            stored_table(&self.state, name).ok_or_else(|| invalid(format!("no table {name}")))?;
        Ok((
            tables_csv::write(&table.columns, &table.rows),
            table.rows.len(),
        ))
    }

    /// What importing a CSV as the table `name` would do; nothing changes.
    pub fn preview_table_csv(&self, name: &str, text: &str) -> Result<TableCsvImport, ModelError> {
        let read = self.read_table_csv(text)?;
        Ok(preview(&self.state, name, read)?.0)
    }

    /// Imports a CSV as the table `name`, created or replaced whole, as one
    /// undo step (`Import Table <name>`; none when the table is already as
    /// the file has it). Refused while a cell of the file is refused or a
    /// parameter would fail (see the preview).
    pub fn import_table_csv(
        &mut self,
        name: &str,
        text: &str,
    ) -> Result<TableCsvImport, ModelError> {
        let read = self.read_table_csv(text)?;
        let (outcome, table) = preview(&self.state, name, read)?;
        if let Some(reason) = outcome.refusal() {
            return Err(invalid(reason));
        }
        if outcome.unchanged {
            return Ok(outcome);
        }
        // A table edit (mitcad#803): features recompute only when a
        // parameter's value changed.
        self.set_table_as(to_table(&table), &format!("Import Table {name}"))?;
        Ok(outcome)
    }

    /// A table CSV read in the design's default units.
    fn read_table_csv(&self, text: &str) -> Result<TableCsv, ModelError> {
        tables_csv::parse(text, self.state.parameters.context()).map_err(|e| invalid(e.to_string()))
    }
}
