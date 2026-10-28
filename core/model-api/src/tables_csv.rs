// SPDX-License-Identifier: MIT
//! A table as CSV (mitcad#804, `document/tables_csv.rs`): the `table_csv`
//! query (the table's CSV), the `table_csv_preview` query (what an import
//! would do) and the `import_table_csv` command. The CSV is the text
//! itself: the application reads and writes the file. See `commands.md`,
//! "Tables as CSV".

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use super::dispatch::named_enum;
use mitcad_model::kernel::Kernel;
use mitcad_model::{Document, TableCsvImport};

named_enum! {
    #[derive(Debug, Deserialize)]
    #[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
    pub(super) enum TableCsvCommand {
        "import_table_csv" => ImportTableCsv {
            /// The table's name: the table is created, or replaced whole.
            name: String,
            /// The CSV text: a header of names with units in brackets.
            csv: String,
        },
    }
}

/// The table a `table_csv` query exports.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TableCsvQuery {
    name: String,
}

/// The table and the CSV text a preview reads.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TableCsvPreviewQuery {
    name: String,
    csv: String,
}

/// An import's outcome as JSON, with its `summary`.
fn outcome_json(outcome: &TableCsvImport) -> Value {
    let mut value = serde_json::to_value(outcome).unwrap_or_else(|_| json!({}));
    value["summary"] = json!(outcome.summary());
    value
}

/// The document's methods of this module used across the api, as an
/// extension trait (mitcad#777).
pub(super) trait TablesCsvOps<K: Kernel> {
    fn run_table_csv_command(&mut self, command: TableCsvCommand) -> Result<Value, ApiError>;

    /// The `table_csv` query: the table's CSV and its number of rows.
    fn table_csv_json(&self, query: &TableCsvQuery) -> Result<Value, ApiError>;

    /// The `table_csv_preview` query.
    fn table_csv_preview_json(&self, query: &TableCsvPreviewQuery) -> Result<Value, ApiError>;
}

impl<K: Kernel> TablesCsvOps<K> for Document<K> {
    fn run_table_csv_command(&mut self, command: TableCsvCommand) -> Result<Value, ApiError> {
        Ok(match command {
            TableCsvCommand::ImportTableCsv { name, csv } => {
                let outcome = self.import_table_csv(&name, &csv)?;
                let mut value = outcome_json(&outcome);
                // The configuration options the import moved, as
                // `set_table`'s answer lists them (mitcad#806, #849). An
                // import that changes nothing is no command: the moves of
                // an earlier one are not its own.
                value["moves"] = if outcome.unchanged {
                    json!([])
                } else {
                    super::configurations::moves_json(self.configuration_moves())
                };
                value
            }
        })
    }

    fn table_csv_json(&self, query: &TableCsvQuery) -> Result<Value, ApiError> {
        let (csv, rows) = self.table_csv(&query.name)?;
        Ok(json!({"csv": csv, "rows": rows}))
    }

    fn table_csv_preview_json(&self, query: &TableCsvPreviewQuery) -> Result<Value, ApiError> {
        Ok(outcome_json(
            &self.preview_table_csv(&query.name, &query.csv)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::DocumentApi;
    use mitcad_model::Document;
    use mitcad_model::testing::MockKernel;

    fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
        let result = doc
            .command(&command.to_string())
            .unwrap_or_else(|e| panic!("{command}: {e}"));
        serde_json::from_str(&result).unwrap()
    }

    fn query(doc: &Document<MockKernel>, query: Value) -> Value {
        serde_json::from_str(&doc.query(&query.to_string()).unwrap()).unwrap()
    }

    #[test]
    fn export_preview_and_import_through_the_api() {
        let mut doc = Document::new(MockKernel::default());
        let csv = "Size;Head diameter [mm];k\nM3;5,5;3\nM4;0.25 in;oops\n";
        let undo_before = query(&doc, json!({"query": "document"}))["undo_depth"].clone();
        let preview = query(
            &doc,
            json!({"query": "table_csv_preview", "name": "Screws", "csv": csv}),
        );
        assert_eq!(preview["name"], "Screws");
        assert_eq!(preview["created"], true);
        assert_eq!(
            preview["columns"][1],
            json!({"name": "Head diameter", "renamed": "Head_diameter",
                   "unit": "mm", "inferred": false})
        );
        assert_eq!(preview["columns"][0]["unit"], "Text");
        assert_eq!(preview["columns"][2]["unit"], "Text");
        assert_eq!(preview["rows"][0], json!(["M3", 5.5, "3"]));
        assert_eq!(preview["refused"], json!([]));
        assert_eq!(preview["changed"], json!([]));
        assert_eq!(preview["failing"], json!([]));
        assert_eq!(
            preview["summary"],
            "new table Screws: 3 columns, 2 rows, 1 renamed, 2 inferred"
        );
        // The preview changed nothing.
        assert_eq!(
            query(&doc, json!({"query": "document"}))["undo_depth"],
            undo_before
        );

        let done = command(
            &mut doc,
            json!({"cmd": "import_table_csv", "name": "Screws", "csv": csv}),
        );
        assert_eq!(done["summary"], preview["summary"]);
        assert_eq!(
            query(&doc, json!({"query": "document"}))["undo"],
            "Import Table Screws"
        );
        let exported = query(&doc, json!({"query": "table_csv", "name": "Screws"}));
        assert_eq!(exported["rows"], 2);
        assert_eq!(
            exported["csv"],
            "Size [Text],Head_diameter [mm],k [Text]\r\nM3,5.5,3\r\nM4,6.35,oops\r\n"
        );

        // Refused cells: listed by the preview, refusing the import.
        let wrong = "d [mm]\nfive\n";
        let preview = query(
            &doc,
            json!({"query": "table_csv_preview", "name": "Screws", "csv": wrong}),
        );
        assert_eq!(preview["refused"][0]["line"], 2);
        assert_eq!(preview["refused"][0]["column"], "d");
        assert_eq!(preview["refused"][0]["text"], "five");
        assert_eq!(preview["rows"], json!([[null]]));
        let error = doc
            .command(
                &json!({"cmd": "import_table_csv", "name": "Screws", "csv": wrong}).to_string(),
            )
            .unwrap_err();
        assert!(error.0.contains("nothing is imported"), "{error}");
        // No such table; no table at all.
        let error = doc
            .query(&json!({"query": "table_csv", "name": "Nuts"}).to_string())
            .unwrap_err();
        assert_eq!(error.0, "no table Nuts");
        let error = doc
            .query(&json!({"query": "table_csv_preview", "name": "Nuts", "csv": ""}).to_string())
            .unwrap_err();
        assert_eq!(error.0, "the CSV file is empty");
    }
}
