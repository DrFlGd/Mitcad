// SPDX-License-Identifier: MIT
//! Failed features in command results (mitcad#401): every failed feature
//! is listed, and `edited` tells whether the command added or edited it,
//! so that a failure elsewhere in the timeline is not the command's.

use serde_json::{Value, json};

use crate::Document;
use crate::testing::MockKernel;

const REGION: &str = "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}";
const EDGE: &str = "E{F2:side(c1[c4,c2])|F2:side(c2[c1,c3])}";

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let result = doc
        .command(&command.to_string())
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&result).unwrap()
}

/// A 60 x 40 block (F1, F2) with a fillet (F3, radius d4).
fn filleted_block() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    command(&mut doc, json!({"cmd": "sketch.create"}));
    command(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
               "profiles": [{"sketch": "F1", "region": REGION}],
               "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}}),
    );
    let fillet = command(&mut doc, fillet(5));
    assert_eq!(fillet["uid"], "F3");
    assert!(fillet.get("failed_features").is_none(), "{fillet}");
    doc
}

fn fillet(radius: u32) -> Value {
    json!({"cmd": "add_feature", "def": {"type": "fillet", "body": "F2.b0",
           "edges": [EDGE], "radius": radius}})
}

fn failed(uid: &str, name: &str, edited: bool, new: bool) -> Value {
    json!({"uid": uid, "name": name, "error": "fillet failed", "edited": edited, "new": new})
}

#[test]
fn failures_elsewhere_are_listed_but_not_the_commands() {
    let mut doc = filleted_block();
    doc.kernel().fail.borrow_mut().insert("fillet");

    // A parameter change breaks the fillet: listed as new, not edited.
    let changed = command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "d4", "value": 6}),
    );
    assert_eq!(changed["error"], "Fillet1: fillet failed");
    assert_eq!(
        changed["failed_features"],
        json!([failed("F3", "Fillet1", false, true)])
    );

    // Unrelated commands while it fails: listed, neither edited nor new.
    for unrelated in [
        json!({"cmd": "add_parameter", "name": "wall", "value": 2}),
        json!({"cmd": "sketch.create"}),
        json!({"cmd": "rename_body", "uid": "F2.b0", "name": "Base"}),
    ] {
        let result = command(&mut doc, unrelated.clone());
        assert_eq!(
            result["failed_features"],
            json!([failed("F3", "Fillet1", false, false)]),
            "{unrelated}"
        );
    }
    // The new sketch (F4) is fine; a rename changes nothing that is built.
    let renamed = command(
        &mut doc,
        json!({"cmd": "rename_feature", "uid": "F3", "name": "Round"}),
    );
    assert_eq!(
        renamed["failed_features"],
        json!([failed("F3", "Round", false, false)])
    );

    // An edit of the failed feature, also one of its values only, is the
    // command's failure.
    let mut edit = fillet(7);
    edit["cmd"] = json!("edit_feature");
    edit["uid"] = json!("F3");
    let edited = command(&mut doc, edit);
    assert_eq!(
        edited["failed_features"],
        json!([failed("F3", "Round", true, false)])
    );

    // A new failing feature: the added one is edited and new, the old one
    // neither.
    let added = command(&mut doc, fillet(3));
    assert_eq!(added["uid"], "F5");
    assert_eq!(
        added["failed_features"],
        json!([
            failed("F3", "Round", false, false),
            failed("F5", "Fillet2", true, true)
        ])
    );

    // Moving a failed feature in the timeline is editing it.
    let moved = command(
        &mut doc,
        json!({"cmd": "reorder_feature", "uid": "F5", "index": 2}),
    );
    assert_eq!(
        moved["failed_features"],
        json!([
            failed("F5", "Fillet2", true, false),
            failed("F3", "Round", false, false)
        ])
    );

    // Suppressed, it fails no more; unsuppressed, it fails again by this
    // command.
    let suppressed = command(&mut doc, json!({"cmd": "suppress_feature", "uid": "F3"}));
    assert_eq!(
        suppressed["failed_features"],
        json!([failed("F5", "Fillet2", false, false)])
    );
    let unsuppressed = command(
        &mut doc,
        json!({"cmd": "suppress_feature", "uid": "F3", "suppressed": false}),
    );
    assert_eq!(
        unsuppressed["failed_features"],
        json!([
            failed("F5", "Fillet2", false, false),
            failed("F3", "Round", true, true)
        ])
    );

    // Deleting the failed features leaves none: no list at all.
    command(&mut doc, json!({"cmd": "delete_feature", "uid": "F5"}));
    let deleted = command(&mut doc, json!({"cmd": "delete_feature", "uid": "F3"}));
    assert_eq!(deleted["error"], Value::Null);
    assert!(deleted.get("failed_features").is_none(), "{deleted}");
}
