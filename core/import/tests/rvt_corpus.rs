// SPDX-License-Identifier: MIT
//! The `.rvt` import on the corpus's MIT-licensed 2024 project (by its
//! content id under `MITCAD_RVT_CORPUS`; skipped without it), on the mock
//! kernel: every building element the reference reader lists (its 970
//! elements but the rooms, which are spaces) comes in as a body named after
//! it, in the component of the level the reference names; its doors,
//! windows, curtain panels and mullions and furniture as features or
//! meshes of the geometry the file keeps (mitcad#333), every box with its
//! reason. On the 2024 architectural sample (`33271010`), the plans of the
//! walls attached to roofs, floors and reference planes (mitcad#348); on
//! both, the walls' openings in their plans (mitcad#345).

use std::collections::HashMap;
use std::path::PathBuf;

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_rvt::RvtFile;
use mitcad_rvt::elements::Model;

const PROJECT: &str = "c805df44";

/// The 2024 architectural sample with roofs, walls attached to them and
/// openings of several kinds (mitcad#345, mitcad#348).
const SAMPLE: &str = "33271010";

fn project() -> Option<(PathBuf, String)> {
    corpus_file(PROJECT)
}

/// A corpus file by its content id: the corpus folder and its path there.
fn corpus_file(id: &str) -> Option<(PathBuf, String)> {
    let dir = PathBuf::from(std::env::var_os("MITCAD_RVT_CORPUS")?);
    let index = std::fs::read_to_string(dir.join("index.tsv")).ok()?;
    let path = index.lines().find_map(|line| {
        let mut cols = line.split('\t');
        let path = cols.next()?;
        cols.next()?.starts_with(id).then(|| path.to_string())
    })?;
    dir.join(&path).is_file().then_some((dir, path))
}

/// The walls' openings in their plans (mitcad#345): every door and window
/// a wall built as features hosts is among its openings, and every opening
/// has its cut tools or a reason. Returns the openings with tools, those
/// cut by void geometry, and the reasons of those without, counted.
fn openings_of(id: &str, model: &Model) -> (usize, usize, HashMap<String, usize>) {
    use mitcad_rvt::categories::{OST_DOORS, OST_WINDOWS};
    let b = &model.building;
    let (mut with_tools, mut by_void) = (0, 0);
    let mut missing: HashMap<String, usize> = HashMap::new();
    for (wid, wall) in &b.walls {
        let Some(e) = model.element(*wid) else {
            continue;
        };
        let Ok(p) = mitcad_import::rvt::plan::plan(model, e) else {
            continue;
        };
        for o in &p.openings {
            match &o.missing {
                None => with_tools += 1,
                Some(why) => {
                    eprintln!("{id}: {} in wall {wid}: {why}", o.label);
                    *missing.entry(why.clone()).or_default() += 1;
                }
            }
        }
        by_void += p
            .notes
            .iter()
            .filter(|n| n.ends_with("is cut by its type's void geometry"))
            .count();
        for i in b.instances.values().filter(|i| i.host == Some(wall.id)) {
            let category = model.element(i.id).and_then(|e| e.category);
            if matches!(category, Some(OST_DOORS | OST_WINDOWS)) {
                assert!(
                    p.openings.iter().any(|o| o.id == i.id),
                    "{id}: insert {} of wall {wid} is not among its openings",
                    i.id
                );
            }
        }
    }
    eprintln!(
        "{id}: {with_tools} openings with their tools ({by_void} by void geometry), without: {missing:?}"
    );
    (with_tools, by_void, missing)
}

#[test]
fn the_walls_openings_have_their_tools_or_say_why() {
    // mitcad#345: on the MIT project every door and window of a wall has
    // its type's cut loops; on the architectural sample the inserts whose
    // types cut by their void geometry cut by it, and the rest say why.
    for id in [PROJECT, SAMPLE] {
        let Some((dir, path)) = corpus_file(id) else {
            eprintln!("MITCAD_RVT_CORPUS has no {id}: skipped");
            continue;
        };
        let file = RvtFile::open(&dir.join(&path)).unwrap();
        let schema = file.schema().unwrap();
        let model = Model::read(&file, &schema).unwrap();
        let (with_tools, by_void, missing) = openings_of(id, &model);
        assert!(with_tools > 0, "{id}");
        if id == PROJECT {
            assert!(missing.is_empty(), "{id}: {missing:?}");
            assert!(with_tools >= 192, "{id}: {with_tools}");
        } else {
            assert!(by_void > 0, "{id}: no insert cut by its void geometry");
        }
    }
}

#[test]
fn the_sample_s_attached_walls_follow_their_roofs_or_say_why() {
    // mitcad#348: every wall attached to a roof, floor or reference plane
    // follows its surface, or its plan names the attachment it does not
    // apply, or the wall is not built as features (with the reason).
    let Some((dir, path)) = corpus_file(SAMPLE) else {
        eprintln!("MITCAD_RVT_CORPUS has no {SAMPLE}: skipped");
        return;
    };
    let file = RvtFile::open(&dir.join(&path)).unwrap();
    let schema = file.schema().unwrap();
    let model = Model::read(&file, &schema).unwrap();
    let (mut applied, mut not_applied, mut not_built) = (0, 0, 0);
    for (id, wall) in &model.building.walls {
        if wall.attachments.is_empty() {
            continue;
        }
        let Some(e) = model.element(*id) else {
            continue;
        };
        match mitcad_import::rvt::plan::plan(&model, e) {
            Ok(p) => {
                assert!(!p.attached.is_empty(), "wall {id}");
                for a in &p.attached {
                    eprintln!("{SAMPLE}: wall {id}: {a}");
                }
                let follows = p.notes.iter().filter(|n| n.contains(" follows ")).count();
                let skipped = p
                    .notes
                    .iter()
                    .filter(|n| n.contains("is not applied"))
                    .count()
                    + usize::from(p.notes.iter().any(|n| n.contains("its edited profile")));
                assert!(follows + skipped >= 1, "wall {id}: {:?}", p.notes);
                applied += follows;
                not_applied += skipped;
            }
            Err(why) => {
                eprintln!("{SAMPLE}: wall {id} not built as features: {why}");
                not_built += 1;
            }
        }
    }
    eprintln!(
        "{SAMPLE}: {applied} attachments applied, {not_applied} not applied (named), \
         {not_built} attached walls not built as features"
    );
    assert!(applied > 0, "no attachment applied");
}

#[test]
fn the_mit_project_by_level() {
    let Some((dir, path)) = project() else {
        eprintln!("MITCAD_RVT_CORPUS has no {PROJECT}: skipped");
        return;
    };
    let file = RvtFile::open(&dir.join(&path)).unwrap();
    let schema = file.schema().unwrap();
    let model = Model::read(&file, &schema).unwrap();
    let mut document = Document::new(MockKernel::default());
    let report = mitcad_import::rvt::import_rvt(
        &mut document,
        "project.rvt",
        &model,
        &mitcad_import::rvt::RvtOptions::default(),
    )
    .unwrap();
    assert!(report.not_built.is_empty(), "{:?}", report.not_built);

    // Where each element's body is: the levels of its component (one, or
    // several of the same name and elevation, mitcad#310).
    let mut level_of: HashMap<i64, Vec<Option<i64>>> = HashMap::new();
    for level in &report.levels {
        let ids: Vec<Option<i64>> = std::iter::once(level.id)
            .chain(level.merged.iter().map(|m| Some(*m)))
            .collect();
        for id in &level.elements {
            level_of.insert(*id, ids.clone());
        }
    }
    let names: Vec<String> = document.bodies().iter().map(|b| b.name.clone()).collect();
    assert_eq!(names.len(), report.bodies);

    let data = std::fs::read(dir.join("reference").join(&path).join("elements.json.gz")).unwrap();
    let inflated: Vec<u8> = mitcad_rvt::gzip::members_from(&data, 0)
        .unwrap()
        .into_iter()
        .flat_map(|m| m.data)
        .collect();
    let reference: serde_json::Value = serde_json::from_slice(&inflated).unwrap();
    let mut compared = 0;
    for element in reference.as_array().unwrap() {
        let Some(id) = element["id"].as_i64() else {
            continue;
        };
        if matches!(element["class_name"].as_str(), Some("Room" | "Material")) {
            continue;
        }
        let level = element["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == "m_levelId")
            .and_then(|f| f["id"].as_i64());
        let Some(placed) = level_of.get(&id) else {
            panic!("element {id} ({}) has no body", element["class_name"]);
        };
        assert!(placed.contains(&level), "element {id} on another level");
        let suffix = format!(":{id}");
        assert!(
            names.iter().any(|n| n.ends_with(&suffix)),
            "no body named after {id}"
        );
        compared += 1;
    }
    assert_eq!(compared, 854);

    // The family elements come in with their geometry, not as boxes
    // (mitcad#333): doors, windows, curtain panels and mullions, furniture
    // (the members of the placed groups among them).
    let mut by_category: HashMap<&str, usize> = HashMap::new();
    for line in &report.elements {
        let wanted = [
            "OST_Doors",
            "OST_Windows",
            "OST_CurtainWallPanels",
            "OST_CurtainWallMullions",
            "OST_Furniture",
        ];
        if wanted.contains(&line.category.as_str()) {
            assert!(
                matches!(line.outcome.as_str(), "mesh" | "features"),
                "{line:?}"
            );
            *by_category.entry(line.category.as_str()).or_default() += 1;
        }
        // A box says why, and is hidden unless structural.
        if line.outcome == "box" {
            assert!(line.notes.iter().any(|n| n.contains("box: ")), "{line:?}");
            assert_eq!(
                line.hidden,
                line.kind == "other" || !mitcad_import::rvt::plan::structural(&line.kind),
                "{line:?}"
            );
        }
    }
    for (category, least) in [
        ("OST_Doors", 185),
        ("OST_Windows", 7),
        ("OST_CurtainWallPanels", 5000),
        ("OST_CurtainWallMullions", 6000),
        ("OST_Furniture", 16),
    ] {
        assert!(
            by_category.get(category).copied().unwrap_or(0) >= least,
            "{category}: {by_category:?}"
        );
    }
    eprintln!(
        "{PROJECT}: {} bodies ({} meshes, {} oriented boxes, {} boxes) on {} levels, {} by elevation; \
         the reference's {compared} building elements each on its level",
        report.bodies,
        report.meshes,
        report.oriented_boxes,
        report.boxes,
        report.levels.iter().filter(|l| l.bodies > 0).count(),
        report.by_elevation
    );
}
