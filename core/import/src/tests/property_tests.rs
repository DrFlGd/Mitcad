// SPDX-License-Identifier: MIT
//! The part numbers and descriptions a dump gives its components become
//! the components' properties with the source `f3d` (mitcad#496).

use super::*;
use mitcad_model::properties::{PropertyOwner, PropertyValue, Source};

#[test]
fn components_bring_their_part_numbers() {
    let dump = dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "part.f3d"},
        "document": {"name": "Part", "root_component": "Part"},
        "components": [{"name": "Part", "is_root": true, "partNumber": " P-100 ",
                        "description": "A bracket", "_f3d": {"object_id": 1}}],
        "timeline": {"items": []}
    }));
    let mut doc = Document::new(MockKernel::default());
    import(&mut doc, &dump);
    let store = doc.property_store();
    let root = PropertyOwner::Component(ComponentUid::ROOT);
    let number = store
        .get(root, "Component", "Part Number")
        .expect("the part number");
    assert_eq!(number.value, PropertyValue::Text("P-100".to_owned()));
    assert_eq!(number.source, Source::F3d);
    assert_eq!(
        store.get(root, "Component", "Description").unwrap().value,
        PropertyValue::Text("A bracket".to_owned())
    );
}
