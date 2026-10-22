// SPDX-License-Identifier: MIT
//! Property sets and element quantities (IfcPropertySet with its single
//! and enumerated values, IfcElementQuantity with its quantities) as
//! named values in Mitcad's units: lengths mm, areas mm², volumes mm³,
//! angles radians, masses kg (mitcad#496). Which sets an element has, its
//! own (IfcRelDefinesByProperties) and its type's (HasPropertySets), is in
//! [`crate::model::Product::property_sets`] and
//! [`crate::model::Model::type_property_sets`].

use serde::Serialize;

use crate::units::{Units, unit_value};
use crate::{Entity, IfcFile, Value};

/// A value of a property or quantity.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PropertyValue {
    Text(String),
    Integer(i64),
    /// A plain number with the unit its measure type names, if any.
    Number(f64, Option<String>),
    Boolean(bool),
    /// mm.
    Length(f64),
    /// mm².
    Area(f64),
    /// mm³.
    Volume(f64),
    /// kg.
    Mass(f64),
    /// Radians.
    Angle(f64),
    /// `YYYY-MM-DD`.
    Date(String),
}

/// A property set or an element quantity with its values in order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PropertySet {
    pub id: u64,
    pub name: String,
    /// An element quantity (`Qto_...`), not a property set.
    pub quantities: bool,
    pub properties: Vec<(String, PropertyValue)>,
}

/// Whether an entity is a set this module reads.
pub fn is_property_set(file: &IfcFile, id: u64) -> bool {
    matches!(
        file.kind(id),
        Some("IFCPROPERTYSET") | Some("IFCELEMENTQUANTITY")
    )
}

/// The size of a value's own unit of `kind` (`LENGTHUNIT`) when it gives
/// one, else `default`.
fn own_unit(file: &IfcFile, unit: &Value, kind: &str, default: f64) -> f64 {
    file.deref(unit)
        .and_then(|u| unit_value(file, u, 0))
        .filter(|(k, _, _)| k == kind)
        .map_or(default, |(_, size, _)| size)
}

/// The size of a mass unit in kg (an SI gram with its prefix, or a unit
/// converted from one); kg when there is none or it is not understood.
fn mass_unit(file: &IfcFile, unit: &Value) -> f64 {
    fn size(file: &IfcFile, unit: &Entity, depth: usize) -> Option<f64> {
        if depth > 8 {
            return None;
        }
        if unit.is("IFCSIUNIT") {
            // (Dimensions, UnitType, Prefix, Name)
            if unit.arg(3).as_enum()? != "GRAM" {
                return None;
            }
            let prefix = match unit.arg(2).as_enum() {
                Some("KILO") => 1e3,
                Some("MILLI") => 1e-3,
                Some("MEGA") => 1e6,
                Some(_) => return None,
                None => 1.0,
            };
            return Some(prefix * 1e-3);
        }
        if unit.is("IFCCONVERSIONBASEDUNIT") {
            let factor = file.deref(unit.arg(3))?;
            let value = factor.arg(0).as_f64()?;
            let inner = file.deref(factor.arg(1))?;
            return Some(value * size(file, inner, depth + 1)?);
        }
        None
    }
    file.deref(unit)
        .and_then(|u| size(file, u, 0))
        .unwrap_or(1.0)
}

/// A date as `YYYY-MM-DD` when the text starts with one.
fn date(text: &str) -> Option<String> {
    let head = text.get(..10)?;
    let b = head.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    (digits(0..4) && b[4] == b'-' && digits(5..7) && b[7] == b'-' && digits(8..10))
        .then(|| head.to_owned())
}

/// A typed value (`IFCLABEL('x')`, `IFCLENGTHMEASURE(2.5)`) in Mitcad's
/// units; `unit` the property's own unit (else the project's).
fn value_of(file: &IfcFile, units: &Units, value: &Value, unit: &Value) -> Option<PropertyValue> {
    let (name, inner) = match value {
        Value::Typed(name, inner) => (name.as_str(), &**inner),
        Value::String(text) => return Some(PropertyValue::Text(text.clone())),
        Value::Integer(i) => return Some(PropertyValue::Integer(*i)),
        Value::Real(r) => return Some(PropertyValue::Number(*r, None)),
        _ => return None,
    };
    let number = || inner.as_f64();
    Some(match name {
        "IFCLABEL" | "IFCTEXT" | "IFCIDENTIFIER" | "IFCDESCRIPTIVEMEASURE" => {
            PropertyValue::Text(inner.as_str()?.to_owned())
        }
        "IFCBOOLEAN" | "IFCLOGICAL" => match inner.as_bool() {
            Some(b) => PropertyValue::Boolean(b),
            None => PropertyValue::Text("unknown".to_owned()),
        },
        "IFCINTEGER" | "IFCCOUNTMEASURE" if inner.as_i64().is_some() => {
            PropertyValue::Integer(inner.as_i64()?)
        }
        "IFCLENGTHMEASURE" | "IFCPOSITIVELENGTHMEASURE" | "IFCNONNEGATIVELENGTHMEASURE" => {
            PropertyValue::Length(number()? * own_unit(file, unit, "LENGTHUNIT", units.length))
        }
        "IFCAREAMEASURE" => {
            PropertyValue::Area(number()? * own_unit(file, unit, "AREAUNIT", units.area))
        }
        "IFCVOLUMEMEASURE" => {
            PropertyValue::Volume(number()? * own_unit(file, unit, "VOLUMEUNIT", units.volume))
        }
        "IFCPLANEANGLEMEASURE" | "IFCPOSITIVEPLANEANGLEMEASURE" => {
            PropertyValue::Angle(number()? * own_unit(file, unit, "PLANEANGLEUNIT", units.angle))
        }
        "IFCMASSMEASURE" => PropertyValue::Mass(number()? * mass_unit(file, unit)),
        "IFCDATE" => {
            let text = inner.as_str()?;
            match date(text) {
                Some(date) => PropertyValue::Date(date),
                None => PropertyValue::Text(text.to_owned()),
            }
        }
        _ => match inner {
            Value::String(text) => PropertyValue::Text(text.clone()),
            Value::Integer(i) => PropertyValue::Integer(*i),
            Value::Real(r) => PropertyValue::Number(*r, measure_label(name)),
            Value::Enum(e) => PropertyValue::Text(e.clone()),
            _ => return None,
        },
    })
}

/// A measure type's unit where it is a common one (`IFCTHERMODYNAMIC
/// TEMPERATUREMEASURE`: K); none for ratios and plain reals.
fn measure_label(name: &str) -> Option<String> {
    let label = match name {
        "IFCTHERMALTRANSMITTANCEMEASURE" => "W/(m²·K)",
        "IFCTHERMODYNAMICTEMPERATUREMEASURE" => "K",
        "IFCPOWERMEASURE" => "W",
        "IFCFORCEMEASURE" => "N",
        "IFCPRESSUREMEASURE" => "Pa",
        "IFCTIMEMEASURE" => "s",
        "IFCMASSDENSITYMEASURE" => "kg/m³",
        "IFCPOSITIVERATIOMEASURE" | "IFCRATIOMEASURE" | "IFCNORMALISEDRATIOMEASURE" => {
            return None;
        }
        _ => return None,
    };
    Some(label.to_owned())
}

/// A property set or element quantity by id, with the values Mitcad
/// reads: single values, enumerated values (as text), and quantities of
/// length, area, volume, count, weight and time. Complex, bounded, list
/// and table values are left out.
pub fn property_set(file: &IfcFile, units: &Units, id: u64) -> Option<PropertySet> {
    let set = file.get(id)?;
    // IfcPropertySet(GlobalId, OwnerHistory, Name, Description,
    // HasProperties); IfcElementQuantity(..., Name, Description,
    // MethodOfMeasurement, Quantities).
    let quantities = set.is("IFCELEMENTQUANTITY");
    if !quantities && !set.is("IFCPROPERTYSET") {
        return None;
    }
    let name = set.arg(2).as_str().unwrap_or_default().to_owned();
    let members = if quantities {
        set.arg(5).refs()
    } else {
        set.arg(4).refs()
    };
    let mut properties = Vec::new();
    for member in members {
        let Some(p) = file.get(member) else {
            continue;
        };
        let key = p.arg(0).as_str().unwrap_or_default().to_owned();
        if key.is_empty() {
            continue;
        }
        let value = match &*p.kind {
            // (Name, Description, NominalValue, Unit)
            "IFCPROPERTYSINGLEVALUE" => value_of(file, units, p.arg(2), p.arg(3)),
            // (Name, Description, EnumerationValues, EnumerationReference)
            "IFCPROPERTYENUMERATEDVALUE" => {
                let values: Vec<String> = p
                    .arg(2)
                    .as_list()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .or_else(|| v.as_enum().map(str::to_owned))
                    })
                    .collect();
                (!values.is_empty()).then(|| PropertyValue::Text(values.join(", ")))
            }
            // (Name, Description, Unit, Value[, Formula])
            "IFCQUANTITYLENGTH" => p.arg(3).as_f64().map(|v| {
                PropertyValue::Length(v * own_unit(file, p.arg(2), "LENGTHUNIT", units.length))
            }),
            "IFCQUANTITYAREA" => p
                .arg(3)
                .as_f64()
                .map(|v| PropertyValue::Area(v * own_unit(file, p.arg(2), "AREAUNIT", units.area))),
            "IFCQUANTITYVOLUME" => p
                .arg(3)
                .as_f64()
                .map(|v| PropertyValue::Volume(v * units.volume_of(file, p.arg(2)))),
            "IFCQUANTITYCOUNT" => p.arg(3).as_f64().map(|v| {
                if v.fract() == 0.0 && v.abs() < 9e15 {
                    PropertyValue::Integer(v as i64)
                } else {
                    PropertyValue::Number(v, None)
                }
            }),
            "IFCQUANTITYWEIGHT" => p
                .arg(3)
                .as_f64()
                .map(|v| PropertyValue::Mass(v * mass_unit(file, p.arg(2)))),
            "IFCQUANTITYTIME" => p
                .arg(3)
                .as_f64()
                .map(|v| PropertyValue::Number(v, Some("s".to_owned()))),
            _ => None,
        };
        if let Some(value) = value {
            properties.push((key, value));
        }
    }
    Some(PropertySet {
        id,
        name,
        quantities,
        properties,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Model;

    #[test]
    fn the_buildings_sets() {
        let file = IfcFile::parse(crate::testdata::BUILDING).expect("reads");
        let model = Model::read(&file);
        let wall = &model.products[&100];
        let sets: Vec<PropertySet> = wall
            .property_sets
            .iter()
            .filter_map(|id| property_set(&file, &model.units, *id))
            .collect();
        let common = sets
            .iter()
            .find(|s| s.name == "Pset_WallCommon")
            .expect("the wall's property set");
        assert!(!common.quantities);
        assert_eq!(
            common.properties,
            vec![
                (
                    "FireRating".to_owned(),
                    PropertyValue::Text("EI60".to_owned())
                ),
                ("IsExternal".to_owned(), PropertyValue::Boolean(true)),
                (
                    "Reference".to_owned(),
                    PropertyValue::Text("W-12".to_owned())
                ),
                ("Width".to_owned(), PropertyValue::Length(200.0)),
                (
                    "ThermalTransmittance".to_owned(),
                    PropertyValue::Number(0.24, Some("W/(m²·K)".to_owned()))
                ),
                (
                    "Status".to_owned(),
                    PropertyValue::Text("NEW, EXISTING".to_owned())
                ),
            ]
        );
        let quantities = sets
            .iter()
            .find(|s| s.name == "Qto_WallBaseQuantities")
            .expect("the wall's quantities");
        assert!(quantities.quantities);
        assert_eq!(
            quantities.properties,
            vec![("NetVolume".to_owned(), PropertyValue::Volume(2.8e9))]
        );
        // The window type's set, which both windows share.
        let window = &model.products[&130];
        let t = window.type_object.expect("typed");
        let type_sets = &model.type_property_sets[&t];
        let set = property_set(&file, &model.units, type_sets[0]).expect("a set");
        assert_eq!(set.name, "Pset_WindowCommon");
        assert_eq!(
            set.properties,
            vec![
                (
                    "GlazingAreaFraction".to_owned(),
                    PropertyValue::Number(0.7, None)
                ),
                (
                    "Manufactured".to_owned(),
                    PropertyValue::Date("2026-03-01".to_owned())
                ),
            ]
        );
    }

    #[test]
    fn dates_and_masses() {
        assert_eq!(date("2026-03-01T10:00"), Some("2026-03-01".to_owned()));
        assert_eq!(date("March"), None);
        let text = "ISO-10303-21;HEADER;FILE_SCHEMA(('IFC4'));ENDSEC;DATA;\n\
                    #1=IFCSIUNIT(*,.MASSUNIT.,.KILO.,.GRAM.);\n\
                    #2=IFCPROPERTYSINGLEVALUE('Weight',$,IFCMASSMEASURE(12.5),#1);\n\
                    #3=IFCSIUNIT(*,.MASSUNIT.,$,.GRAM.);\n\
                    #4=IFCPROPERTYSINGLEVALUE('Small',$,IFCMASSMEASURE(250.),#3);\n\
                    #5=IFCPROPERTYSET('0123456789012345678901',$,'Masses',$,(#2,#4));\n\
                    ENDSEC;END-ISO-10303-21;";
        let file = IfcFile::parse(text).expect("reads");
        let set = property_set(&file, &Units::default(), 5).expect("a set");
        assert_eq!(
            set.properties,
            vec![
                ("Weight".to_owned(), PropertyValue::Mass(12.5)),
                ("Small".to_owned(), PropertyValue::Mass(0.25)),
            ]
        );
    }
}
