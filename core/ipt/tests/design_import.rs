// SPDX-License-Identifier: MIT
//! The design decoded from definitions records (`mitcad_ipt::testdesign`)
//! replayed by the import on the model's mock geometry kernel: the
//! parameters with their expressions, the sketch with its constraints and
//! dimensions, and the extrusion, all parametric.

use mitcad_import::{NoGeometry, Options, Outcome, import_design};
use mitcad_ipt::dc::Definitions;
use mitcad_ipt::params::DisplayUnit;
use mitcad_model::Document;
use mitcad_model::testing::MockKernel;

#[test]
fn the_cube_design_replays_parametric() {
    for major in [20, 22, 24, 25, 28] {
        let dc = Definitions::from_records(major, &mitcad_ipt::testdesign::cube_design(major));
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "cube.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(
            outcomes,
            [Outcome::Parametric, Outcome::Parametric],
            "{}",
            report.text()
        );
        assert_eq!(report.parameters.imported, 5, "{:?}", report.parameters);
        assert!(report.parameters.mismatched.is_empty());
        assert!(report.parameters.literal.is_empty());
        let params = doc.parameters();
        let side = params.get(params.find("Side").unwrap()).unwrap();
        assert_eq!(side.expression(), "10 mm");
        assert!((side.value() - 10.0).abs() < 1e-12);
        let d0 = params.get(params.find("d0").unwrap()).unwrap();
        assert_eq!(d0.expression(), "Side");
        // The sketch owns its dimension's parameter, the extrusion its
        // distance.
        assert!(params.owner(params.find("d0").unwrap()).is_some());
        assert!(params.owner(params.find("d2").unwrap()).is_some());
        assert_eq!(doc.bodies().len(), 1);
    }
}

/// An extrusion on two sides (10 mm and 5 mm) replays as Mitcad's two-sided
/// extent with both distances as its parameters.
#[test]
fn a_two_sided_extrusion_replays_parametric() {
    for major in [21, 24, 28] {
        let records = mitcad_ipt::testdesign::two_sided_cube_design(major, 0.5);
        let dc = Definitions::from_records(major, &records);
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "cube.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(
            outcomes,
            [Outcome::Parametric, Outcome::Parametric],
            "{}",
            report.text()
        );
        let params = doc.parameters();
        let d4 = params.get(params.find("d4").unwrap()).unwrap();
        assert!((d4.value() - 5.0).abs() < 1e-12, "{}", d4.expression());
        assert!(params.owner(params.find("d4").unwrap()).is_some());
        assert_eq!(doc.bodies().len(), 1);
    }
}

/// A sketch of a placed block (a square) and its extrusion replay
/// parametric.
#[test]
fn a_placed_sketch_block_replays_parametric() {
    for major in [24, 27] {
        let records = mitcad_ipt::testdesign::block_design(major);
        let dc = Definitions::from_records(major, &records);
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "block.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(
            outcomes,
            [Outcome::Parametric, Outcome::Parametric],
            "{}",
            report.text()
        );
        assert_eq!(doc.bodies().len(), 1);
    }
}

/// The cube from a square of lines flagged 0x40 whose profile the
/// extrusion selected: the lines bound it, and both items are parametric.
#[test]
fn flagged_lines_of_a_selected_profile_make_the_cube() {
    for major in [21, 24] {
        let dc =
            Definitions::from_records(major, &mitcad_ipt::testdesign::flagged_cube_design(major));
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "cube.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(
            outcomes,
            [Outcome::Parametric, Outcome::Parametric],
            "{}",
            report.text()
        );
        assert_eq!(doc.bodies().len(), 1);
    }
}

/// A square and its offset, driven by the distance between a side and its
/// offset, comes in whole.
#[test]
fn the_offset_replays_parametric() {
    for major in [21, 24, 28] {
        let dc = Definitions::from_records(major, &mitcad_ipt::testdesign::offset_design(major));
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "offset.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(outcomes, [Outcome::Parametric], "{}", report.text());
    }
}

/// A sketch with a polygon, a circular and a rectangular pattern, points
/// on horizontal and vertical lines and an arc with a point on it comes in
/// whole.
#[test]
fn the_sketch_groups_replay_parametric() {
    for major in [21, 24, 25, 28] {
        let dc = Definitions::from_records(major, &mitcad_ipt::testdesign::groups_design(major));
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "groups.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(outcomes, [Outcome::Parametric], "{}", report.text());
    }
}

/// Work planes through work points and axes come in as Mitcad's planes
/// through points and lines, where the file has them.
#[test]
fn planes_through_points_and_axes_replay_parametric() {
    for major in [18, 24, 28] {
        let dc = Definitions::from_records(major, &mitcad_ipt::testdesign::planes_design(major));
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "planes.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(outcomes, [Outcome::Parametric; 4], "{}", report.text());
    }
}

/// Radial lines, a line of no length, a pattern on its originals and a
/// linear diameter: the sketch comes in whole.
#[test]
fn the_sketch_fixes_replay_parametric() {
    for major in [21, 24, 28] {
        let dc =
            Definitions::from_records(major, &mitcad_ipt::testdesign::sketch_fixes_design(major));
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "fixes.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(outcomes, [Outcome::Parametric], "{}", report.text());
    }
}

/// Functions in expressions come in as Mitcad's; a parameter nothing uses
/// whose expression does not give its value keeps the value.
#[test]
fn functions_replay_and_unused_parameters_keep_their_values() {
    let dc = Definitions::from_records(25, &mitcad_ipt::testdesign::function_design(25));
    let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "functions.ipt", None);
    let mut doc = Document::new(MockKernel::default());
    let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
    assert!(
        report.parameters.mismatched.is_empty(),
        "{:?}",
        report.parameters
    );
    let params = doc.parameters();
    let d0 = params.get(params.find("d0").unwrap()).unwrap();
    assert_eq!(d0.expression(), "SW / cos(30 deg)");
    assert!((d0.value() - 2.0 / 0.75f64.sqrt()).abs() < 1e-9);
    let d1 = params.get(params.find("d1").unwrap()).unwrap();
    assert_eq!(d1.expression(), "SW / 2");
    let d2 = params.get(params.find("d2").unwrap()).unwrap();
    assert!((d2.value() - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
}

/// Two cubes and a combine of their bodies, the bodies named by the
/// extrusions that made them: every item parametric, one body left.
#[test]
fn a_combine_joins_the_bodies_its_record_names() {
    for major in [21, 24, 28] {
        let dc =
            Definitions::from_records(major, &mitcad_ipt::testdesign::combine_design(major, false));
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "combine.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(outcomes, [Outcome::Parametric; 5], "{}", report.text());
        assert_eq!(doc.bodies().len(), 1, "{}", report.text());
    }
}

/// A sheet-metal face (mitcad#211): its style's rules and the face come in
/// as Mitcad's sheet rules (named after the style, with the style's
/// parameters and K-factor) and sheet face, at the face's place on the
/// timeline, the material on the side the plate's direction gives; a
/// second face of the style uses the same rules; a
/// changed `Thickness` rebuilds the rules.
#[test]
fn a_sheet_metal_face_replays_as_sheet_rules_and_a_sheet_face() {
    use mitcad_model::features::FeatureDef;
    let style = mitcad_ipt::sheet::Style {
        name: Some("Default_mm".to_owned()),
        unfold: Some("Default_KFactor".to_owned()),
        k_factor: Some((0.44, "0.440 ul".to_owned())),
    };
    for (major, direction, side) in [
        (24, [0.0, 0.0, 1.0], "front"),
        (28, [0.0, 0.0, -1.0], "back"),
    ] {
        let records = mitcad_ipt::testdesign::sheet_face_design(major, direction, 0, 2);
        let dc = Definitions::from_records(major, &records);
        let design =
            mitcad_ipt::design::design_with(&dc, DisplayUnit::Mm, "sheet.ipt", None, &style);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        let outcomes: Vec<Outcome> = report.items.iter().map(|i| i.outcome).collect();
        assert_eq!(outcomes, [Outcome::Parametric; 3], "{}", report.text());
        // The timeline: the sketch, the rules and the face, by name.
        let timeline: Vec<(String, &str)> = doc
            .features()
            .map(|f| {
                let kind = match &f.def {
                    FeatureDef::Sketch(_) => "sketch",
                    FeatureDef::SheetRules(_) => "sheet_rules",
                    FeatureDef::SheetFace(_) => "sheet_face",
                    _ => "other",
                };
                (f.name.clone(), kind)
            })
            .collect();
        assert_eq!(
            timeline,
            [
                ("Sketch1".to_owned(), "sketch"),
                ("Sheet rules Default_mm".to_owned(), "sheet_rules"),
                ("Face1".to_owned(), "sheet_face"),
                ("Face2".to_owned(), "sheet_face"),
            ]
        );
        let rules: serde_json::Value =
            serde_json::from_str(&doc.query(r#"{"query": "sheet_rules"}"#).unwrap()).unwrap();
        let r = &rules[0]["rules"];
        assert_eq!(r["reference_side"], side, "{rules}");
        assert!(
            (r["thickness"].as_f64().unwrap() - 1.0).abs() < 1e-9,
            "{rules}"
        );
        assert!((r["allowance"]["k"].as_f64().unwrap() - 0.44).abs() < 1e-12);
        assert!((r["corner_relief"]["width"].as_f64().unwrap() - 4.0).abs() < 1e-9);
        assert_eq!(doc.bodies().len(), 2);
        // Both faces follow the one rule.
        assert_eq!(rules.as_array().map(Vec::len), Some(1), "{rules}");
        // The thickness follows the style's parameter.
        doc.set_parameter_expression("Thickness", "2 mm", None)
            .unwrap();
        let rules: serde_json::Value =
            serde_json::from_str(&doc.query(r#"{"query": "sheet_rules"}"#).unwrap()).unwrap();
        assert!((rules[0]["rules"]["thickness"].as_f64().unwrap() - 2.0).abs() < 1e-9);
        assert!((rules[0]["rules"]["bend_radius"].as_f64().unwrap() - 2.0).abs() < 1e-9);
    }
}

/// A dimension between geometry projected from the model is driven while
/// its parameter `d0` is in the part's table: the import makes `d0` the
/// sketch's reference with the value the sketch measures (mitcad#250),
/// not a user parameter beside a reference of the dimension's own; `Gap`
/// = `d0` follows it.
#[test]
fn a_driven_dimensions_table_parameter_is_its_reference() {
    use mitcad_model::expr::ParamKind;
    for major in [21, 24, 28] {
        let records = mitcad_ipt::testdesign::driven_reference_design(major);
        let dc = Definitions::from_records(major, &records);
        let design = mitcad_ipt::design::design(&dc, DisplayUnit::Mm, "driven.ipt", None);
        let mut doc = Document::new(MockKernel::default());
        let report = import_design(&mut doc, &design.dump, &mut NoGeometry, &Options::default());
        assert_eq!(report.items.len(), 1, "{}", report.text());
        let params = doc.parameters();
        let d0 = params
            .find("d0")
            .unwrap_or_else(|| panic!("major {major}: no d0\n{}", report.text()));
        let p = params.get(d0).unwrap();
        assert_eq!(
            p.kind(),
            ParamKind::Reference,
            "major {major}\n{}",
            report.text()
        );
        assert!(params.owner(d0).is_some());
        assert!((p.value() - 10.0).abs() < 1e-9, "{}", p.value());
        let gap = params.get(params.find("Gap").unwrap()).unwrap();
        assert!((gap.value() - 10.0).abs() < 1e-9, "{}", gap.value());
        // No reference of the dimension's own beside it.
        let references = params
            .iter()
            .filter(|p| p.kind() == ParamKind::Reference)
            .count();
        assert_eq!(references, 1, "major {major}");
    }
}
