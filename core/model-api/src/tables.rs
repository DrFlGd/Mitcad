// SPDX-License-Identifier: MIT
//! Tables of data that parameter expressions look values up in
//! (mitcad#667, `mitcad_model::tables`, `docs/table-lookups.md`): the
//! commands `set_table`, `rename_table`, `rename_table_column` and
//! `delete_table`, and the `tables` query. See `commands.md`, "Tables".

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use super::dispatch::named_enum;
use mitcad_model::kernel::Kernel;
use mitcad_model::parameters::Parameter;
use mitcad_model::tables::{Table, TableCell, TableColumn};
use mitcad_model::{Document, ModelError};

/// A table's content in `set_table`; its name is the command's.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TableInput {
    #[serde(default)]
    comment: String,
    #[serde(default)]
    columns: Vec<TableColumn>,
    /// Arrays of cells: numbers in the column's unit, texts, null.
    #[serde(default)]
    rows: Vec<Vec<TableCell>>,
}

named_enum! {
    #[derive(Debug, Deserialize)]
    #[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
    pub(super) enum TableCommand {
        /// Creates or replaces a table.
        "set_table" => SetTable {
            name: String,
            table: TableInput,
        },
        "rename_table" => RenameTable {
            name: String,
            new_name: String,
        },
        "rename_table_column" => RenameTableColumn {
            table: String,
            name: String,
            new_name: String,
        },
        "delete_table" => DeleteTable {
            name: String,
        },
    }
}

/// The `tables` query: every table, or only the one named.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TablesQuery {
    #[serde(default)]
    name: Option<String>,
}

/// A parameter's table columns as `Table.Column` texts (the `parameters`
/// query's `table_dependencies`).
pub(super) fn table_dependencies(parameter: &Parameter) -> Vec<String> {
    parameter
        .table_references()
        .into_iter()
        .map(|(table, column)| format!("{table}.{column}"))
        .collect()
}

/// What is worth saying about a table: first cells that more than one row
/// has, as lookups by them find the first of those rows.
fn problems(table: &Table) -> Vec<String> {
    let Some(first) = table.columns.first() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, row) in table.rows.iter().enumerate() {
        let cell = &row[0];
        if cell.is_empty() || table.rows[..i].iter().any(|r| &r[0] == cell) {
            continue;
        }
        let rows: Vec<String> = table
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| &r[0] == cell)
            .map(|(j, _)| (j + 1).to_string())
            .collect();
        if rows.len() > 1 {
            out.push(format!(
                "rows {} have the same {} {}: lookups find the first",
                rows.join(", "),
                first.name,
                cell.display(first.unit)
            ));
        }
    }
    out
}

/// The document's methods of this module used across the api, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(super) trait TablesOps<K: Kernel> {
    fn run_table_command(&mut self, command: TableCommand) -> Result<Value, ApiError>;

    /// The `tables` query: each table with its content, the parameters
    /// that use it and its problems.
    fn tables_json(&self, query: &TablesQuery) -> Result<Value, ApiError>;

    /// The tables and their columns with units, for completion (the
    /// `formula_help` query's `tables`).
    fn table_names_json(&self) -> Value;
}

impl<K: Kernel> TablesOps<K> for Document<K> {
    fn run_table_command(&mut self, command: TableCommand) -> Result<Value, ApiError> {
        Ok(match command {
            TableCommand::SetTable { name, table } => {
                let table = Table {
                    name,
                    comment: table.comment,
                    columns: table.columns,
                    rows: table.rows,
                };
                let changed = self.set_table(table)?;
                // Options moved off removed values and by the cascade
                // (mitcad#806).
                json!({
                    "changed": changed,
                    "moves": super::configurations::moves_json(self.configuration_moves()),
                })
            }
            TableCommand::RenameTable { name, new_name } => {
                self.rename_table(&name, &new_name)?;
                json!({})
            }
            TableCommand::RenameTableColumn {
                table,
                name,
                new_name,
            } => {
                self.rename_table_column(&table, &name, &new_name)?;
                json!({})
            }
            TableCommand::DeleteTable { name } => {
                self.delete_table(&name)?;
                json!({})
            }
        })
    }

    fn tables_json(&self, query: &TablesQuery) -> Result<Value, ApiError> {
        if let Some(name) = &query.name
            && self.table(name).is_none()
        {
            return Err(ModelError::Invalid(format!("the design has no table {name}")).into());
        }
        let tables: Vec<Value> = self
            .tables()
            .iter()
            .filter(|t| query.name.as_ref().is_none_or(|name| *name == t.name))
            .map(|t| {
                json!({
                    "name": t.name,
                    "comment": t.comment,
                    "columns": t.columns,
                    "rows": t.rows,
                    "used_by": self.table_users(&t.name),
                    "problems": problems(t),
                })
            })
            .collect();
        Ok(json!({"tables": tables}))
    }

    fn table_names_json(&self) -> Value {
        Value::Array(
            self.tables()
                .iter()
                .map(|t| json!({"name": t.name, "columns": t.columns}))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mitcad_model::expr::Unit;

    #[test]
    fn repeated_first_cells_are_problems() {
        let mut table = Table::new(
            "Screws",
            vec![
                TableColumn::new("Size", Unit::TEXT),
                TableColumn::new("d", Unit::MM),
            ],
        );
        for (size, d) in [("M3", 3.0), ("M5", 5.0), ("M3", 3.5)] {
            table
                .rows
                .push(vec![TableCell::Text(size.to_owned()), TableCell::Number(d)]);
        }
        assert_eq!(
            problems(&table),
            ["rows 1, 3 have the same Size M3: lookups find the first"]
        );
        table.rows.pop();
        assert!(problems(&table).is_empty());
    }
}
