// SPDX-License-Identifier: MIT
//! The IFC import on the mock kernel: the structure, the outcome of every
//! product of the test building, the features and occurrences made. The
//! property sets on their levels, read through the api's query, are tested
//! in `core/model-tests/imports` (mitcad#831).

use mitcad_ifc::IfcFile;
use mitcad_ifc::testdata::{BUILDING, BUILDING_PRODUCTS};
use mitcad_model::Document;
use mitcad_model::testing::MockKernel;

use super::{IfcOptions, Outcome, import_ifc};

fn imported() -> (Document<MockKernel>, super::IfcReport) {
    let kernel = MockKernel::default();
    kernel.measures.set(true);
    let mut doc = Document::new(kernel);
    let file = IfcFile::parse(BUILDING).expect("reads");
    let report = import_ifc(&mut doc, &file, "building.ifc", &IfcOptions::default());
    (doc, report)
}

fn outcome(report: &super::IfcReport, id: u64) -> Outcome {
    report
        .products
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("#{id} has a line"))
        .outcome
}

#[test]
fn every_product_is_accounted_for() {
    let (doc, report) = imported();
    assert_eq!(
        report.products.len(),
        BUILDING_PRODUCTS,
        "{}",
        report.text()
    );
    for (id, expected) in [
        (40, Outcome::Component),
        (42, Outcome::Component),
        (44, Outcome::Component),
        (46, Outcome::Component),
        (100, Outcome::Parametric),
        (110, Outcome::Cut),
        (130, Outcome::Occurrence),
        (830, Outcome::Occurrence),
        (200, Outcome::Parametric),
        (300, Outcome::Parametric),
        (400, Outcome::Parametric),
        (500, Outcome::Stored),
        (600, Outcome::Mesh),
        (700, Outcome::NotImported),
        (900, Outcome::Axes),
        (960, Outcome::NotImported),
    ] {
        assert_eq!(outcome(&report, id), expected, "#{id}: {}", report.text());
    }
    assert_eq!(report.components, 4);
    assert_eq!(report.maps.len(), 1);
    assert_eq!(report.maps[0].occurrences, 2);
    assert_eq!(report.maps[0].name, "Window type");
    assert_eq!(report.storey_planes, 2);
    assert_eq!(report.grid_axes, 2);
    // One undo step for the whole import.
    assert_eq!(doc.undo_depth(), 1);
    assert_eq!(doc.undo_label(), Some("Import building.ifc"));
    // Bodies named by the element's name and GlobalId.
    let names: Vec<String> = doc.bodies().iter().map(|b| b.name.clone()).collect();
    assert!(
        names.iter().any(|n| n == "Wall A 1WALL00000000000000000"),
        "{names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "Tetrahedron 1MESH00000000000000000")
    );
    // The tetrahedron's red.
    assert_eq!(report.appearances, 2, "{}", report.text());
}

#[test]
fn features_of_the_elements() {
    let (doc, report) = imported();
    let wall = report
        .products
        .iter()
        .find(|p| p.id == 100)
        .expect("a wall");
    // A sketch and an extrusion, the opening's sketch and its cut.
    assert_eq!(wall.features, 4, "{}", report.text());
    assert_eq!(wall.parametric, vec!["extrusion", "extrusion"]);
    let column = report
        .products
        .iter()
        .find(|p| p.id == 400)
        .expect("a column");
    assert_eq!(column.parametric, vec!["extrusion", "half space"]);
    let types: Vec<&str> = doc.features().map(|f| f.def.base_name()).collect();
    assert!(types.contains(&"Plane"), "{types:?}");
    assert!(types.contains(&"Axis"), "{types:?}");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

/// A type's block used once as it is and once scaled non-uniformly and
/// mirrored: the first an occurrence of the type's component, the second
/// a copy of its extrusion, scaled.
const SCALED: &str = r"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('ViewDefinition [DesignTransferView]'),'2;1');
FILE_NAME('scaled.ifc','2026-10-14T00:00:00',(''),(''),'Mitcad test','Mitcad test','');
FILE_SCHEMA(('IFC4'));
ENDSEC;
DATA;
#1=IFCPROJECT('0PROJ00000000000000000',$,'Scaled',$,$,$,$,$,#2);
#2=IFCUNITASSIGNMENT((#3));
#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
#10=IFCBUILDING('2BLDG00000000000000000',$,'Building',$,$,#11,$,$,.ELEMENT.,$,$,$);
#11=IFCLOCALPLACEMENT($,#12);
#12=IFCAXIS2PLACEMENT3D(#13,$,$);
#13=IFCCARTESIANPOINT((0.,0.,0.));
#14=IFCRELAGGREGATES('3AG0000000000000000000',$,$,$,#1,(#10));
#15=IFCRELCONTAINEDINSPATIALSTRUCTURE('3CO0000000000000000000',$,$,$,(#20,#30),#10);
#20=IFCBUILDINGELEMENTPROXY('1PRX000000000000000000',$,'Plain',$,$,#11,#21,$,$);
#21=IFCPRODUCTDEFINITIONSHAPE($,$,(#22));
#22=IFCSHAPEREPRESENTATION($,'Body','MappedRepresentation',(#23));
#23=IFCMAPPEDITEM(#40,#24);
#24=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#13,$,$);
#30=IFCBUILDINGELEMENTPROXY('1PRX100000000000000000',$,'Scaled',$,$,#11,#31,$,$);
#31=IFCPRODUCTDEFINITIONSHAPE($,$,(#32));
#32=IFCSHAPEREPRESENTATION($,'Body','MappedRepresentation',(#33));
#33=IFCMAPPEDITEM(#40,#34);
#34=IFCCARTESIANTRANSFORMATIONOPERATOR3DNONUNIFORM(#35,#37,#36,2.,$,1.,3.);
#37=IFCDIRECTION((0.,1.,0.));
#35=IFCDIRECTION((-1.,0.,0.));
#36=IFCCARTESIANPOINT((500.,0.,0.));
#40=IFCREPRESENTATIONMAP(#12,#41);
#41=IFCSHAPEREPRESENTATION($,'Body','SweptSolid',(#42));
#42=IFCEXTRUDEDAREASOLID(#43,$,#45,100.);
#43=IFCRECTANGLEPROFILEDEF(.AREA.,$,$,100.,100.);
#45=IFCDIRECTION((0.,0.,1.));
ENDSEC;
END-ISO-10303-21;
";

#[test]
fn scaled_and_mirrored_maps_are_copied() {
    let kernel = MockKernel::default();
    let mut doc = Document::new(kernel);
    let file = IfcFile::parse(SCALED).expect("reads");
    let report = import_ifc(&mut doc, &file, "scaled.ifc", &IfcOptions::default());
    assert_eq!(
        outcome(&report, 20),
        Outcome::Occurrence,
        "{}",
        report.text()
    );
    assert_eq!(
        outcome(&report, 30),
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let scaled = report.products.iter().find(|p| p.id == 30).expect("a line");
    assert_eq!(scaled.parametric, vec!["extrusion"]);
    assert_eq!(report.maps.len(), 1);
    assert_eq!(report.maps[0].occurrences, 1);
}
