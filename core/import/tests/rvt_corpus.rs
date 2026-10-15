// SPDX-License-Identifier: MIT
//! The `.rvt` import on the corpus's MIT-licensed 2024 project (by its
//! content id under `MITCAD_RVT_CORPUS`; skipped without it), on the mock
//! kernel: every building element the reference reader lists (its 970
//! elements but the rooms, which are spaces) comes in as a body named after
//! it, in the component of the level the reference names.

use std::collections::HashMap;
use std::path::PathBuf;

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_rvt::RvtFile;
use mitcad_rvt::elements::Model;

const PROJECT: &str = "c805df44";

fn project() -> Option<(PathBuf, String)> {
    let dir = PathBuf::from(std::env::var_os("MITCAD_RVT_CORPUS")?);
    let index = std::fs::read_to_string(dir.join("index.tsv")).ok()?;
    let path = index.lines().find_map(|line| {
        let mut cols = line.split('\t');
        let path = cols.next()?;
        cols.next()?.starts_with(PROJECT).then(|| path.to_string())
    })?;
    dir.join(&path).is_file().then_some((dir, path))
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
