// SPDX-License-Identifier: MIT
//! Text parameters (mitcad#664) with the mock kernel: a sketch text that
//! follows a text parameter or an expression, laid out again when the
//! parameter's text changes and only then, the type errors both ways, no
//! limits, and the design file saved and read again.

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::ids::FeatureUid;
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

fn sketch(doc: &Document<MockKernel>) -> Value {
    serde_json::from_str(
        &doc.query(&json!({"query": "sketch", "uid": "F1"}).to_string())
            .unwrap(),
    )
    .unwrap()
}

/// The glyph contours of the sketch's first text as it last evaluated
/// (the mock kernel's glyph of `X` has one).
fn contours(doc: &Document<MockKernel>) -> usize {
    doc.sketch_output(FeatureUid(1))
        .expect("the sketch evaluated")
        .texts[0]
        .contours
        .len()
}

/// The id of the sketch's first text (`t5`).
fn text_id(doc: &Document<MockKernel>) -> String {
    sketch(doc)["texts"][0]["id"].as_str().unwrap().to_owned()
}

/// A sketch with a text that follows `Label`, `left('XXXXXXXX'; Count)`
/// with `Count` 2.
fn label_sketch() -> Document<MockKernel> {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    command(
        &mut d,
        json!({"cmd": "add_parameter", "name": "Count", "expression": "2"}),
    );
    // Without a unit, a text expression makes a text parameter.
    command(
        &mut d,
        json!({"cmd": "add_parameter", "name": "Label",
               "expression": "left('XXXXXXXX'; Count)"}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_text", "sketch": "F1", "value": "Label", "at": [0, 0],
               "height": 10}),
    );
    d
}

#[test]
fn a_sketch_text_follows_its_text_parameter() {
    let mut d = label_sketch();
    let params = d.parameters();
    let label = params.get(params.find("Label").unwrap()).unwrap();
    assert_eq!(label.unit().to_string(), "Text");
    assert_eq!(label.text(), Some("XX"));
    let listed: Value =
        serde_json::from_str(&d.query(r#"{"query": "parameters"}"#).unwrap()).unwrap();
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Label")
        .unwrap();
    assert_eq!(row["unit"], "Text");
    assert_eq!(row["text"], "XX");
    let s = sketch(&d);
    let t = &s["texts"][0];
    assert_eq!(t["text"], "XX", "{t}");
    assert_eq!(t["value"]["parameter"], "Label");
    assert_eq!(t["value"]["own"], false);
    assert_eq!(contours(&d), 2);

    // A change of what it uses lays the text out again.
    let calls = d.kernel().count("font_glyphs");
    let changed = command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "Count", "expression": "3"}),
    );
    assert_eq!(changed["changed"], json!(["Count", "Label"]));
    assert_eq!(sketch(&d)["texts"][0]["text"], "XXX");
    assert_eq!(contours(&d), 3);
    assert!(d.kernel().count("font_glyphs") > calls);
    // A change that leaves its text as it was does not.
    let calls = d.kernel().count("font_glyphs");
    let changed = command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "Count", "expression": "3.5"}),
    );
    assert_eq!(changed["changed"], json!(["Count"]));
    assert_eq!(d.kernel().count("font_glyphs"), calls);
    // Undo takes the text back.
    d.undo();
    d.undo();
    assert_eq!(contours(&d), 2);
    assert_eq!(sketch(&d)["texts"][0]["text"], "XX");
}

#[test]
fn a_number_value_lists_the_text_parameters_that_use_it() {
    // A text parameter's number value is always 0, so the changed names
    // come from the table's change set (mitcad#758).
    let mut d = label_sketch();
    let changed = command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "Count", "value": 3}),
    );
    assert_eq!(changed["changed"], json!(["Count", "Label"]));
    assert_eq!(sketch(&d)["texts"][0]["text"], "XXX");
    // A value that leaves the text as it was lists only the number.
    let changed = command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "Count", "value": 3.5}),
    );
    assert_eq!(changed["changed"], json!(["Count"]));
    // The same value is no change at all.
    let changed = command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "Count", "value": 3.5}),
    );
    assert_eq!(changed["changed"], json!([]));
}

#[test]
fn a_sketch_text_follows_an_expression_of_its_own() {
    let mut d = label_sketch();
    let id = text_id(&d);
    command(
        &mut d,
        json!({"cmd": "sketch.edit_text", "sketch": "F1", "id": id,
               "value": "'Box ' & Count & 'x' & Count"}),
    );
    let s = sketch(&d);
    let t = &s["texts"][0];
    assert_eq!(t["text"], "Box 2x2", "{t}");
    assert_eq!(t["value"]["own"], true);
    assert_eq!(t["value"]["expression"], "'Box ' & Count & 'x' & Count");
    // The sketch's own text parameter, a model parameter of the sketch.
    let name = t["value"]["parameter"].as_str().unwrap().to_owned();
    let params = d.parameters();
    let param = params.find(&name).unwrap();
    assert_eq!(params.owner(param), Some(FeatureUid(1)));
    assert!(params.get(param).unwrap().is_text());
    assert_eq!(params.text(param), Some("Box 2x2"));
    // A new expression changes that parameter.
    command(
        &mut d,
        json!({"cmd": "sketch.edit_text", "sketch": "F1", "id": id, "value": "'Lid'"}),
    );
    let params = d.parameters();
    assert_eq!(params.text(params.find(&name).unwrap()), Some("Lid"));
    assert_eq!(sketch(&d)["texts"][0]["text"], "Lid");
    // An empty value: the text keeps its string and follows nothing.
    command(
        &mut d,
        json!({"cmd": "sketch.edit_text", "sketch": "F1", "id": id, "value": ""}),
    );
    let s = sketch(&d);
    assert_eq!(s["texts"][0]["text"], "Lid");
    assert!(s["texts"][0].get("value").is_none(), "{s}");
}

#[test]
fn texts_and_numbers_do_not_mix() {
    let mut d = label_sketch();
    let id = text_id(&d);
    // A number parameter or a number expression for a text.
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.edit_text", "sketch": "F1", "id": id, "value": "Count"}),
    );
    assert!(error.contains("not a text"), "{error}");
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.edit_text", "sketch": "F1", "id": id, "value": "Count * 2"}),
    );
    assert!(error.contains("a number where text is expected"), "{error}");
    // A text for a number.
    let error = rejected(
        &mut d,
        json!({"cmd": "add_parameter", "name": "W", "expression": "Label * 2", "unit": "mm"}),
    );
    assert!(error.contains("text where a number is expected"), "{error}");
    let error = rejected(
        &mut d,
        json!({"cmd": "add_parameter", "name": "W", "expression": "len(Label) mm + Label"}),
    );
    assert!(error.contains("text where a number is expected"), "{error}");
    // A number's expression for the text parameter.
    let error = rejected(
        &mut d,
        json!({"cmd": "set_parameter", "name": "Label", "expression": "Count"}),
    );
    assert!(error.contains("a number where text is expected"), "{error}");
    // The text a sketch text follows keeps its unit.
    let error = rejected(
        &mut d,
        json!({"cmd": "set_parameter", "name": "Label", "unit": "mm"}),
    );
    assert!(error.contains("as a text"), "{error}");
    // A text parameter has no limits.
    let error = rejected(
        &mut d,
        json!({"cmd": "set_parameter", "name": "Label", "max": 5}),
    );
    assert!(error.contains("a text parameter has no minimum"), "{error}");
    let error = rejected(
        &mut d,
        json!({"cmd": "add_parameter", "name": "Tag", "expression": "'a'", "min": 1}),
    );
    assert!(error.contains("a text parameter has no minimum"), "{error}");
    // Nothing changed.
    assert_eq!(sketch(&d)["texts"][0]["text"], "XX");
}

#[test]
fn text_parameters_are_saved_and_read_again() {
    let mut d = label_sketch();
    command(
        &mut d,
        json!({"cmd": "add_parameter", "name": "Note", "expression": "\"It's \"\"quoted\"\"\""}),
    );
    let file = d.to_json();
    let saved: Value = serde_json::from_str(&file).unwrap();
    let label = saved["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Label")
        .unwrap();
    assert_eq!(label["unit"], "Text");
    assert_eq!(label["expression"], "left('XXXXXXXX'; Count)");
    let mut back = Document::from_json(&file, MockKernel::default()).unwrap();
    back.recompute();
    let params = back.parameters();
    assert_eq!(params.text(params.find("Label").unwrap()), Some("XX"));
    assert_eq!(
        params.text(params.find("Note").unwrap()),
        Some("It's \"quoted\"")
    );
    let s = sketch(&back);
    assert_eq!(s["texts"][0]["value"]["parameter"], "Label", "{s}");
    assert_eq!(s["texts"][0]["text"], "XX");
    assert_eq!(contours(&back), 2);
    let saved_again: Value = serde_json::from_str(&back.to_json()).unwrap();
    assert_eq!(saved_again["parameters"], saved["parameters"]);
    assert_eq!(saved_again["features"], saved["features"]);
}
