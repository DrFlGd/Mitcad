// SPDX-License-Identifier: MIT
//! An element's properties as Mitcad's (mitcad#496), with the source
//! `rvt`: its identity (element id, GlobalId, category, class, family,
//! type, level), its instance parameters and its type's parameters, as
//! its records keep them.
//!
//! Each parameter is named and typed by its definition (mitcad#501,
//! [`mitcad_rvt::params`]): a project, shared or family parameter by its
//! definition in the file (its name, and its spec or class: length, area,
//! volume, angle, mass, number, integer, yes or no, text, element), a
//! built-in parameter by the reader's table of the common ones, else
//! `Parameter <id>`. Lengths, areas, volumes, angles and masses come in
//! as those quantities, converted from the file's internal units (feet,
//! square and cubic feet, radians, kilograms) to the model's canonical
//! ones, so they show in the design's units. Numbers of other kinds
//! (counts, currencies, plain numbers) stay numbers; a number whose kind
//! is not known or not converted (electrical, mechanical and structural
//! quantities, built-in parameters not in the table, files before the
//! 2021 release) stays in the internal units and says so: a number with
//! the unit label [`INTERNAL_UNITS`]. A wall's type brings its width (its
//! layers' total) as the built-in `Width`.
//!
//! The import puts the type's parameters on the element's bodies, beside
//! its own: an element comes in as bodies in its level's component, with
//! no component of its type. Were the import to make a component per
//! element type, the type's parameters would go on it.

use mitcad_model::properties::{BaseType, Property, PropertyValue, Source, key_of};
use mitcad_rvt::building::Param;
use mitcad_rvt::categories::OST_WALLS;
use mitcad_rvt::elements::{Element, Model, ParamValue};
use mitcad_rvt::params::{self, ParamKind};

/// The set of an element's identity.
pub const IDENTITY_SET: &str = "Identity";
/// The set of its instance parameters.
pub const INSTANCE_SET: &str = "Instance Parameters";
/// The set of its type's parameters.
pub const TYPE_SET: &str = "Type Parameters";
/// The unit label of a number of an unknown kind, kept in the file's
/// internal units.
pub const INTERNAL_UNITS: &str = "internal units";
/// The built-in parameter of a wall type's width.
const WALL_WIDTH: i64 = -1_001_000;

/// A set's name and one of its properties.
pub type SetProperty = (String, Property);

fn property(set: &str, key: &str, base: BaseType, value: PropertyValue) -> SetProperty {
    (
        set.to_owned(),
        Property::new(&key_of(key), base, value, Source::Rvt),
    )
}

fn text(set: &str, key: &str, value: &str) -> SetProperty {
    property(
        set,
        key,
        BaseType::Text,
        PropertyValue::Text(value.to_owned()),
    )
}

fn integer(set: &str, key: &str, value: i64) -> SetProperty {
    property(set, key, BaseType::Integer, PropertyValue::Integer(value))
}

fn boolean(set: &str, key: &str, value: bool) -> SetProperty {
    property(set, key, BaseType::Boolean, PropertyValue::Boolean(value))
}

/// The standard type of a kind of quantity.
fn quantity(kind: ParamKind) -> Option<BaseType> {
    Some(match kind {
        ParamKind::Length => BaseType::Length,
        ParamKind::Area => BaseType::Area,
        ParamKind::Volume => BaseType::Volume,
        ParamKind::Angle => BaseType::Angle,
        ParamKind::Mass => BaseType::Mass,
        _ => return None,
    })
}

/// A number of a kind: a quantity in canonical units, a plain number, an
/// integer or a yes or no, else a number in the internal units, labelled.
fn number(set: &str, key: &str, kind: ParamKind, value: f64) -> SetProperty {
    if let (Some(base), Some(factor)) = (quantity(kind), kind.to_canonical()) {
        return property(set, key, base, PropertyValue::Number(value * factor));
    }
    match kind {
        ParamKind::Number => property(set, key, BaseType::Number, PropertyValue::Number(value)),
        ParamKind::Integer if value.fract() == 0.0 => integer(set, key, value as i64),
        ParamKind::Boolean => boolean(set, key, value != 0.0),
        _ => {
            let (set, p) = property(set, key, BaseType::Number, PropertyValue::Number(value));
            (set, p.with_unit(Some(INTERNAL_UNITS.to_owned())))
        }
    }
}

/// A parameter's name and kind: its definition's (a project, shared or
/// family parameter), the built-in table's, else `Parameter <id>` of an
/// unknown kind.
pub fn parameter(model: &Model, id: i64) -> (String, ParamKind) {
    if let Some(def) = model.building.param_defs.get(&id) {
        let name = if def.name.trim().is_empty() {
            format!("Parameter {id}")
        } else {
            def.name.clone()
        };
        return (name, def.kind);
    }
    match params::built_in(id) {
        Some((name, kind)) => (name.to_owned(), kind),
        None => (format!("Parameter {id}"), ParamKind::Unknown),
    }
}

/// An instance parameter's value as a property of its kind.
fn instance_value(key: &str, kind: ParamKind, value: &ParamValue) -> SetProperty {
    match value {
        ParamValue::Text(t) => text(INSTANCE_SET, key, t),
        ParamValue::Number(n) => number(INSTANCE_SET, key, kind, *n),
        ParamValue::Integer(i) if kind == ParamKind::Boolean => boolean(INSTANCE_SET, key, *i != 0),
        ParamValue::Integer(i) => integer(INSTANCE_SET, key, *i),
        ParamValue::Element(id) => integer(INSTANCE_SET, key, *id),
    }
}

/// A type parameter's value as a property of its kind: the field of its
/// record its kind keeps it in (None for an element parameter naming no
/// element). Of an unknown kind, its text, element or integer when it has
/// one, else its number.
fn type_value(key: &str, kind: ParamKind, p: &Param) -> Option<SetProperty> {
    let element = p.element.filter(|id| *id != -1);
    Some(match kind {
        ParamKind::Text => text(TYPE_SET, key, &p.text),
        ParamKind::Element => integer(TYPE_SET, key, element?),
        ParamKind::Boolean => boolean(TYPE_SET, key, p.int != 0),
        ParamKind::Integer => integer(TYPE_SET, key, p.int),
        ParamKind::Unknown if !p.text.is_empty() => text(TYPE_SET, key, &p.text),
        ParamKind::Unknown if element.is_some() => integer(TYPE_SET, key, element?),
        ParamKind::Unknown if p.value == 0.0 && p.int != 0 => integer(TYPE_SET, key, p.int),
        _ => number(TYPE_SET, key, kind, p.value),
    })
}

/// The properties of an element with its GlobalId, in the level (its
/// component's name) it comes in on.
pub fn element_properties(
    model: &Model,
    e: &Element,
    global_id: Option<&str>,
    level: Option<&str>,
) -> Vec<SetProperty> {
    let mut out = vec![integer(IDENTITY_SET, "ElementId", e.id)];
    if let Some(global_id) = global_id {
        out.push(text(IDENTITY_SET, "GlobalId", global_id));
    }
    match (e.category_name, e.category) {
        (Some(name), _) => out.push(text(IDENTITY_SET, "Category", name)),
        (None, Some(id)) => out.push(text(IDENTITY_SET, "Category", &format!("category {id}"))),
        _ => {}
    }
    if let Some(class) = &e.class {
        out.push(text(IDENTITY_SET, "Class", class));
    }
    if let Some(family) = e.family_name.as_deref().filter(|f| !f.trim().is_empty()) {
        out.push(text(IDENTITY_SET, "Family", family));
    }
    if let Some(name) = e.type_name.as_deref().filter(|t| !t.trim().is_empty()) {
        out.push(text(IDENTITY_SET, "Type", name));
    }
    if let Some(type_id) = e.type_id {
        out.push(integer(IDENTITY_SET, "TypeId", type_id));
    }
    if let Some(level) = level.filter(|l| !l.trim().is_empty()) {
        out.push(text(IDENTITY_SET, "Level", level));
    }
    for (id, value) in &e.params {
        let (key, kind) = parameter(model, *id);
        out.push(instance_value(&key, kind, value));
    }
    // The type's parameters: a family type's (FamilySymbol).
    let symbol = e
        .type_id
        .and_then(|t| model.building.symbols.get(&t))
        .or_else(|| e.symbol.and_then(|s| model.building.symbols.get(&s)));
    if let Some(symbol) = symbol {
        for p in &symbol.params {
            let (key, kind) = parameter(model, p.id);
            out.extend(type_value(&key, kind, p));
        }
    }
    // A wall type's width: its layers' total (the type keeps no parameter
    // of it).
    if e.category == Some(OST_WALLS)
        && let Some(width) = e.type_id.and_then(|t| model.building.width(t))
    {
        let (key, kind) = parameter(model, WALL_WIDTH);
        out.push(number(TYPE_SET, &key, kind, width));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mitcad_rvt::building::{Layer, Symbol};
    use mitcad_rvt::params::ParamDef;

    fn def(name: &str, kind: ParamKind) -> ParamDef {
        ParamDef {
            name: name.into(),
            kind,
            spec: None,
        }
    }

    fn get<'a>(props: &'a [SetProperty], set: &str, key: &str) -> &'a Property {
        props
            .iter()
            .find(|(s, p)| s == set && p.key == key)
            .map(|(_, p)| p)
            .unwrap_or_else(|| panic!("no {set} / {key} in {props:?}"))
    }

    fn close(p: &Property, ty: &str, value: f64) {
        assert_eq!(p.ty, ty, "{p:?}");
        let PropertyValue::Number(n) = p.value else {
            panic!("{p:?}");
        };
        assert!((n - value).abs() < 1e-6, "{p:?}: {value}");
    }

    #[test]
    fn parameters_come_in_with_their_kinds_in_canonical_units() {
        let mut model = Model::default();
        let b = &mut model.building;
        b.param_defs
            .insert(10, def("Trim Width", ParamKind::Length));
        b.param_defs.insert(11, def("A", ParamKind::Area));
        b.param_defs.insert(12, def("Rotation", ParamKind::Angle));
        b.param_defs.insert(13, def("Casement", ParamKind::Boolean));
        b.param_defs.insert(14, def("Note", ParamKind::Text));
        b.param_defs
            .insert(15, def("Door Material", ParamKind::Element));
        b.param_defs.insert(16, def("Load", ParamKind::Unknown));
        b.param_defs.insert(17, def("Count", ParamKind::Integer));
        b.param_defs.insert(18, def("Volume", ParamKind::Volume));
        let param = |id, value, text: &str, element, int| Param {
            id,
            value,
            text: text.into(),
            element,
            int,
        };
        b.symbols.insert(
            100,
            Symbol {
                id: 100,
                params: vec![
                    // A door type's height and width (built-in), 8 by 3 ft.
                    param(-1_001_300, 8.0, "", None, 0),
                    param(-1_001_301, 3.0, "", None, 0),
                    param(10, 0.25, "", None, 0),
                    param(11, 1.0, "", None, 0),
                    param(12, std::f64::consts::FRAC_PI_4, "", None, 0),
                    param(13, 0.0, "", None, 1),
                    param(14, 0.0, "", None, 0),
                    param(15, 0.0, "", Some(-1), 0),
                    param(16, 2.5, "", None, 0),
                    param(17, 0.0, "", None, 4),
                    param(18, 1.0, "", None, 0),
                    // A built-in text parameter, empty, and one of a kind
                    // not known.
                    param(-1_001_206, 0.0, "", None, 0),
                    param(-1_999_999, 0.656_167_979, "", None, 0),
                ],
                ..Symbol::default()
            },
        );
        b.structures.insert(
            200,
            vec![Layer {
                width: 0.656_167_979,
                material: None,
                function: 0,
            }],
        );
        let door = Element {
            id: 1,
            type_id: Some(100),
            params: vec![
                (-1_001_203, ParamValue::Text("D1".into())),
                (12, ParamValue::Number(std::f64::consts::FRAC_PI_2)),
                (13, ParamValue::Integer(0)),
                (17, ParamValue::Integer(3)),
                (-1_999_998, ParamValue::Number(1.5)),
            ],
            ..Element::default()
        };
        let props = element_properties(&model, &door, None, None);
        close(get(&props, TYPE_SET, "Height"), "length", 2438.4);
        close(get(&props, TYPE_SET, "Width"), "length", 914.4);
        close(get(&props, TYPE_SET, "Trim Width"), "length", 76.2);
        close(get(&props, TYPE_SET, "A"), "area", 92_903.04);
        close(
            get(&props, TYPE_SET, "Rotation"),
            "angle",
            std::f64::consts::FRAC_PI_4,
        );
        close(get(&props, TYPE_SET, "Volume"), "volume", 28_316_846.592);
        let casement = get(&props, TYPE_SET, "Casement");
        assert_eq!(casement.value, PropertyValue::Boolean(true));
        assert_eq!(
            get(&props, TYPE_SET, "Note").value,
            PropertyValue::Text(String::new())
        );
        assert_eq!(
            get(&props, TYPE_SET, "Fire Rating").value,
            PropertyValue::Text(String::new())
        );
        assert!(
            !props.iter().any(|(_, p)| p.key == "Door Material"),
            "{props:?}"
        );
        assert_eq!(
            get(&props, TYPE_SET, "Count").value,
            PropertyValue::Integer(4)
        );
        // Of an unknown kind: a number in the internal units, labelled.
        for key in ["Load", "Parameter -1999999"] {
            let p = get(&props, TYPE_SET, key);
            assert_eq!(p.ty, "number");
            assert_eq!(p.unit.as_deref(), Some(INTERNAL_UNITS));
        }
        // The instance's own.
        assert_eq!(
            get(&props, INSTANCE_SET, "Mark").value,
            PropertyValue::Text("D1".into())
        );
        close(
            get(&props, INSTANCE_SET, "Rotation"),
            "angle",
            std::f64::consts::FRAC_PI_2,
        );
        assert_eq!(
            get(&props, INSTANCE_SET, "Casement").value,
            PropertyValue::Boolean(false)
        );
        assert_eq!(
            get(&props, INSTANCE_SET, "Count").value,
            PropertyValue::Integer(3)
        );
        let unknown = get(&props, INSTANCE_SET, "Parameter -1999998");
        assert_eq!(unknown.unit.as_deref(), Some(INTERNAL_UNITS));
        assert_eq!(unknown.value, PropertyValue::Number(1.5));

        // A wall's type's width: 0.656 ft, 200 mm.
        let wall = Element {
            id: 2,
            category: Some(OST_WALLS),
            type_id: Some(200),
            params: vec![(-1_001_101, ParamValue::Number(10.0))],
            ..Element::default()
        };
        let props = element_properties(&model, &wall, None, None);
        close(get(&props, TYPE_SET, "Width"), "length", 200.0);
        close(get(&props, INSTANCE_SET, "Height"), "length", 3048.0);
    }
}
