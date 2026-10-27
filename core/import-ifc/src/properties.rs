// SPDX-License-Identifier: MIT
//! An IFC element's properties as Mitcad's (mitcad#496): its identity
//! (GlobalId, class, name, object type), its own property sets and
//! element quantities, and its type object's property sets, each value
//! with the source `ifc`. The import puts the element's own sets on its
//! bodies and occurrences, and its type's on the component its type's
//! shape became (the bodies of an element without one take them too).

use mitcad_ifc::IfcFile;
use mitcad_ifc::model::{Model, Product};
use mitcad_ifc::properties::{PropertyValue as IfcValue, property_set};
use mitcad_model::properties::{BaseType, Property, PropertyValue, Source, key_of};

/// The set of an element's identity.
pub const IDENTITY_SET: &str = "Identity";

/// A set's name and one of its properties.
pub type SetProperty = (String, Property);

/// An IFC value as a property of the source `ifc`.
pub fn property(key: &str, value: &IfcValue) -> Property {
    let (base, value, unit) = match value {
        IfcValue::Text(t) => (BaseType::Text, PropertyValue::Text(t.clone()), None),
        IfcValue::Integer(i) => (BaseType::Integer, PropertyValue::Integer(*i), None),
        IfcValue::Number(n, unit) => (BaseType::Number, PropertyValue::Number(*n), unit.clone()),
        IfcValue::Boolean(b) => (BaseType::Boolean, PropertyValue::Boolean(*b), None),
        IfcValue::Length(v) => (BaseType::Length, PropertyValue::Number(*v), None),
        IfcValue::Area(v) => (BaseType::Area, PropertyValue::Number(*v), None),
        IfcValue::Volume(v) => (BaseType::Volume, PropertyValue::Number(*v), None),
        IfcValue::Mass(v) => (BaseType::Mass, PropertyValue::Number(*v), None),
        IfcValue::Angle(v) => (BaseType::Angle, PropertyValue::Number(*v), None),
        IfcValue::Date(d) => (BaseType::Date, PropertyValue::Text(d.clone()), None),
    };
    Property::new(&key_of(key), base, value, Source::Ifc).with_unit(unit)
}

fn text(key: &str, value: &str) -> Property {
    Property::new(
        key,
        BaseType::Text,
        PropertyValue::Text(value.to_owned()),
        Source::Ifc,
    )
}

/// The properties of property sets by id, in their order.
fn sets(file: &IfcFile, model: &Model, ids: &[u64]) -> Vec<SetProperty> {
    let mut out = Vec::new();
    for id in ids {
        let Some(set) = property_set(file, &model.units, *id) else {
            continue;
        };
        let name = if set.name.trim().is_empty() {
            format!("#{id}")
        } else {
            set.name.clone()
        };
        for (key, value) in &set.properties {
            out.push((name.clone(), property(key, value)));
        }
    }
    out
}

/// An element's identity and its own property sets and quantities.
pub fn instance_properties(file: &IfcFile, model: &Model, p: &Product) -> Vec<SetProperty> {
    let mut out = vec![
        (IDENTITY_SET.to_owned(), text("GlobalId", &p.global_id)),
        (IDENTITY_SET.to_owned(), text("Class", &p.kind)),
    ];
    if !p.name.trim().is_empty() {
        out.push((IDENTITY_SET.to_owned(), text("Name", &p.name)));
    }
    if !p.object_type.trim().is_empty() {
        out.push((IDENTITY_SET.to_owned(), text("ObjectType", &p.object_type)));
    }
    out.extend(sets(file, model, &p.property_sets));
    out
}

/// The property sets of an element's type object, with the type's name.
pub fn type_properties(file: &IfcFile, model: &Model, p: &Product) -> Vec<SetProperty> {
    let Some(t) = p.type_object else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(name) = file
        .get(t)
        .and_then(|e| e.arg(2).as_str())
        .filter(|n| !n.trim().is_empty())
    {
        out.push((IDENTITY_SET.to_owned(), text("Type", name)));
    }
    if let Some(ids) = model.type_property_sets.get(&t) {
        out.extend(sets(file, model, ids));
    }
    out
}
