// SPDX-License-Identifier: MIT
//! The IFC import on the mock kernel, checked through the api's queries
//! (mitcad#831; the import's other tests are in `core/import-ifc`).

use mitcad_ifc::IfcFile;
use mitcad_ifc::testdata::BUILDING;
use mitcad_import_ifc::{IfcOptions, IfcReport, import_ifc};
use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_model_api::DocumentApi;

fn imported() -> (Document<MockKernel>, IfcReport) {
    let kernel = MockKernel::default();
    kernel.measures.set(true);
    let mut doc = Document::new(kernel);
    let file = IfcFile::parse(BUILDING).expect("reads");
    let report = import_ifc(&mut doc, &file, "building.ifc", &IfcOptions::default());
    (doc, report)
}

/// The value of a stored property, as JSON (mitcad#496).
fn stored(
    doc: &Document<MockKernel>,
    owner: mitcad_model::properties::PropertyOwner,
    set: &str,
    key: &str,
) -> Option<(serde_json::Value, &'static str)> {
    let p = doc.property_store().get(owner, set, key)?;
    Some((p.value.to_json(), p.source.as_str()))
}

#[test]
fn property_sets_land_on_their_levels() {
    use mitcad_model::properties::PropertyOwner;
    use serde_json::json;
    let (doc, report) = imported();
    assert!(report.properties > 0, "{}", report.text());
    // The wall's own set and quantities on its bodies, from the file.
    let wall = report
        .products
        .iter()
        .find(|p| p.id == 100)
        .expect("the wall");
    assert!(!wall.bodies.is_empty());
    for body in &wall.bodies {
        let owner = PropertyOwner::Body(body.parse().expect("a body id"));
        assert_eq!(
            stored(&doc, owner, "Pset_WallCommon", "FireRating"),
            Some((json!("EI60"), "ifc"))
        );
        assert_eq!(
            stored(&doc, owner, "Pset_WallCommon", "IsExternal"),
            Some((json!(true), "ifc"))
        );
        // mm in the file's millimetres.
        assert_eq!(
            stored(&doc, owner, "Pset_WallCommon", "Width"),
            Some((json!(200.0), "ifc"))
        );
        assert_eq!(
            stored(&doc, owner, "Qto_WallBaseQuantities", "NetVolume"),
            Some((json!(2.8e9), "ifc"))
        );
        assert_eq!(
            stored(&doc, owner, "Identity", "GlobalId"),
            Some((json!("1WALL00000000000000000"), "ifc"))
        );
        assert_eq!(
            stored(&doc, owner, "Identity", "Class"),
            Some((json!("IFCWALL"), "ifc"))
        );
        let width = doc
            .property_store()
            .get(owner, "Pset_WallCommon", "ThermalTransmittance")
            .expect("a measure with its unit");
        assert_eq!(width.unit.as_deref(), Some("W/(m²·K)"));
    }
    // The windows: their identity on their occurrences, their type's set
    // once on the component their type's shape became.
    let mut components = Vec::new();
    for id in [130, 830] {
        let line = report
            .products
            .iter()
            .find(|p| p.id == id)
            .expect("a window");
        assert_eq!(line.occurrences.len(), 1);
        let uid: mitcad_model::OccurrenceUid = line.occurrences[0].parse().expect("an id");
        let owner = PropertyOwner::Occurrence(uid);
        let (global_id, _) = stored(&doc, owner, "Identity", "GlobalId").expect("its id");
        assert!(
            global_id.as_str().unwrap().starts_with("1WIN"),
            "{global_id}"
        );
        assert!(stored(&doc, owner, "Pset_WindowCommon", "GlazingAreaFraction").is_none());
        components.push(doc.assembly().occurrence(uid).unwrap().component);
    }
    assert_eq!(components[0], components[1]);
    let window_type = PropertyOwner::Component(components[0]);
    assert_eq!(
        stored(
            &doc,
            window_type,
            "Pset_WindowCommon",
            "GlazingAreaFraction"
        ),
        Some((json!(0.7), "ifc"))
    );
    assert_eq!(
        stored(&doc, window_type, "Pset_WindowCommon", "Manufactured"),
        Some((json!("2026-03-01"), "ifc"))
    );
    assert_eq!(
        stored(&doc, window_type, "Identity", "Type"),
        Some((json!("Window type"), "ifc"))
    );
    // An occurrence shows its type's values as inherited.
    let levels: serde_json::Value = serde_json::from_str(
        &doc.query(
            &json!({"query": "property_sets",
                    "occurrence": report.products.iter().find(|p| p.id == 130).unwrap().occurrences[0]})
            .to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    let occurrence = &levels["levels"][0];
    let window = occurrence["sets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Pset_WindowCommon")
        .expect("the type's set");
    assert_eq!(window["properties"][0]["inherited"], true);
    assert_eq!(window["properties"][0]["source"], "ifc");
}
