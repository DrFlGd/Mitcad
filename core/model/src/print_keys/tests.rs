// SPDX-License-Identifier: MIT
//! The key map's table and its conversions in both directions.

use std::collections::{HashMap, HashSet};

use serde_json::json;

use super::*;

fn def(name: &str) -> &'static KeyDef {
    key(name).unwrap_or_else(|| panic!("no key {name}"))
}

/// Reads `text` of the key's own dialect key with the values of `context`.
fn read_with(name: &str, dialect: Dialect, text: &str, context: &[(&str, Value)]) -> Value {
    let values: HashMap<&str, Value> = context.iter().cloned().collect();
    read_text(def(name), dialect, text, &|k| values.get(k).cloned())
        .unwrap_or_else(|e| panic!("{name} from {dialect}: {e}"))
        .unwrap_or_else(|| panic!("{name} from {dialect}: nothing"))
}

/// Writes a value of the key in a dialect: its one key's text.
fn write_with(name: &str, dialect: Dialect, value: &Value, context: &[(&str, Value)]) -> String {
    let values: HashMap<&str, Value> = context.iter().cloned().collect();
    let written = write(def(name), dialect, value, &|k| values.get(k).cloned())
        .unwrap_or_else(|e| panic!("{name} to {dialect}: {e}"));
    assert_eq!(written.len(), 1, "{name} to {dialect}: {written:?}");
    written[0].1.clone()
}

fn written_key(name: &str, dialect: Dialect) -> &'static str {
    def(name).dialect(dialect).unwrap().key
}

use Dialect::{BambuStudio, Cura, OrcaSlicer, PrusaSlicer, QidiStudio};

#[test]
fn keys_are_unique_and_every_reference_is_a_key() {
    let mut seen = HashSet::new();
    for k in keys() {
        assert!(seen.insert(k.key), "{} twice", k.key);
        assert!(!k.name.is_empty(), "{} has no name", k.key);
        let bases = match k.kind {
            Kind::OrPercent(_, basis) => vec![basis],
            _ => Vec::new(),
        };
        let forms = k.dialects.iter().filter_map(|d| match d.form {
            Form::OrPercent(of) | Form::PercentOf(of) => Some(of),
            _ => None,
        });
        for basis in bases.into_iter().chain(forms) {
            assert!(
                basis.is_empty() || key(basis).is_some(),
                "{}: no key {basis}",
                k.key
            );
        }
    }
    assert!(keys().len() >= 400, "{} keys", keys().len());
}

#[test]
fn every_key_has_two_dialects_and_one_entry_per_dialect() {
    for k in keys() {
        // Mitcad's own keys (mitcad#705) have none.
        if MITCAD_OWN.contains(&k.key) {
            assert!(k.dialects.is_empty(), "{} is Mitcad's own", k.key);
            continue;
        }
        let mut bits = 0u8;
        for d in k.dialects {
            assert_eq!(bits & d.dialects, 0, "{}: a dialect twice", k.key);
            bits |= d.dialects;
        }
        assert!(
            bits.count_ones() >= 2,
            "{} has {} dialect(s)",
            k.key,
            bits.count_ones()
        );
    }
}

#[test]
fn choices_name_only_the_keys_values() {
    for k in keys() {
        for d in k.dialects {
            let pairs: &[(&str, &str)] = match d.form {
                Form::Choices(pairs) => pairs,
                Form::Switch { choices, off, .. } => {
                    if let Kind::Choice(_) = k.kind {
                        assert!(k.kind.has_choice(off), "{}: no {off}", k.key);
                    }
                    choices
                }
                _ => &[],
            };
            for (mitcad, _) in pairs {
                match k.kind {
                    Kind::Choice(_) => {
                        assert!(k.kind.has_choice(mitcad), "{}: no {mitcad}", k.key)
                    }
                    Kind::Boolean => assert!(["true", "false"].contains(mitcad), "{}", k.key),
                    _ => panic!("{}: choices of a {:?}", k.key, k.kind),
                }
            }
        }
        if let Kind::Choice(choices) = k.kind {
            let values: HashSet<_> = choices.iter().map(|(value, _)| value).collect();
            assert_eq!(values.len(), choices.len(), "{}", k.key);
            assert!(!choices.is_empty(), "{}", k.key);
        }
    }
}

#[test]
fn no_key_carries_a_vendors_name() {
    for k in keys() {
        for word in ["bambu", "bbl", "prusa", "orca", "qidi", "cura", "supertack"] {
            assert!(!k.key.contains(word), "{}", k.key);
        }
    }
}

#[test]
fn the_three_layers_have_their_keys() {
    for name in [
        "bed_shape",
        "bed_exclude_area",
        "max_print_height",
        "nozzle_diameter",
        "filament_slots",
        "printer_model",
        "printer_settings_id",
    ] {
        assert_eq!(def(name).set, PrintSet::Printer, "{name}");
    }
    assert!(def("printer_settings_id").per_dialect);
    assert_eq!(def("fill_density").set, PrintSet::Process);
    assert_eq!(def("temperature").set, PrintSet::Filament);
    assert_eq!(PrintSet::Filament.property_set(), "Print Filament");
    let counts = counts();
    assert_eq!(counts.len(), 3);
    assert!(counts.iter().all(|(_, n, _)| *n > 0));
}

#[test]
fn percentages_convert_between_dialects() {
    // "15%" <-> 15 <-> Cura's 15.
    let density = read_with("fill_density", BambuStudio, "15%", &[]);
    assert_eq!(density, Value::Number(15.0));
    assert_eq!(read_with("fill_density", Cura, "15", &[]), density);
    assert_eq!(
        write_with("fill_density", PrusaSlicer, &density, &[]),
        "15%"
    );
    assert_eq!(write_with("fill_density", Cura, &density, &[]), "15");
    assert_eq!(write_with("fill_density", OrcaSlicer, &density, &[]), "15%");
    assert_eq!(
        written_key("fill_density", BambuStudio),
        "sparse_infill_density"
    );
    assert_eq!(written_key("fill_density", Cura), "infill_sparse_density");
}

#[test]
fn walls_are_one_key_by_meaning() {
    assert_eq!(written_key("perimeters", BambuStudio), "wall_loops");
    assert_eq!(written_key("perimeters", PrusaSlicer), "perimeters");
    assert_eq!(written_key("perimeters", Cura), "wall_line_count");
    let walls = read_with("perimeters", OrcaSlicer, "3", &[]);
    assert_eq!(walls, Value::Integer(3));
    assert_eq!(write_with("perimeters", Cura, &walls, &[]), "3");
    // A whole number only.
    assert!(read_text(def("perimeters"), PrusaSlicer, "2.5", &no_context).is_err());
}

#[test]
fn a_ratio_is_a_percentage_in_cura() {
    let flow = read_with("extrusion_multiplier", BambuStudio, "0.95", &[]);
    assert_eq!(flow, Value::Number(0.95));
    assert_eq!(write_with("extrusion_multiplier", Cura, &flow, &[]), "95");
    assert_eq!(
        read_with("extrusion_multiplier", Cura, "95", &[]),
        Value::Number(0.95)
    );
    assert_eq!(
        written_key("extrusion_multiplier", QidiStudio),
        "filament_flow_ratio"
    );
}

#[test]
fn yes_and_no_are_written_as_each_dialect_does() {
    let yes = read_with("support_material", Cura, "True", &[]);
    assert_eq!(yes, Value::Bool(true));
    assert_eq!(write_with("support_material", BambuStudio, &yes, &[]), "1");
    assert_eq!(
        write_with("support_material", Cura, &Value::Bool(false), &[]),
        "False"
    );
    assert_eq!(
        read_with("support_material", PrusaSlicer, "0", &[]),
        Value::Bool(false)
    );
}

#[test]
fn choices_are_mapped_by_meaning() {
    // Rectilinear infill: Bambu Studio's zig-zag, OrcaSlicer's and
    // PrusaSlicer's rectilinear, Cura's lines.
    let rectilinear = read_with("fill_pattern", BambuStudio, "zig-zag", &[]);
    assert_eq!(rectilinear, Value::Text("rectilinear".into()));
    assert_eq!(
        read_with("fill_pattern", OrcaSlicer, "rectilinear", &[]),
        rectilinear
    );
    assert_eq!(
        write_with("fill_pattern", PrusaSlicer, &rectilinear, &[]),
        "rectilinear"
    );
    assert_eq!(write_with("fill_pattern", Cura, &rectilinear, &[]), "lines");
    assert_eq!(
        write_with("fill_pattern", QidiStudio, &rectilinear, &[]),
        "zig-zag"
    );
    // Stars are the Bambu family's tri-hexagon.
    let stars = read_with("fill_pattern", PrusaSlicer, "stars", &[]);
    assert_eq!(
        write_with("fill_pattern", BambuStudio, &stars, &[]),
        "tri-hexagon"
    );
    // A seam at the rear is the Bambu family's back.
    let rear = read_with("seam_position", BambuStudio, "back", &[]);
    assert_eq!(rear, Value::Text("rear".into()));
    assert_eq!(write_with("seam_position", PrusaSlicer, &rear, &[]), "rear");
    assert_eq!(write_with("seam_position", Cura, &rear, &[]), "back");
    // A choice a dialect lacks is refused, not guessed.
    let aligned = Value::Text("aligned".into());
    assert!(write(def("seam_position"), Cura, &aligned, &no_context).is_err());
    assert!(read_text(def("fill_pattern"), BambuStudio, "spaghetti", &no_context).is_err());
}

#[test]
fn a_yes_or_no_may_be_a_choice_elsewhere() {
    let by_object = read_with("complete_objects", BambuStudio, "by object", &[]);
    assert_eq!(by_object, Value::Bool(true));
    assert_eq!(
        write_with("complete_objects", PrusaSlicer, &by_object, &[]),
        "1"
    );
    assert_eq!(
        write_with("complete_objects", Cura, &by_object, &[]),
        "one_at_a_time"
    );
    let order = read_with("wall_sequence", PrusaSlicer, "1", &[]);
    assert_eq!(order, Value::Text("outer_inner".into()));
    assert_eq!(
        write_with("wall_sequence", BambuStudio, &order, &[]),
        "outer wall/inner wall"
    );
    assert_eq!(write_with("wall_sequence", Cura, &order, &[]), "outside_in");
}

#[test]
fn overhang_thresholds_take_the_slicers_values() {
    // mitcad#654: Bambu Studio and QIDI Studio write 95% (bridges) for the
    // overhangs slowed for cooling, as for the overhang fan's threshold.
    for dialect in [BambuStudio, QidiStudio] {
        let bridges = read_with(
            "overhang_threshold_participating_cooling",
            dialect,
            "95%",
            &[],
        );
        assert_eq!(bridges, Value::Text("95".into()));
        assert_eq!(
            write_with(
                "overhang_threshold_participating_cooling",
                dialect,
                &bridges,
                &[]
            ),
            "95%"
        );
        let fan = read_with("overhang_fan_threshold", dialect, "95%", &[]);
        assert_eq!(fan, Value::Text("95".into()));
        assert_eq!(
            write_with("overhang_fan_threshold", dialect, &fan, &[]),
            "95%"
        );
    }
    // No slicer takes 100 %: refused both ways, not guessed.
    let cooling = def("overhang_threshold_participating_cooling");
    assert!(read_text(cooling, BambuStudio, "100%", &no_context).is_err());
    assert!(
        write(
            cooling,
            BambuStudio,
            &Value::Text("100".into()),
            &no_context
        )
        .is_err()
    );
    assert!(
        read_text(
            def("overhang_fan_threshold"),
            OrcaSlicer,
            "100%",
            &no_context
        )
        .is_err()
    );
}

#[test]
fn percentages_of_other_values_keep_or_resolve_their_basis() {
    let context = [
        ("layer_height", Value::Number(0.2)),
        ("nozzle_diameter", Value::Number(0.4)),
        ("perimeter_speed", Value::Number(60.0)),
    ];
    // PrusaSlicer's line widths are of the layer height, Mitcad's of the
    // nozzle diameter: resolved to mm.
    let width = read_with("extrusion_width", PrusaSlicer, "225%", &context);
    let Value::Number(mm) = width else {
        panic!("{width:?}")
    };
    assert!((mm - 0.45).abs() < 1e-12);
    assert_eq!(
        write_with("extrusion_width", OrcaSlicer, &width, &context),
        "0.45"
    );
    // OrcaSlicer's are of the nozzle diameter as Mitcad's: kept.
    let width = read_with("extrusion_width", OrcaSlicer, "105%", &context);
    assert_eq!(width, Value::Percent(105.0));
    assert_eq!(
        write_with("extrusion_width", OrcaSlicer, &width, &context),
        "105%"
    );
    assert_eq!(
        write_with("extrusion_width", BambuStudio, &width, &context),
        "0.42"
    );
    assert_eq!(
        write_with("extrusion_width", PrusaSlicer, &width, &context),
        "0.42"
    );
    assert_eq!(
        write_with("extrusion_width", Cura, &width, &context),
        "0.42"
    );
    // The first layer's height as a percentage of the layer height.
    let first = read_with("first_layer_height", PrusaSlicer, "150%", &context);
    assert_eq!(first, Value::Percent(150.0));
    assert_eq!(
        write_with("first_layer_height", BambuStudio, &first, &context),
        "0.3"
    );
    // An outer wall speed of the inner wall's.
    let outer = read_with("external_perimeter_speed", PrusaSlicer, "50%", &context);
    assert_eq!(
        write_with("external_perimeter_speed", Cura, &outer, &context),
        "30"
    );
    // The Bambu family writes a seam gap as a percentage of the nozzle.
    let gap = read_with("seam_gap_distance", BambuStudio, "15%", &context);
    assert_eq!(gap, Value::Percent(15.0));
    assert_eq!(
        write_with("seam_gap_distance", PrusaSlicer, &gap, &context),
        "15%"
    );
    let gap_mm = Value::Number(0.1);
    assert_eq!(
        write_with("seam_gap_distance", QidiStudio, &gap_mm, &context),
        "25%"
    );
    // Without the basis it cannot convert.
    assert!(write(def("extrusion_width"), BambuStudio, &width, &no_context).is_err());
    assert!(read_text(def("extrusion_width"), PrusaSlicer, "225%", &no_context).is_err());
}

#[test]
fn linear_conversions_go_both_ways() {
    // Supports below an overhang angle measured from the bed, Cura's from
    // the vertical.
    let threshold = read_with("support_material_threshold", PrusaSlicer, "30", &[]);
    assert_eq!(
        write_with("support_material_threshold", Cura, &threshold, &[]),
        "60"
    );
    assert_eq!(
        read_with("support_material_threshold", Cura, "60", &[]),
        Value::Integer(30)
    );
    // Filaments from 1, Cura's extruders from 0 (-1: the default).
    assert_eq!(
        write_with("perimeter_extruder", Cura, &Value::Integer(2), &[]),
        "1"
    );
    assert_eq!(
        read_with("perimeter_extruder", Cura, "-1", &[]),
        Value::Integer(0)
    );
    // Elephant foot compensation is Cura's negative first layer expansion.
    assert_eq!(
        write_with("elefant_foot_compensation", Cura, &Value::Number(0.2), &[]),
        "-0.2"
    );
}

#[test]
fn integers_of_a_dialect_round_mitcad_numbers() {
    let hot = Value::Number(215.4);
    assert_eq!(write_with("temperature", BambuStudio, &hot, &[]), "215");
    assert_eq!(write_with("temperature", Cura, &hot, &[]), "215.4");
    assert_eq!(read_with("temperature", Cura, "215.4", &[]), hot);
    assert_eq!(written_key("temperature", OrcaSlicer), "nozzle_temperature");
    assert!(def("temperature").dialect(OrcaSlicer).unwrap().list);
}

#[test]
fn switches_use_two_keys() {
    let raw = |pairs: &'static [(&'static str, &'static str)]| {
        move |k: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| (*v).to_owned())
        }
    };
    let ironing = def("ironing_type");
    let off = read(ironing, PrusaSlicer, &raw(&[("ironing", "0")]), &no_context)
        .unwrap()
        .unwrap();
    assert_eq!(off, Value::Text("no_ironing".into()));
    let topmost = read(
        ironing,
        PrusaSlicer,
        &raw(&[("ironing", "1"), ("ironing_type", "topmost")]),
        &no_context,
    )
    .unwrap()
    .unwrap();
    assert_eq!(topmost, Value::Text("topmost".into()));
    assert_eq!(
        write(ironing, Cura, &topmost, &no_context).unwrap(),
        vec![
            ("ironing_enabled", "True".to_owned()),
            ("ironing_only_highest_layer", "True".to_owned())
        ]
    );
    assert_eq!(
        write(ironing, BambuStudio, &off, &no_context).unwrap(),
        vec![("ironing_type", "no ironing".to_owned())]
    );
    assert_eq!(
        write(
            ironing,
            PrusaSlicer,
            &Value::Text("no_ironing".into()),
            &no_context
        )
        .unwrap(),
        vec![("ironing", "0".to_owned())]
    );
    assert_eq!(
        by_dialect_key(Cura, "ironing_enabled")
            .iter()
            .map(|k| k.key)
            .collect::<Vec<_>>(),
        vec!["ironing_type"]
    );
}

#[test]
fn the_bed_is_an_outline_or_cura_sizes() {
    let bed = def("bed_shape");
    let outline = read_text(bed, BambuStudio, "0x0,256x0,256x256,0x256", &no_context)
        .unwrap()
        .unwrap();
    assert_eq!(
        outline,
        Value::Points(vec![[0.0, 0.0], [256.0, 0.0], [256.0, 256.0], [0.0, 256.0]])
    );
    assert_eq!(
        write(bed, PrusaSlicer, &outline, &no_context).unwrap(),
        vec![("bed_shape", "0x0,256x0,256x256,0x256".to_owned())]
    );
    let cura = write(bed, Cura, &outline, &no_context).unwrap();
    assert_eq!(
        cura,
        vec![
            ("machine_width", "256".to_owned()),
            ("machine_depth", "256".to_owned()),
            ("machine_center_is_zero", "False".to_owned()),
            ("machine_shape", "rectangular".to_owned()),
        ]
    );
    let values: HashMap<&str, String> = cura.into_iter().collect();
    let back = read(bed, Cura, &|k| values.get(k).cloned(), &no_context)
        .unwrap()
        .unwrap();
    assert_eq!(back, outline);
    // A centred bed.
    let centred = Value::Points(vec![
        [-100.0, -100.0],
        [100.0, -100.0],
        [100.0, 100.0],
        [-100.0, 100.0],
    ]);
    let cura = write(bed, Cura, &centred, &no_context).unwrap();
    assert_eq!(cura[2], ("machine_center_is_zero", "True".to_owned()));
    // Filament slots: PrusaSlicer's extruders by its nozzle list.
    let slots = read_text(
        def("filament_slots"),
        PrusaSlicer,
        "0.4,0.4,0.4",
        &no_context,
    )
    .unwrap()
    .unwrap();
    assert_eq!(slots, Value::Integer(3));
}

#[test]
fn unset_and_missing_values_read_as_none() {
    let length = def("filament_retract_length");
    assert_eq!(
        read_text(length, BambuStudio, "nil", &no_context).unwrap(),
        None
    );
    assert_eq!(
        read_text(length, PrusaSlicer, "", &no_context).unwrap(),
        None
    );
    assert_eq!(
        read(length, Cura, &|_| None, &no_context).unwrap(),
        None,
        "no text"
    );
    // A dialect without a counterpart writes nothing.
    assert!(
        write(
            def("fill_multiline"),
            PrusaSlicer,
            &Value::Integer(2),
            &no_context
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn values_have_a_json_form() {
    let width = def("extrusion_width");
    assert_eq!(
        Value::from_json(width, &json!("110%")).unwrap(),
        Value::Percent(110.0)
    );
    assert_eq!(
        Value::from_json(width, &json!(0.45)).unwrap(),
        Value::Number(0.45)
    );
    assert_eq!(Value::Percent(110.0).to_json(), json!("110%"));
    assert_eq!(
        Value::from_json(def("filament_colour"), &json!("#ff8800")).unwrap(),
        Value::Text("#FF8800".into())
    );
    assert!(Value::from_json(def("filament_colour"), &json!("orange")).is_err());
    assert!(Value::from_json(def("fill_pattern"), &json!("spaghetti")).is_err());
    assert_eq!(
        Value::from_json(def("perimeters"), &json!("3")).unwrap(),
        Value::Integer(3)
    );
    assert!(Value::from_json(def("spiral_vase"), &json!(3)).is_err());
}

#[test]
fn prusaslicer_lists_split_and_join() {
    assert_eq!(split_list("215,220"), vec!["215", "220"]);
    assert_eq!(split_list("\"PLA\";\"PETG\""), vec!["PLA", "PETG"]);
    assert_eq!(split_list("PLA;PETG"), vec!["PLA", "PETG"]);
    assert_eq!(
        split_list("#FF8000;\"#0000FF\""),
        vec!["#FF8000", "#0000FF"]
    );
    let text = vec!["a;b".to_owned(), "say \"hi\"".to_owned()];
    assert_eq!(split_list(&join_list(&text, true)), text);
    assert_eq!(join_list(&["1".into(), "2".into()], false), "1,2");
    // Strings quoted only where they need it, as PrusaSlicer writes them;
    // choices (enums) like numbers (mitcad#640).
    assert_eq!(join_list(&["PLA".into(), "PETG".into()], true), "PLA;PETG");
    let names = vec!["Generic PLA".to_owned(), "PETG".to_owned()];
    assert_eq!(join_list(&names, true), "\"Generic PLA\";PETG");
    assert_eq!(split_list(&join_list(&names, true)), names);
    assert_eq!(join_list(&["PLA".into()], true), "PLA");
    assert_eq!(split_list("PLA"), vec!["PLA"]);
    assert_eq!(join_list(&[String::new()], true), "\"\"");
    let comma = vec!["a,b".to_owned()];
    assert_eq!(split_list(&join_list(&comma, true)), comma);
    let logic = vec![
        "uniform_cooling".to_owned(),
        "consistent_surface".to_owned(),
    ];
    assert_eq!(
        join_list(&logic, false),
        "uniform_cooling,consistent_surface"
    );
    assert_eq!(split_list("uniform_cooling,consistent_surface"), logic);
}

/// mitcad#640: Bambu Studio's -1 ("automatic") is out of OrcaSlicer's and
/// PrusaSlicer's range: writing it there fails, so that export leaves it
/// out with a warning; the same value is written to Bambu Studio.
#[test]
fn values_out_of_a_dialects_range_are_not_written() {
    let walls = def("tree_support_wall_count");
    let auto = Value::Integer(-1);
    assert_eq!(
        write_with("tree_support_wall_count", BambuStudio, &auto, &[]),
        "-1"
    );
    assert_eq!(
        write_with("tree_support_wall_count", QidiStudio, &auto, &[]),
        "-1"
    );
    let refused = write(walls, OrcaSlicer, &auto, &no_context).unwrap_err();
    assert!(refused.0.contains("from 0 to 2"), "{refused}");
    assert_eq!(
        write_with(
            "tree_support_wall_count",
            OrcaSlicer,
            &Value::Integer(2),
            &[]
        ),
        "2"
    );
    assert!(write(walls, OrcaSlicer, &Value::Integer(3), &no_context).is_err());
    let expansion = def("raft_first_layer_expansion");
    let auto = Value::Number(-1.0);
    assert_eq!(
        write_with("raft_first_layer_expansion", BambuStudio, &auto, &[]),
        "-1"
    );
    for dialect in [OrcaSlicer, PrusaSlicer] {
        assert!(write(expansion, dialect, &auto, &no_context).is_err());
        assert_eq!(
            write_with(
                "raft_first_layer_expansion",
                dialect,
                &Value::Number(2.0),
                &[]
            ),
            "2"
        );
    }
    // PrusaSlicer's extruders count from 1; a 0 is left out (#640).
    for key in [
        "perimeter_extruder",
        "infill_extruder",
        "solid_infill_extruder",
    ] {
        assert!(write(def(key), PrusaSlicer, &Value::Integer(0), &no_context).is_err());
        assert_eq!(write_with(key, PrusaSlicer, &Value::Integer(2), &[]), "2");
    }
    // The same dialect key for every dialect that has the setting.
    assert_eq!(
        written_key("raft_first_layer_expansion", OrcaSlicer),
        "raft_first_layer_expansion"
    );
    assert!(
        write(
            def("wipe_tower_brim_width"),
            PrusaSlicer,
            &auto,
            &no_context
        )
        .is_err()
    );
    assert_eq!(
        write_with("wipe_tower_brim_width", OrcaSlicer, &auto, &[]),
        "-1"
    );
    // A range applies to plain numbers only.
    let range = def("tree_support_wall_count").dialect(OrcaSlicer).unwrap();
    assert_eq!(range.range, Some((0.0, 2.0)));
    assert!(range.in_range("nil") && range.in_range("50%"));
}

#[test]
fn numbers_are_written_without_noise() {
    assert_eq!(number_text(0.95 * 100.0), "95");
    assert_eq!(number_text(0.1 + 0.2), "0.3");
    assert_eq!(number_text(-0.0), "0");
    assert_eq!(number_text(215.0), "215");
}

#[test]
fn dialects_have_ids_and_names() {
    for d in Dialect::ALL {
        assert_eq!(Dialect::from_id(d.id()), Some(d));
    }
    assert!(QidiStudio.bambu_family() && !Cura.bambu_family());
    assert_eq!(BambuStudio.to_string(), "Bambu Studio");
}

#[test]
fn every_key_has_a_group_of_its_sets_category() {
    for k in keys() {
        assert_eq!(k.category().set(), k.set, "{}", k.key);
        assert_eq!(k.category(), k.group.category(), "{}", k.key);
    }
    // A few places of the map.
    assert_eq!(def("layer_height").group, Group::LayerHeight);
    assert_eq!(def("layer_height").category(), Category::Quality);
    assert!(!def("layer_height").advanced);
    assert_eq!(def("perimeters").group, Group::Walls);
    assert_eq!(def("travel_speed").group, Group::TravelSpeed);
    assert_eq!(def("avoid_crossing_perimeters").group, Group::Travel);
    assert!(def("avoid_crossing_perimeters").advanced);
    assert_eq!(def("bed_shape").category(), Category::BasicInformation);
    assert_eq!(def("temperature").group, Group::NozzleTemperature);
    assert!(def("printer_settings_id").advanced);
}

#[test]
fn no_group_is_empty_and_the_rows_are_in_display_order() {
    for group in Group::ALL {
        assert!(
            keys().iter().any(|k| k.group == group),
            "{} has no keys",
            group.id()
        );
    }
    // The table reads as the outline: by set, then group, common keys
    // first in each group.
    let place = |k: &KeyDef| {
        let set = PrintSet::ALL.iter().position(|s| *s == k.set).unwrap();
        (set, k.group, k.advanced)
    };
    for pair in keys().windows(2) {
        assert!(
            place(&pair[0]) <= place(&pair[1]),
            "{} comes before {}",
            pair[0].key,
            pair[1].key
        );
    }
    // The common keys of the proposal's map: printer 5, process 60,
    // filament 25; and the purge multiplier and a filament's two purge
    // volumes (mitcad#705).
    let common = |set: PrintSet| keys_in(set).filter(|k| !k.advanced).count();
    assert_eq!(common(PrintSet::Printer), 5);
    assert_eq!(common(PrintSet::Process), 61);
    assert_eq!(common(PrintSet::Filament), 27);
    assert_eq!(def("purge_into").group, Group::Purge);
    assert_eq!(def("purge_out").category(), Category::Filament);
    assert_eq!(def("flush_multiplier").category(), Category::Others);
}

#[test]
fn every_choice_has_a_name() {
    for k in keys() {
        let Kind::Choice(choices) = k.kind else {
            continue;
        };
        let names: HashSet<_> = choices.iter().map(|(_, name)| name).collect();
        assert_eq!(names.len(), choices.len(), "{}: a name twice", k.key);
        for (value, name) in choices {
            assert!(!name.trim().is_empty(), "{}: {value} has no name", k.key);
            assert_ne!(name, value, "{}: {value} is named by its value", k.key);
        }
    }
    let Kind::Choice(choices) = def("wall_sequence").kind else {
        panic!("wall_sequence is a choice");
    };
    assert!(choices.contains(&("inner_outer", "Inner, then outer")));
    assert!(def("fill_pattern").kind.has_choice("3dhoneycomb"));
    assert!(!def("fill_pattern").kind.has_choice("3D honeycomb"));
}

#[test]
fn the_layout_follows_the_enums() {
    let mut categories = Vec::new();
    let mut groups = Vec::new();
    for set in PrintSet::ALL {
        for (category, in_category) in layout(set) {
            assert_eq!(category.set(), set, "{}", category.id());
            assert!(!in_category.is_empty(), "{}", category.id());
            categories.push(category);
            for group in in_category {
                assert_eq!(group.category(), category, "{}", group.id());
                groups.push(group);
            }
        }
    }
    assert_eq!(categories, Category::ALL.to_vec());
    assert_eq!(groups, Group::ALL.to_vec());
    // The ids are unique, the titles not empty.
    let ids: HashSet<_> = Group::ALL.iter().map(|g| g.id()).collect();
    assert_eq!(ids.len(), Group::ALL.len());
    let ids: HashSet<_> = Category::ALL.iter().map(|c| c.id()).collect();
    assert_eq!(ids.len(), Category::ALL.len());
    assert!(Group::ALL.iter().all(|g| !g.title().is_empty()));
    assert_eq!(Group::TravelSpeed.title(), "Travel");
    assert_eq!(Category::SettingOverrides.title(), "Retraction");
}

/// mitcad#746: every numeric key has the range its slicers take, the
/// widest of them; a dialect's narrower range is its own. Every percentage
/// and every angle is listed in the limits, none falls back to the default.
#[test]
fn limits_cover_every_percentage_and_angle() {
    let listed: HashSet<&str> = limits::LIMITS.iter().map(|(k, _, _)| *k).collect();
    assert_eq!(listed.len(), limits::LIMITS.len(), "a key listed twice");
    for (name, min, max) in limits::LIMITS {
        let key = def(name);
        assert!(key.limits().is_some(), "{name} is not a number");
        assert!(min <= max, "{name}: {min} > {max}");
    }
    for key in keys() {
        let Some((min, max)) = key.limits() else {
            assert!(!listed.contains(key.key), "{}", key.key);
            continue;
        };
        let unit = key.unit();
        if matches!(key.kind, Kind::Percent) || unit == Unit::Percent || unit == Unit::Degree {
            assert!(
                listed.contains(key.key),
                "{} has no limits of its own",
                key.key
            );
        }
        // A dialect takes no more than the key.
        for dialect in Dialect::ALL {
            if let Some((low, high)) = key.dialect_limits(dialect) {
                assert!(low >= min && high <= max, "{} in {dialect}", key.key);
            }
        }
    }
}

#[test]
fn limits_keep_what_a_slicer_takes() {
    let inf = f64::INFINITY;
    // Not above 100 %, not negative.
    assert_eq!(def("fill_density").limits(), Some((0.0, 100.0)));
    assert_eq!(def("layer_height").limits(), Some((0.0, inf)));
    assert_eq!(def("perimeters").limits(), Some((0.0, inf)));
    // Purge line spacing and the tower's infill gap go above 100 %.
    let (low, high) = def("wipe_tower_extra_spacing").limits().unwrap();
    assert!(low <= 150.0 && 150.0 <= high);
    let (low, high) = def("prime_tower_infill_gap").limits().unwrap();
    assert!(low <= 150.0 && 150.0 <= high);
    // Bambu Studio's -1 (automatic) is in the key's range, not in
    // OrcaSlicer's or PrusaSlicer's.
    for name in [
        "tree_support_wall_count",
        "raft_first_layer_expansion",
        "wipe_tower_brim_width",
    ] {
        assert_eq!(def(name).limits().unwrap().0, -1.0, "{name}");
        assert_eq!(def(name).dialect_limits(BambuStudio), None, "{name}");
    }
    assert_eq!(
        def("tree_support_wall_count").dialect_limits(OrcaSlicer),
        Some((0.0, 2.0))
    );
    assert_eq!(
        def("raft_first_layer_expansion").dialect_limits(PrusaSlicer),
        Some((0.0, inf))
    );
    // A compensation takes either sign, an angle of a direction too.
    assert_eq!(def("xy_size_compensation").limits(), Some((-inf, inf)));
    assert_eq!(def("fill_angle").limits(), Some((-inf, inf)));
    // Fans run up to 100 %.
    assert_eq!(def("max_fan_speed").limits(), Some((0.0, 100.0)));
    // Not a number: no range.
    assert_eq!(def("seam_position").limits(), None);
    assert_eq!(def("bed_shape").limits(), None);
}
