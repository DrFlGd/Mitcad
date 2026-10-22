// SPDX-License-Identifier: MIT
//! Library groups as a tree (mitcad#527): a category's `parent` and path
//! names make its group's names, a component's `category` may be a list
//! (`categories`), `library check` warns of an unknown parent and of a
//! cycle, and `library_search` gives every component its groups across two
//! libraries that share a group by its name. Nothing goes over the
//! network; the search's part is skipped without git.

use serde_json::{Value, json};

use super::remote::{slashes, system_git};
use super::*;
use crate::library::manifest::{CategoryIds, Manifest};
use crate::library::{LibraryCache, api};
use crate::remote::GitCli;

fn library_command(cache: &LibraryCache, git: Option<&GitCli>, command: Value) -> Value {
    serde_json::from_str(&api::command(&command.to_string(), cache, git, None).unwrap()).unwrap()
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// A library's categories: a parent by id, path names, an unknown parent,
/// two categories each other's parents and one its own.
fn tree_manifest() -> Value {
    json!({"format": "mitcad-library", "version": 1, "id": "test-wood", "name": "Test wood",
    "license": "MIT",
    "categories": [
        {"id": "wood", "name": "Wood"},
        {"id": "timber", "name": "Dimensioned timber", "parent": "wood"},
        {"id": "boards", "name": "Wood/ Boards /"},
        {"id": "slot-screws", "name": "Slot profile/Screws"},
        {"id": "general-screws", "name": "General fasteners/Screws"},
        {"id": "lost", "name": "Lost", "parent": "nowhere"},
        {"id": "a", "name": "A", "parent": "b"},
        {"id": "b", "name": "B", "parent": "a"},
        {"id": "itself", "name": "Itself", "parent": "itself"},
        {"id": "plain"}
    ],
    "components": [
        {"id": "beam", "path": "timber/beam.mitcad", "name": "Beam", "category": "timber"},
        {"id": "screw", "path": "slot-screws/screw.mitcad", "name": "Slot screw",
         "category": ["slot-screws", "general-screws"]},
        {"id": "bolt", "path": "slot-screws/bolt.mitcad", "name": "Bolt",
         "category": "slot-screws", "categories": ["general-screws", "slot-screws"]},
        {"id": "odd", "path": "parts/odd.mitcad", "name": "Odd", "category": "unlisted"},
        {"id": "none", "path": "parts/none.mitcad", "name": "None"}
    ]})
}

#[test]
fn categories_make_a_tree_of_groups() {
    let manifest: Manifest = serde_json::from_value(tree_manifest()).unwrap();
    // A parent by its id, then the category's own name.
    let timber = manifest.group_path("timber");
    assert_eq!(timber.names, names(&["Wood", "Dimensioned timber"]));
    assert_eq!(timber.order, vec![0, 1]);
    assert_eq!(timber.category, "timber");
    // A path name, trimmed, its empty parts left out; each name in the
    // place of the category that names it.
    let boards = manifest.group_path("boards");
    assert_eq!(boards.names, names(&["Wood", "Boards"]));
    assert_eq!(boards.order, vec![2, 2]);
    assert_eq!(manifest.group_path("wood").names, names(&["Wood"]));
    // Without a name, the id; a category the list lacks is its id at the
    // top, after every listed one.
    assert_eq!(manifest.group_path("plain").names, names(&["plain"]));
    let unlisted = manifest.group_path("unlisted");
    assert_eq!(unlisted.names, names(&["unlisted"]));
    assert_eq!(unlisted.order, vec![manifest.categories.len()]);
    // An unknown parent: at the top. A cycle is cut where it closes.
    assert_eq!(manifest.group_path("lost").names, names(&["Lost"]));
    assert_eq!(manifest.group_path("a").names, names(&["B", "A"]));
    assert_eq!(manifest.group_path("b").names, names(&["A", "B"]));
    assert_eq!(manifest.group_path("itself").names, names(&["Itself"]));

    // A component in several groups: its home first, each once.
    let screw = manifest.component("screw").unwrap();
    assert_eq!(screw.category_ids(), vec!["slot-screws", "general-screws"]);
    assert_eq!(screw.home_category(), "slot-screws");
    assert!(screw.in_category("general-screws"));
    let bolt = manifest.component("bolt").unwrap();
    assert_eq!(bolt.category_ids(), vec!["slot-screws", "general-screws"]);
    let groups = manifest.component_groups(bolt);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].names, names(&["Slot profile", "Screws"]));
    assert_eq!(groups[1].names, names(&["General fasteners", "Screws"]));
    let none = manifest.component("none").unwrap();
    assert_eq!(none.home_category(), "");
    assert!(manifest.component_groups(none).is_empty());

    // Kept as written: one category as text, a list as a list.
    let text = serde_json::to_value(&manifest).unwrap();
    assert_eq!(text["components"][0]["category"], "timber");
    assert_eq!(
        text["components"][1]["category"],
        json!(["slot-screws", "general-screws"])
    );
    assert_eq!(
        text["components"][2]["categories"],
        json!(["general-screws", "slot-screws"])
    );
    assert_eq!(text["components"][4].get("category"), None);
    assert_eq!(text["categories"][1]["parent"], "wood");
    assert_eq!(text["categories"][0].get("parent"), None);
    let again: Manifest = serde_json::from_value(text).unwrap();
    assert_eq!(again, manifest);
    assert_eq!(CategoryIds::one(""), CategoryIds::default());

    // The check: the tree's problems are warnings (the library stays
    // usable), each cycle told once.
    let problems = manifest.problems();
    assert_eq!(problems.errors, Vec::<String>::new());
    let warnings = problems.warnings.join("\n");
    assert!(
        warnings.contains(
            "the category lost: its parent nowhere is not one of the library's categories"
        ),
        "{warnings}"
    );
    assert!(
        warnings.contains("the categories a -> b -> a are each other's parents"),
        "{warnings}"
    );
    assert!(!warnings.contains("b -> a -> b"), "{warnings}");
    assert!(
        warnings.contains("the categories itself -> itself are each other's parents"),
        "{warnings}"
    );
    assert!(
        warnings.contains("odd: the category unlisted is not in the library's categories"),
        "{warnings}"
    );
    assert!(!warnings.contains("beam:"), "{warnings}");
    assert!(!warnings.contains("screw:"), "{warnings}");
}

#[test]
fn library_add_takes_categories_with_parents_and_paths() {
    let scratch = Scratch::new("library-groups-add");
    let cache = LibraryCache::new(scratch.join("cache"));
    let dir = scratch.join("mine");
    let made = library_command(
        &cache,
        None,
        json!({"cmd": "library_init", "dir": dir, "id": "test-mine", "name": "Mine",
               "license": "MIT"}),
    );
    assert_eq!(made["id"], "test-mine", "{made}");
    let design = Document::new(MockKernel::default()).to_json();
    let added = library_command(
        &cache,
        None,
        json!({"cmd": "library_add", "dir": dir, "text": design, "id": "rack",
               "name": "Rack", "category": ["racks", "storage"],
               "category_name": "Workshop/Racks"}),
    );
    assert_eq!(added["errors"], json!([]), "{added}");
    assert_eq!(added["component"]["path"], "racks/rack.mitcad", "{added}");
    let added = library_command(
        &cache,
        None,
        json!({"cmd": "library_add", "dir": dir, "text": design, "id": "shelf",
               "name": "Shelf", "category": "shelves", "category_name": "Shelves",
               "category_parent": "racks"}),
    );
    assert_eq!(added["errors"], json!([]), "{added}");
    let manifest: Manifest =
        serde_json::from_str(&fs::read_to_string(dir.join("mitcad-library.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest.component("rack").unwrap().category_ids(),
        vec!["racks", "storage"]
    );
    assert_eq!(
        manifest.group_path("racks").names,
        names(&["Workshop", "Racks"])
    );
    assert_eq!(manifest.group_path("storage").names, names(&["storage"]));
    assert_eq!(
        manifest.group_path("shelves").names,
        names(&["Workshop", "Racks", "Shelves"])
    );
}

/// A library folder of empty designs with `manifest`, recorded with git.
fn write_library(dir: &Path, manifest: &Value) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("mitcad-library.json"), manifest.to_string()).unwrap();
    fs::write(dir.join("LICENSE"), "MIT\n").unwrap();
    let design = Document::new(MockKernel::default()).to_json();
    for component in manifest["components"].as_array().unwrap() {
        let file = dir.join(component["path"].as_str().unwrap());
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, &design).unwrap();
    }
    git(dir, &["init", "-q"]).unwrap();
    git(dir, &["add", "-A"]).unwrap();
    git(dir, &["commit", "-q", "-m", "Parts"]).unwrap();
    git(dir, &["tag", "v1.0.0"]).unwrap();
}

#[test]
fn search_gives_every_part_its_groups_across_libraries() {
    let Some(cli) = system_git() else {
        return;
    };
    let scratch = Scratch::new("library-groups-search");
    let cache = LibraryCache::new(scratch.join("cache"));
    let wood = scratch.join("wood");
    write_library(&wood, &tree_manifest());
    // Another library's Wood: the same group by its name, through a
    // parent of its own.
    let sheets = scratch.join("sheets");
    write_library(
        &sheets,
        &json!({"format": "mitcad-library", "version": 1, "id": "test-sheets",
            "name": "Test sheets", "license": "MIT",
            "categories": [{"id": "timber-things", "name": "Wood"},
                           {"id": "sheets", "name": "Sheets", "parent": "timber-things"}],
            "components": [{"id": "plywood", "path": "sheets/plywood.mitcad", "name": "Plywood",
                            "category": "sheets"}]}),
    );
    for dir in [&wood, &sheets] {
        let fetched = library_command(
            &cache,
            Some(&cli),
            json!({"cmd": "library_fetch", "url": slashes(dir)}),
        );
        assert_eq!(fetched["error"], Value::Null, "{fetched}");
    }
    let found = library_command(&cache, None, json!({"cmd": "library_search"}));
    let component = |id: &str| -> Value {
        found["components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .cloned()
            .unwrap_or_else(|| panic!("{id} not found: {found}"))
    };
    let screw = component("screw");
    assert_eq!(screw["category"], "slot-screws");
    assert_eq!(
        screw["categories"],
        json!(["slot-screws", "general-screws"])
    );
    assert_eq!(
        screw["groups"],
        json!([{"category": "slot-screws", "path": ["Slot profile", "Screws"], "order": [3, 3]},
               {"category": "general-screws", "path": ["General fasteners", "Screws"],
                "order": [4, 4]}])
    );
    assert_eq!(
        component("beam")["groups"][0]["path"],
        json!(["Wood", "Dimensioned timber"])
    );
    // The other library's Wood has the same name: one group in the picker.
    let plywood = component("plywood");
    assert_eq!(
        plywood["groups"],
        json!([{"category": "sheets", "path": ["Wood", "Sheets"], "order": [0, 1]}])
    );
    assert_eq!(component("none")["groups"], json!([]));
    // `category` finds a part by any of its categories, once.
    let general = library_command(
        &cache,
        None,
        json!({"cmd": "library_search", "category": "general-screws"}),
    );
    let ids: Vec<&str> = general["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["screw", "bolt"], "{general}");
    // And a search word by any of them.
    let words = library_command(
        &cache,
        None,
        json!({"cmd": "library_search", "text": "general-screws"}),
    );
    assert_eq!(words["components"].as_array().unwrap().len(), 2, "{words}");
}
