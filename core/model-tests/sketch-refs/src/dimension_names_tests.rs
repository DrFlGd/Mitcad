// SPDX-License-Identifier: MIT
//! Dimension names (mitcad#231): `name = expression` in a value slot
//! renames the slot's parameter and sets its expression in one step.

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_model_api::DocumentApi;

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let result = doc
        .command(&command.to_string())
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&result).unwrap()
}

fn rejected(doc: &mut Document<MockKernel>, command: Value) -> String {
    doc.command(&command.to_string())
        .map(|r| panic!("{command} must fail, got {r}"))
        .unwrap_err()
        .to_string()
}

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    serde_json::from_str(&doc.query(&query.to_string()).unwrap()).unwrap()
}

/// (name, expression) of every parameter.
fn parameters(doc: &Document<MockKernel>) -> Vec<(String, String)> {
    query(doc, json!({"query": "parameters"}))
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["name"].as_str().unwrap().to_owned(),
                p["expression"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn dimension(doc: &Document<MockKernel>, id: &str) -> Value {
    query(doc, json!({"query": "sketch", "uid": "F1"}))["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == id)
        .cloned()
        .unwrap()
}

/// Sketch1 (F1) with a 60 x 40 rectangle: width k5 (d1), height k6 (d2).
fn rectangle() -> Document<MockKernel> {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    command(
        &mut d,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}),
    );
    d
}

fn set_dimension(dimension: &str, value: &str) -> Value {
    json!({"cmd": "sketch.set_dimension", "sketch": "F1", "dimension": dimension, "value": value})
}

#[test]
fn name_equals_expression_renames_and_sets_in_one_step() {
    let mut d = rectangle();
    command(&mut d, set_dimension("k6", "d1 / 2"));
    command(&mut d, set_dimension("k5", "width = 50"));
    let k5 = dimension(&d, "k5");
    assert_eq!(k5["value"], "width");
    assert_eq!(k5["measured"], 50.0);
    assert_eq!(k5["comment"], "Sketch1 width");
    // The height's expression follows the rename.
    assert_eq!(
        parameters(&d),
        [
            ("width".to_owned(), "50".to_owned()),
            ("d2".to_owned(), "width / 2".to_owned())
        ]
    );
    assert_eq!(dimension(&d, "k6")["measured"], 25.0);
    // An expression, and the same name again.
    command(&mut d, set_dimension("k5", "width = 30 mm * 2"));
    assert_eq!(dimension(&d, "k5")["measured"], 60.0);
    // One undo step back to d1 = 50 mm.
    command(&mut d, json!({"cmd": "undo"}));
    command(&mut d, json!({"cmd": "undo"}));
    assert_eq!(parameters(&d)[0], ("d1".to_owned(), "60 mm".to_owned()));
    // A plain name still means "use that parameter".
    command(
        &mut d,
        json!({"cmd": "add_parameter", "name": "w", "value": "70 mm"}),
    );
    command(&mut d, set_dimension("k5", "w"));
    assert_eq!(dimension(&d, "k5")["value"], "w");
    // The parameter it uses, by its own name, takes the expression.
    command(&mut d, set_dimension("k5", "w = 80"));
    assert_eq!(dimension(&d, "k5")["measured"], 80.0);
    // Another name makes a new parameter of the dimension.
    command(&mut d, set_dimension("k5", "len = w / 2"));
    assert_eq!(dimension(&d, "k5")["value"], "len");
    assert_eq!(dimension(&d, "k5")["measured"], 40.0);
}

#[test]
fn taken_and_invalid_names_are_refused() {
    let mut d = rectangle();
    let before = parameters(&d);
    let error = rejected(&mut d, set_dimension("k5", "d2 = 10"));
    assert!(error.contains("d2"), "{error}");
    let error = rejected(&mut d, set_dimension("k5", "2x = 10"));
    assert!(error.contains("2x"), "{error}");
    let error = rejected(&mut d, set_dimension("k5", "w ="));
    assert!(error.contains("needs a value"), "{error}");
    assert_eq!(parameters(&d), before);
    // Comparisons are no names.
    let error = rejected(&mut d, set_dimension("k5", "d1 == 10"));
    assert!(!error.contains("needs a value"), "{error}");
}

#[test]
fn feature_slots_take_names_and_configurations_follow() {
    let mut d = rectangle();
    let region = query(&d, json!({"query": "sketch", "uid": "F1"}))["regions"][0]["key"].clone();
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F1", "region": region}],
            "extent": {"type": "distance", "distance": "depth = 10"}, "operation": "new_body"}}),
    );
    assert_eq!(parameters(&d)[2], ("depth".to_owned(), "10".to_owned()));
    command(
        &mut d,
        json!({"cmd": "set_configurations", "configurations": {"parameters": ["depth"],
            "rows": [{"name": "Thin", "values": {"depth": "5 mm"}}]}}),
    );
    // Editing the feature with a new name renames its own parameter, and
    // the table follows.
    let mut def = query(&d, json!({"query": "feature", "uid": "F2"}))["def"].clone();
    def["extent"]["distance"] = json!("thickness = 12");
    command(
        &mut d,
        json!({"cmd": "edit_feature", "uid": "F2", "def": def}),
    );
    assert_eq!(parameters(&d)[2], ("thickness".to_owned(), "12".to_owned()));
    let table = query(&d, json!({"query": "configurations"}));
    assert_eq!(table["configurations"]["parameters"], json!(["thickness"]));
}
