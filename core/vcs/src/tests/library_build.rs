// SPDX-License-Identifier: MIT
//! BUILD parts of component libraries (mitcad#375): the manifest's
//! supplier fields and `build` block kept and shown, `library check`'s
//! rules for them (roles, systems, lengths, frames that are joint origins
//! of the design, the profile layout, connector, panel and linear blocks,
//! references that resolve), `library_search`'s BUILD filters and
//! `library_build_candidates` for junctions and pickers. The designs are
//! made with the mock kernel; nothing goes over the network.

use serde_json::{Value, json};

use super::remote::{slashes, system_git};
use super::*;
use crate::library::manifest::Manifest;
use crate::library::{LibraryCache, api};

fn library_command(cache: &LibraryCache, git: Option<&GitCli>, command: Value) -> Value {
    serde_json::from_str(&api::command(&command.to_string(), cache, git, None).unwrap()).unwrap()
}

use crate::remote::GitCli;

const SQUARE: &str = "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}";

fn design(commands: &[Value]) -> String {
    let mut doc = Document::new(MockKernel::default());
    for command in commands {
        doc.command(&command.to_string())
            .unwrap_or_else(|e| panic!("{command}: {e}"));
    }
    doc.to_json()
}

/// A joint origin `mount:<name>` on a fixed plane.
fn frame(name: &str, origin: [f64; 3], normal: [f64; 3], x_axis: [f64; 3]) -> Value {
    json!({"cmd": "add_feature", "name": format!("mount:{name}"), "def": {
        "type": "joint_origin",
        "geometry": {"origin": origin, "normal": normal, "x_axis": x_axis}}})
}

/// A 30 x 30 profile in the layout: the section on XY at the origin
/// extruded by L along +Z, end A on XY turned over, end B on a plane at
/// L, a slot frame on the +X side and the core.
fn profile(slot_normal: [f64; 3]) -> String {
    design(&[
        json!({"cmd": "add_parameter", "name": "L", "expression": "100 mm"}),
        json!({"cmd": "sketch.create"}),
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-15, -15], "width": 30, "height": 30}),
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F1", "region": SQUARE}],
            "extent": {"type": "distance", "distance": "L"}, "operation": "new_body"}}),
        json!({"cmd": "add_feature", "name": "End B plane", "def": {"type": "construction_plane",
            "definition": {"type": "offset", "plane": "xy", "distance": "L"}}}),
        json!({"cmd": "add_feature", "name": "mount:end_a", "def": {
            "type": "joint_origin", "geometry": "xy", "flip": true}}),
        json!({"cmd": "add_feature", "name": "mount:end_b", "def": {
            "type": "joint_origin", "geometry": "F3"}}),
        frame("slot_x+_1", [15.0, 0.0, 0.0], slot_normal, [0.0, 0.0, 1.0]),
        frame("core", [0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]),
    ])
}

/// A 20 mm block with fixed frames of these names.
fn block(frames: &[&str]) -> String {
    let mut commands = vec![
        json!({"cmd": "sketch.create"}),
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 20, "height": 20}),
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F1", "region": SQUARE}],
            "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}}),
    ];
    for (i, name) in frames.iter().enumerate() {
        commands.push(frame(
            name,
            [i as f64, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
        ));
    }
    design(&commands)
}

/// A T-nut: a block with a table of two sizes.
fn nut() -> String {
    design(&[
        json!({"cmd": "add_parameter", "name": "d", "expression": "6 mm"}),
        json!({"cmd": "sketch.create"}),
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 16, "height": "d"}),
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F1", "region": SQUARE}],
            "extent": {"type": "distance", "distance": 6}, "operation": "new_body"}}),
        frame("slot", [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        json!({"cmd": "set_configurations", "configurations": {
            "selectors": ["Size"], "parameters": ["d"], "default": "M6",
            "rows": [{"name": "M5", "select": {"Size": "M5"}, "values": {"d": "5 mm"}},
                     {"name": "M6", "select": {"Size": "M6"}, "values": {"d": "6 mm"}}]}}),
    ])
}

fn b8() -> Value {
    json!({"type": "B", "slot": 8, "grid": 30})
}

/// The manifest entries of a small library of BUILD parts: a profile, two
/// brackets (B-type slot 8 and I-type slot 5) and a T-nut.
fn components() -> Value {
    json!([
        {"id": "p30-b8", "path": "profiles/p30-b8.mitcad", "name": "Profile 30x30 B8",
         "category": "profiles", "supplier": "Profile Shop", "part_number": "P30_N8_B",
         "supplier_url": "https://example.org/p30", "designation": "Profile 30x30 B8 L={L}",
         "build": {"role": "profile", "system": b8(),
            "length": {"parameter": "L", "min": "10 mm", "max": "6 m", "stock": ["6000 mm"]},
            "section": {"width": "30 mm", "height": "30 mm"},
            "slot": {"depth": "9 mm", "opening": "8.2 mm", "inner_width": "16.5 mm"},
            "mass_per_metre": "0.9 kg", "future_field": {"kept": true},
            "frames": [{"name": "end_a", "kind": "end"}, {"name": "end_b", "kind": "end"},
                       {"name": "slot_x+_1", "kind": "slot", "accepts": ["connector", "panel"]},
                       {"name": "core", "kind": "bore", "diameter": "6.8 mm"}]}},
        {"id": "bracket-b8", "path": "connectors/bracket-b8.mitcad", "name": "Bracket 30 B8",
         "category": "connectors", "supplier": "Profile Shop", "part_number": "W30_B8",
         "build": {"role": "connector", "systems": [b8()],
            "frames": [{"name": "mount_a", "kind": "face"}, {"name": "mount_b", "kind": "face"}],
            "connector": {"junctions": ["L", "T"], "space": "inner_corner",
                "envelope": {"x": "28 mm", "y": "28 mm", "z": "20 mm"},
                "mates": [{"frame": "mount_a", "to": "slot"}, {"frame": "mount_b", "to": "slot"}],
                "fasteners": [{"frame": "mount_a",
                    "nut": {"library": "test-profiles", "component": "tnut-b8", "config": "M6"},
                    "screw": {"library": "mitcad-parts", "component": "iso4762", "config": "M6x12"}}]}}},
        {"id": "bracket-i5", "path": "connectors/bracket-i5.mitcad", "name": "Bracket 20 I5",
         "category": "connectors", "supplier": "Profile Shop", "part_number": "W20_I5",
         "build": {"role": "connector", "systems": [{"type": "I", "slot": 5, "grid": 20}],
            "frames": [{"name": "mount_a", "kind": "face"}, {"name": "mount_b", "kind": "face"}],
            "connector": {"junctions": ["L", "T"], "space": "inner_corner"}}},
        {"id": "tnut-b8", "path": "nuts/tnut-b8.mitcad", "name": "T-nut B8",
         "category": "nuts", "supplier": "Profile Shop", "part_number": "TN_B8",
         "build": {"role": "fastener", "systems": [{"type": "B", "slot": 8}],
            "frames": [{"name": "slot", "kind": "slot"}]}},
        {"id": "carriage-15", "path": "linear/carriage-15.mitcad", "name": "Carriage 15",
         "category": "linear", "supplier": "Profile Shop", "part_number": "C15",
         "build": {"role": "linear", "frames": [{"name": "rail", "kind": "rail"}],
            "linear": {"kind": "carriage", "size": "HGW15",
                       "mate": {"frame": "rail", "on": "rail", "joint": "slider", "axis": "x"}}}},
        {"id": "carriage-20", "path": "linear/carriage-20.mitcad", "name": "Carriage 20",
         "category": "linear", "supplier": "Profile Shop", "part_number": "C20",
         "build": {"role": "linear", "frames": [{"name": "rail", "kind": "rail"}],
            "linear": {"kind": "carriage", "size": "HGW20",
                       "mate": {"frame": "rail", "on": "rail", "joint": "slider", "axis": "x"}}}}
    ])
}

/// The library's folder: its manifest, designs and licence note.
fn write_library(dir: &Path, components: Value, designs: &[(&str, String)]) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("mitcad-library.json"),
        json!({"format": "mitcad-library", "version": 1, "id": "test-profiles",
               "name": "Test profiles", "license": "MIT", "library_version": "1.0.0",
               "categories": [{"id": "profiles", "name": "Profiles"},
                              {"id": "connectors", "name": "Connectors"},
                              {"id": "nuts", "name": "Nuts"},
                              {"id": "linear", "name": "Linear parts"}],
               "components": components})
        .to_string(),
    )
    .unwrap();
    fs::write(dir.join("LICENSE"), "MIT\n").unwrap();
    for (path, text) in designs {
        let file = dir.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
}

fn designs() -> Vec<(&'static str, String)> {
    vec![
        ("profiles/p30-b8.mitcad", profile([1.0, 0.0, 0.0])),
        (
            "connectors/bracket-b8.mitcad",
            block(&["mount_a", "mount_b"]),
        ),
        (
            "connectors/bracket-i5.mitcad",
            block(&["mount_a", "mount_b"]),
        ),
        ("nuts/tnut-b8.mitcad", nut()),
        ("linear/carriage-15.mitcad", block(&["rail"])),
        ("linear/carriage-20.mitcad", block(&["rail"])),
    ]
}

fn check(cache: &LibraryCache, dir: &Path) -> (Vec<String>, Vec<String>) {
    let answer = library_command(cache, None, json!({"cmd": "library_check", "dir": dir}));
    let list = |key: &str| -> Vec<String> {
        answer[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_str().unwrap().to_owned())
            .collect()
    };
    (list("errors"), list("warnings"))
}

#[test]
fn build_fields_are_kept_in_the_manifest() {
    let mut manifest: Manifest = serde_json::from_value(json!({
        "format": "mitcad-library", "version": 1, "id": "test-profiles", "name": "Test profiles",
        "license": "MIT", "components": components()}))
    .unwrap();
    let profile = manifest.component("p30-b8").unwrap().clone();
    assert_eq!(profile.supplier, "Profile Shop");
    assert_eq!(profile.part_number, "P30_N8_B");
    assert_eq!(profile.supplier_url, "https://example.org/p30");
    // Unknown fields of the block survive a round trip.
    let text = serde_json::to_string(&manifest).unwrap();
    let again: Manifest = serde_json::from_str(&text).unwrap();
    assert_eq!(again, manifest);
    assert_eq!(
        again.component("p30-b8").unwrap().build.as_ref().unwrap()["future_field"],
        json!({"kept": true})
    );
    // The manifest's own rules hold for the good parts.
    let problems = manifest.problems();
    assert_eq!(problems.errors, Vec::<String>::new());
    // A plain address is refused.
    manifest.components[0].supplier_url = "http://example.org/p30".to_owned();
    assert!(
        manifest.problems().errors[0].contains("no https:// address"),
        "{:?}",
        manifest.problems()
    );
}

#[test]
fn build_parts_are_checked_with_their_designs() {
    let scratch = Scratch::new("library-build-check");
    let cache = LibraryCache::new(scratch.join("cache"));
    let good = scratch.join("good");
    write_library(&good, components(), &designs());
    let (errors, warnings) = check(&cache, &good);
    assert_eq!(errors, Vec::<String>::new());
    // The screw is in a library not fetched here.
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("mitcad-parts is not fetched here")),
        "{warnings:?}"
    );

    // Broken parts: each rule once.
    let mut broken = components();
    let parts = broken.as_array_mut().unwrap();
    // The profile: no slot block, a slot frame named badly, a frame the
    // design lacks, a stock length outside its limits.
    parts[0]["build"]["frames"].as_array_mut().unwrap().extend([
        json!({"name": "side_1", "kind": "slot"}),
        json!({"name": "slot_y+_1", "kind": "slot"}),
    ]);
    parts[0]["build"]["length"]["stock"] = json!(["7 m"]);
    parts[0]["build"].as_object_mut().unwrap().remove("slot");
    // The bracket: an unknown junction, a mate on a frame it lacks, a nut
    // size the nut has not.
    parts[1]["build"]["connector"]["junctions"] = json!(["L", "Y"]);
    parts[1]["build"]["connector"]["mates"][1]["frame"] = json!("mount_c");
    parts[1]["build"]["connector"]["fasteners"][0]["nut"]["config"] = json!("M8");
    // The other bracket: no systems; a nut of a component that is not there.
    parts[2]["build"].as_object_mut().unwrap().remove("systems");
    parts[2]["build"]["connector"]["fasteners"] =
        json!([{"frame": "mount_a", "nut": {"library": "test-profiles", "component": "gone"}}]);
    // The nut: an unknown role.
    parts[3]["build"]["role"] = json!("washer");
    // A carriage: no count of teeth.
    parts[4]["build"]["linear"]["teeth"] = json!(0);
    let bad = scratch.join("bad");
    let mut files = designs();
    // The profile's slot frame points out of the wrong side.
    files[0].1 = profile([0.0, 1.0, 0.0]);
    // A size of the nut with a plain address for its own page.
    let mut nut: Value = serde_json::from_str(&files[3].1).unwrap();
    nut["configurations"]["rows"][0]["supplier_url"] = json!("http://example.org/m5");
    files[3].1 = nut.to_string();
    write_library(&bad, broken, &files);
    let (errors, warnings) = check(&cache, &bad);
    let has = |text: &str| errors.iter().any(|e| e.contains(text));
    for expected in [
        "p30-b8: slot frame side_1 is not named slot_<side>_<n>",
        "p30-b8: profiles/p30-b8.mitcad: the frame slot_y+_1 is no joint origin mount:slot_y+_1",
        "p30-b8: profiles/p30-b8.mitcad: the slot frame slot_x+_1 points out of the x+ side",
        "bracket-b8: build.connector.junctions: \"Y\" is none of",
        "bracket-b8: build.connector.mates[1] names the frame mount_c",
        "bracket-b8: build.connector.fasteners[0].nut: test-profiles/tnut-b8 has no configuration M8",
        "bracket-i5: a connector lists the slot systems it fits",
        "bracket-i5: build.connector.fasteners[0].nut: test-profiles has no component gone",
        "tnut-b8: build.role 'washer' is none of",
        "tnut-b8: nuts/tnut-b8.mitcad: row M5: the supplier_url 'http://example.org/m5' is no https:// address",
        "carriage-15: build.linear.teeth is no count of teeth",
    ] {
        assert!(has(expected), "{expected}\n{errors:#?}");
    }
    let warned = |text: &str| warnings.iter().any(|w| w.contains(text));
    assert!(warned("p30-b8: no build.slot"), "{warnings:#?}");
    assert!(
        warned("p30-b8: build.length.stock[0] (7000 mm) is outside"),
        "{warnings:#?}"
    );

    // A profile out of the layout: extruded by a number, end B fixed.
    let mut moved = components();
    moved.as_array_mut().unwrap().truncate(1);
    let off = scratch.join("off-layout");
    let text = design(&[
        json!({"cmd": "add_parameter", "name": "L", "expression": "100 mm"}),
        json!({"cmd": "sketch.create"}),
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-15, -15], "width": 30, "height": 30}),
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F1", "region": SQUARE}],
            "extent": {"type": "distance", "distance": 100}, "operation": "new_body"}}),
        json!({"cmd": "add_feature", "name": "mount:end_a", "def": {
            "type": "joint_origin", "geometry": "xy"}}),
        frame("end_b", [0.0, 0.0, 100.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        frame(
            "slot_x+_1",
            [15.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
        ),
        frame("core", [0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]),
    ]);
    write_library(&off, moved, &[("profiles/p30-b8.mitcad", text)]);
    let (errors, _) = check(&cache, &off);
    let has = |text: &str| errors.iter().any(|e| e.contains(text));
    assert!(has("no extrusion by L makes the profile"), "{errors:#?}");
    assert!(
        has("end_a is at the origin pointing along -Z"),
        "{errors:#?}"
    );
    assert!(has("end_b is fixed; it follows the length"), "{errors:#?}");
}

#[test]
fn build_candidates_fit_junctions_and_pickers() {
    let Some(cli) = system_git() else {
        return;
    };
    let scratch = Scratch::new("library-build-candidates");
    let cache = LibraryCache::new(scratch.join("cache"));
    let dir = scratch.join("profiles");
    let mut files = designs();
    // The nut's rows carry their own part numbers.
    let mut nut: Value = serde_json::from_str(&files[3].1).unwrap();
    nut["configurations"]["rows"][0]["part_number"] = json!("TN_B8_M5");
    nut["configurations"]["rows"][1]["part_number"] = json!("TN_B8_M6");
    files[3].1 = nut.to_string();
    write_library(&dir, components(), &files);
    git(&dir, &["init", "-q"]).unwrap();
    git(&dir, &["add", "-A"]).unwrap();
    git(&dir, &["commit", "-q", "-m", "Profiles 1.0"]).unwrap();
    git(&dir, &["tag", "v1.0.0"]).unwrap();
    let fetched = library_command(
        &cache,
        Some(&cli),
        json!({"cmd": "library_fetch", "url": slashes(&dir)}),
    );
    assert_eq!(fetched["error"], Value::Null, "{fetched}");

    // library_show carries the supplier, the block and the rows' numbers.
    let shown = library_command(
        &cache,
        None,
        json!({"cmd": "library_show", "id": "test-profiles"}),
    );
    let profile = &shown["components"][0];
    assert_eq!(profile["supplier"], "Profile Shop", "{shown}");
    assert_eq!(profile["part_number"], "P30_N8_B");
    assert_eq!(profile["supplier_url"], "https://example.org/p30");
    assert_eq!(profile["build"]["role"], "profile");
    let nut = &shown["components"][3];
    assert_eq!(
        nut["configurations"]["rows"][1],
        json!({"name": "M6", "select": {"Size": "M6"}, "part_number": "TN_B8_M6"}),
        "{nut}"
    );

    let ids = |answer: &Value, key: &str| -> Vec<String> {
        answer[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap().to_owned())
            .collect()
    };
    let run = |command: Value| library_command(&cache, None, command);

    // Search: by role and system, supplier and part numbers (a row's too).
    let found = run(json!({"cmd": "library_search", "role": "connector", "system": "B/8/30"}));
    assert_eq!(ids(&found, "components"), ["bracket-b8"], "{found}");
    let found = run(json!({"cmd": "library_search", "system": {"type": "I", "slot": 5}}));
    // Parts made for no system (the carriages) fit any.
    assert_eq!(
        ids(&found, "components"),
        ["bracket-i5", "carriage-15", "carriage-20"],
        "{found}"
    );
    let found = run(json!({"cmd": "library_search", "text": "p30_n8"}));
    assert_eq!(ids(&found, "components"), ["p30-b8"], "{found}");
    let found = run(json!({"cmd": "library_search", "part_number": "tn_b8_m5"}));
    assert_eq!(ids(&found, "components"), ["tnut-b8"], "{found}");
    let found = run(json!({"cmd": "library_search", "supplier": "elsewhere"}));
    assert_eq!(ids(&found, "components"), Vec::<String>::new());

    // A corner of two 30 B8 profiles: the B8 bracket fits, the I5 one is
    // greyed with its reason.
    let corner = json!({"id": "J1", "kind": "L",
        "a": {"component": "C1", "at": "end_a"}, "b": {"component": "C2", "at": "slot_x+_1"},
        "angle": 90, "systems": [b8(), b8()], "space": {"x": 40, "y": 40, "z": 30},
        "connectors": []});
    let answer = run(json!({"cmd": "library_build_candidates", "junction": corner}));
    assert_eq!(ids(&answer, "components"), ["bracket-b8"], "{answer}");
    assert_eq!(answer["components"][0]["supplier"], "Profile Shop");
    assert_eq!(answer["components"][0]["build"]["role"], "connector");
    assert_eq!(ids(&answer, "refused"), ["bracket-i5"], "{answer}");
    // A greyed row is the part and the reason, nothing more.
    assert_eq!(
        answer["refused"][0],
        json!({"id": "bracket-i5", "name": "Bracket 20 I5", "library": "test-profiles",
               "reason": "it is made for I-type slot 5 (20), not B-type slot 8 (30)"}),
        "{answer}"
    );
    assert!(
        answer["refused"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("made for I-type slot 5 (20), not B-type slot 8 (30)"),
        "{answer}"
    );
    // A cross: the bracket goes on L and T only.
    let mut cross = corner.clone();
    cross["kind"] = json!("cross");
    let answer = run(json!({"cmd": "library_build_candidates", "junction": cross}));
    assert_eq!(ids(&answer, "components"), Vec::<String>::new());
    assert!(
        answer["refused"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("goes on L, T junctions, not on cross"),
        "{answer}"
    );
    // A corner too tight for the bracket.
    let mut tight = corner.clone();
    tight["space"] = json!({"x": 20, "y": 40, "z": 30});
    let answer = run(json!({"cmd": "library_build_candidates", "junction": tight}));
    assert!(
        answer["refused"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("needs 28 x 28 x 20 mm in the inner corner"),
        "{answer}"
    );

    // Pickers: a role and a system.
    let answer =
        run(json!({"cmd": "library_build_candidates", "role": "profile", "system": "B/8"}));
    assert_eq!(ids(&answer, "components"), ["p30-b8"], "{answer}");
    let answer =
        run(json!({"cmd": "library_build_candidates", "role": "profile", "system": "I/5/20"}));
    assert_eq!(ids(&answer, "refused"), ["p30-b8"], "{answer}");
    let answer = run(json!({"cmd": "library_build_candidates", "role": "fastener"}));
    assert_eq!(ids(&answer, "components"), ["tnut-b8"], "{answer}");
    // A carriage for a rail of size HGW15: the other size is greyed.
    let answer = run(json!({"cmd": "library_build_candidates", "role": "linear",
                            "kind": "carriage", "size": "hgw15"}));
    assert_eq!(ids(&answer, "components"), ["carriage-15"], "{answer}");
    assert_eq!(ids(&answer, "refused"), ["carriage-20"], "{answer}");
    assert_eq!(answer["refused"][0]["reason"], "it is for HGW20, not hgw15");
    let answer = run(json!({"cmd": "library_build_candidates", "role": "linear", "kind": "rail"}));
    assert_eq!(ids(&answer, "components"), Vec::<String>::new());
    assert!(api::command(r#"{"cmd": "library_build_candidates"}"#, &cache, None, None).is_err());
}
