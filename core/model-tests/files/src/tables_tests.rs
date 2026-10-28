// SPDX-License-Identifier: MIT
//! Tables in the design (mitcad#667) in the design file and the design
//! comparison: saved and read back, and the diff's rows for tables added,
//! deleted, renamed and changed.

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_model_api::DocumentApi;
use mitcad_model_diff::*;

fn run(doc: &mut Document<MockKernel>, commands: Value) {
    doc.command(&commands.to_string())
        .unwrap_or_else(|e| panic!("{commands}: {e}"));
}

fn screws() -> Value {
    json!({
        "columns": [
            {"name": "Size", "unit": "Text"},
            {"name": "d", "unit": "mm"},
            {"name": "dk", "unit": "mm"}],
        "rows": [["M3", 3, 5.5], ["M4", 4, 7], ["M5", 5, 8.5]]
    })
}

/// A design with the table Screws and Size = 'M5', HeadDia its head
/// diameter.
fn design() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    run(
        &mut doc,
        json!([
            {"cmd": "set_table", "name": "Screws", "table": screws()},
            {"cmd": "add_parameter", "name": "Size", "expression": "'M5'"},
            {"cmd": "add_parameter", "name": "HeadDia",
             "expression": "XLOOKUP(Size; Screws.Size; Screws.dk)"},
        ]),
    );
    doc
}

fn changed(doc: &mut Document<MockKernel>, commands: Value) -> DesignDiff {
    let before = doc.to_json();
    run(doc, commands);
    diff_files(&before, &doc.to_json()).unwrap()
}

fn texts(diff: &DesignDiff) -> Vec<&str> {
    diff.tables.iter().map(|t| t.text.as_str()).collect()
}

#[test]
fn tables_are_saved_and_read_back() {
    let doc = design();
    let saved = doc.to_json();
    let file: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(file["tables"][0]["name"], "Screws");
    assert_eq!(
        file["tables"][0]["columns"][1],
        json!({"name": "d", "unit": "mm"})
    );
    assert!(file["tables"][0].get("comment").is_none());
    let loaded = Document::from_json(&saved, MockKernel::default()).unwrap();
    assert_eq!(loaded.tables(), doc.tables());
    let params = loaded.parameters();
    assert_eq!(params.value(params.find("HeadDia").unwrap()), Some(8.5));
    assert!(diff_files(&saved, &loaded.to_json()).unwrap().identical);
    // Unknown fields of a table are refused.
    let mut wrong = file.clone();
    wrong["tables"][0]["colour"] = json!("red");
    assert!(Document::from_json(&wrong.to_string(), MockKernel::default()).is_err());
    // A table that a parameter's lookup needs, gone: the file is refused.
    let mut wrong = file;
    wrong["tables"] = json!([]);
    assert!(Document::from_json(&wrong.to_string(), MockKernel::default()).is_err());
}

#[test]
fn the_diff_shows_tables_row_by_row() {
    let mut doc = design();
    let mut table = screws();
    table["rows"][2][2] = json!(8.7);
    table["rows"]
        .as_array_mut()
        .unwrap()
        .push(json!(["M6", 6, 10]));
    let diff = changed(
        &mut doc,
        json!({"cmd": "set_table", "name": "Screws", "table": table}),
    );
    assert_eq!(
        texts(&diff),
        ["table Screws row 3 (M5): dk 8.5 mm -> 8.7 mm; row 4 (M6) added"]
    );
    assert!(
        diff.to_text()
            .contains("Tables:\n  table Screws row 3 (M5)")
    );
    // A column added and one of another unit.
    let mut table = screws();
    table["columns"][1]["unit"] = json!("in");
    table["columns"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "k", "unit": "mm"}));
    for row in table["rows"].as_array_mut().unwrap() {
        row.as_array_mut().unwrap().push(json!(null));
    }
    let diff = changed(
        &mut doc,
        json!({"cmd": "set_table", "name": "Screws", "table": table}),
    );
    let text = texts(&diff).join("\n");
    assert!(text.contains("column d mm -> in"), "{text}");
    assert!(text.contains("column k added"), "{text}");
    // Renamed, added and deleted tables.
    let diff = changed(
        &mut doc,
        json!({"cmd": "rename_table", "name": "Screws", "new_name": "Bolts"}),
    );
    assert_eq!(texts(&diff), ["table Screws renamed to Bolts"]);
    let diff = changed(
        &mut doc,
        json!({"cmd": "set_table", "name": "Nuts",
               "table": {"columns": [{"name": "m", "unit": "mm"}], "rows": [[4]]}}),
    );
    assert_eq!(texts(&diff), ["table Nuts added"]);
    assert!(diff.summary.contains("+1 table"), "{}", diff.summary);
    let diff = changed(&mut doc, json!({"cmd": "delete_table", "name": "Nuts"}));
    assert_eq!(texts(&diff), ["table Nuts deleted"]);
}
