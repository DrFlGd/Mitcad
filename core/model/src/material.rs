// SPDX-License-Identifier: MIT
//! Materials (mitcad#640, part 2): what a body is made of, as an object of
//! its own. A material has a physical side (its density), a look (an
//! appearance of the library or the document) and, for printing, a
//! filament (`Print Filament` values of the key map, `crate::print_keys`).
//!
//! The built-in materials are the fixed list of densities the model always
//! had (`crate::analysis::MATERIALS`), with their ids (`steel`, `pla`,
//! ...), now each with an appearance and, for plastics, a filament type. A
//! design keeps materials of its own (`document/materials.rs`): made by
//! the user, imported, or copied from a library; one with a built-in's id
//! takes that built-in's place in the design. A copy remembers where it
//! came from (`origin`: the library, the id and the version), so that a
//! newer version in a library can be offered and a library change never
//! changes the design by itself.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::analysis;
use crate::print_keys::{self, Dialect, PrintSet};
use crate::print_library::BasedOn;

/// A material: its id, name, density, appearance and filament.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    /// The id bodies and components refer to (`steel`, `pla`, `custom1`,
    /// a vendor's `<vendor>/<product>`).
    pub id: String,
    pub name: String,
    /// g/cm³.
    pub density: f64,
    /// The appearance a body gets with the material: an id of the
    /// library's or the document's appearances.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<String>,
    /// The filament's print settings: values of keys of the `Print
    /// Filament` set by Mitcad key, in their JSON form (a key that names a
    /// dialect's own preset holds an object of a value per dialect id).
    /// The density is the material's own.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub filament: BTreeMap<String, Value>,
    /// A slicer's filament settings that have no counterpart in the key
    /// map, by slicer id: its key and text, written only to that slicer
    /// (mitcad#640, part 5).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slicer: BTreeMap<String, BTreeMap<String, String>>,
    /// The material it was made from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    /// Its class (mitcad#655): metal, plastic (the filaments too) or other,
    /// the group it is listed in. Without one (designs and library files
    /// before it) `material_class` works it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<MaterialClass>,
    /// Its version in its library (counts up with each change there).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// Where the design's copy came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<MaterialOrigin>,
}

/// A material's class (mitcad#655): what it is made of, by which the
/// lists group it. Plastics hold the filaments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterialClass {
    Metal,
    Plastic,
    Other,
}

impl MaterialClass {
    /// Its id in commands and files.
    pub fn id(self) -> &'static str {
        match self {
            MaterialClass::Metal => "metal",
            MaterialClass::Plastic => "plastic",
            MaterialClass::Other => "other",
        }
    }

    /// The group of the lists (`library_items`' `group`).
    pub fn group(self) -> &'static str {
        match self {
            MaterialClass::Metal => "metals",
            MaterialClass::Plastic => "plastics",
            MaterialClass::Other => "other",
        }
    }
}

/// Where a design's material came from: the library (`base`, `user`,
/// `project`, a 3MF import's `3mf`), its id there and the version copied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialOrigin {
    pub library: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// A newer library version the design kept its own values over
    /// ("Keep these", mitcad#655): not offered again until the library
    /// changes again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declined: Option<u32>,
}

/// A change of a material in a command: the fields given; in
/// `edit_material`, `null` takes an optional field away and a filament
/// key's `null` removes that key.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialChange {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub density: Option<f64>,
    #[serde(default, deserialize_with = "some")]
    pub appearance: Option<Option<String>>,
    #[serde(default)]
    pub filament: Option<BTreeMap<String, Value>>,
    /// `create_material` only: the material whose fields it starts from,
    /// by id, or a library's item `{library, id}` (mitcad#655).
    #[serde(default)]
    pub based_on: Option<BasedOn>,
    /// The class (mitcad#655).
    #[serde(default)]
    pub class: Option<MaterialClass>,
    #[serde(default, deserialize_with = "some")]
    pub version: Option<Option<u32>>,
    #[serde(default, deserialize_with = "some")]
    pub origin: Option<Option<MaterialOrigin>>,
    /// The library the item lives in (`user` or `project`), when the
    /// command works there rather than on the design's own material
    /// (mitcad#655).
    #[serde(default)]
    pub library: Option<String>,
    /// The project's folder, when the design is in one.
    #[serde(default)]
    pub project: Option<String>,
}

/// A field given as `null` is `Some(None)`; one left out stays `None`.
fn some<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

impl MaterialChange {
    /// Applies the given fields to a material (not `id` or `based_on`).
    pub(crate) fn apply_to(&self, material: &mut Material) {
        if let Some(name) = &self.name {
            material.name = name.trim().to_owned();
        }
        if let Some(density) = self.density {
            material.density = density;
        }
        if let Some(appearance) = &self.appearance {
            material.appearance = appearance.as_ref().map(|a| a.trim().to_owned());
        }
        if let Some(filament) = &self.filament {
            for (key, value) in filament {
                if value.is_null() {
                    material.filament.remove(key);
                } else {
                    material.filament.insert(key.clone(), value.clone());
                }
            }
        }
        if let Some(class) = self.class {
            material.class = Some(class);
        }
        if let Some(version) = self.version {
            material.version = version;
        }
        if let Some(origin) = &self.origin {
            material.origin = origin.clone();
        }
    }
}

/// Bodies and components without a material of their own are the default
/// material's, steel.
pub use analysis::DEFAULT_MATERIAL;

/// The built-in materials' appearances and filament types, by id.
const BUILT_IN_LOOKS: &[(&str, Option<&str>, Option<&str>)] = &[
    ("steel", Some("steel_satin"), None),
    ("stainless_steel", Some("steel_polished"), None),
    ("cast_iron", Some("cast_iron"), None),
    ("aluminum", Some("aluminum_anodized"), None),
    ("copper", Some("copper"), None),
    ("brass", Some("brass_polished"), None),
    ("titanium", Some("steel_satin"), None),
    ("abs", Some("plastic_black"), Some("ABS")),
    ("pla", Some("plastic_white"), Some("PLA")),
    ("nylon", Some("plastic_white"), Some("PA")),
    ("polycarbonate", Some("plastic_clear"), Some("PC")),
    ("glass", Some("glass_clear"), None),
    ("water", None, None),
];

/// The built-in materials, in the application's order.
pub fn built_in() -> &'static [Material] {
    static BUILT_IN: LazyLock<Vec<Material>> = LazyLock::new(|| {
        analysis::MATERIALS
            .iter()
            .map(|m| {
                let (appearance, filament_type) = BUILT_IN_LOOKS
                    .iter()
                    .find(|(id, ..)| *id == m.id)
                    .map(|(_, a, f)| (*a, *f))
                    .unwrap_or((None, None));
                Material {
                    id: m.id.to_owned(),
                    name: m.name.to_owned(),
                    density: m.density,
                    appearance: appearance.map(str::to_owned),
                    filament: filament_type
                        .map(|t| BTreeMap::from([("filament_type".to_owned(), Value::from(t))]))
                        .unwrap_or_default(),
                    slicer: BTreeMap::new(),
                    based_on: None,
                    class: None,
                    version: None,
                    origin: None,
                }
            })
            .collect()
    });
    &BUILT_IN
}

/// A built-in material by id.
pub fn built_in_material(id: &str) -> Option<&'static Material> {
    built_in().iter().find(|m| m.id == id)
}

/// The built-in metals.
const METALS: [&str; 7] = [
    "steel",
    "stainless_steel",
    "cast_iron",
    "aluminum",
    "copper",
    "brass",
    "titanium",
];

/// A material's class (mitcad#655): its own; without one (a design or a
/// library file from before classes) a built-in metal's id, or the
/// built-in it is based on, makes it a metal, a filament setting a
/// plastic, else other.
pub fn material_class(material: &Material) -> MaterialClass {
    if let Some(class) = material.class {
        return class;
    }
    let metal = |id: &str| METALS.contains(&id);
    if metal(&material.id) || material.based_on.as_deref().is_some_and(metal) {
        MaterialClass::Metal
    } else if !material.filament.is_empty()
        || material
            .based_on
            .as_deref()
            .and_then(built_in_material)
            .is_some_and(|base| !base.filament.is_empty())
    {
        MaterialClass::Plastic
    } else {
        MaterialClass::Other
    }
}

/// The group a material is listed in (mitcad#655): its class's, `metals`,
/// `plastics` (the filaments too) or `other`. Setting print settings does
/// not move it.
pub fn material_group(material: &Material) -> &'static str {
    material_class(material).group()
}

/// Whether an id can name a material: letters, digits, `_`, `-`, and `/`
/// between parts (`<vendor>/<product>`).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.split('/').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

impl Material {
    /// Whether the fields are valid (the name and appearance not empty,
    /// the density positive, the filament's keys and values those of the
    /// `Print Filament` set). Whether the appearance exists is the
    /// document's to say.
    pub fn check(&self) -> Result<(), String> {
        if !valid_id(&self.id) {
            return Err(format!(
                "'{}' is no material id: use letters, digits, '_', '-' and '/' between parts",
                self.id
            ));
        }
        if self.name.trim().is_empty() {
            return Err(format!("material {}: the name is empty", self.id));
        }
        if !(self.density > 0.0 && self.density.is_finite()) {
            return Err(format!(
                "material {}: the density must be greater than zero, got {}",
                self.id, self.density
            ));
        }
        if self.appearance.as_deref().is_some_and(|a| a.is_empty()) {
            return Err(format!("material {}: the appearance id is empty", self.id));
        }
        for (key, value) in &self.filament {
            check_filament_value(key, value)
                .map_err(|e| format!("material {}: filament: {e}", self.id))?;
        }
        if let Some(dialect) = self.slicer.keys().find(|d| Dialect::from_id(d).is_none()) {
            return Err(format!("material {}: '{dialect}' is no slicer id", self.id));
        }
        Ok(())
    }

    /// The filament's value of a key, as the key map's value (None when
    /// not set, or a key kept per dialect).
    pub fn filament_value(&self, key: &str) -> Option<print_keys::Value> {
        let def = print_keys::key(key)?;
        if def.per_dialect {
            return None;
        }
        print_keys::Value::from_json(def, self.filament.get(key)?).ok()
    }
}

/// Whether a filament value fits its key: a key of the `Print Filament`
/// set (not `filament_density`, which is the material's density), its
/// kind; a key kept per dialect holds `{"<dialect id>": value, ...}`.
pub fn check_filament_value(key: &str, value: &Value) -> Result<(), String> {
    let def = print_keys::key(key)
        .filter(|def| def.set == PrintSet::Filament)
        .ok_or_else(|| format!("'{key}' is no key of the Print Filament set"))?;
    if key == "filament_density" {
        return Err("the filament's density is the material's density".to_owned());
    }
    if def.per_dialect {
        let Value::Object(values) = value else {
            return Err(format!(
                "{key} holds a value per slicer: {{\"<slicer id>\": value, ...}}"
            ));
        };
        for (dialect, value) in values {
            if Dialect::from_id(dialect).is_none() {
                return Err(format!("{key}: '{dialect}' is no slicer id"));
            }
            print_keys::Value::from_json(def, value).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    print_keys::Value::from_json(def, value)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn built_in_materials_keep_their_ids_with_looks() {
        assert_eq!(built_in().len(), analysis::MATERIALS.len());
        let pla = built_in_material("pla").unwrap();
        assert_eq!(pla.density, 1.24);
        assert_eq!(pla.appearance.as_deref(), Some("plastic_white"));
        assert_eq!(pla.filament["filament_type"], json!("PLA"));
        assert_eq!(
            built_in_material(DEFAULT_MATERIAL)
                .unwrap()
                .appearance
                .as_deref(),
            Some("steel_satin")
        );
        for m in built_in() {
            m.check().unwrap();
            if let Some(a) = &m.appearance {
                assert!(crate::appearance::library_appearance(a).is_some(), "{a}");
            }
        }
    }

    #[test]
    fn ids_and_filament_values_are_checked() {
        assert!(valid_id("acme/petg-hf"));
        assert!(!valid_id("acme//petg") && !valid_id("/petg") && !valid_id("pe tg"));
        let mut m = built_in_material("pla").unwrap().clone();
        m.filament.insert("temperature".to_owned(), json!(215));
        m.filament
            .insert("filament_colour".to_owned(), json!("#ff0000"));
        m.check().unwrap();
        assert_eq!(
            m.filament_value("temperature"),
            Some(print_keys::Value::Number(215.0))
        );
        m.filament.insert("layer_height".to_owned(), json!(0.2));
        assert!(m.check().unwrap_err().contains("Print Filament"));
        m.filament.remove("layer_height");
        m.filament.insert("filament_density".to_owned(), json!(1.2));
        assert!(m.check().is_err());
        m.filament.remove("filament_density");
        m.filament.insert(
            "filament_settings_id".to_owned(),
            json!({"prusaslicer": "Generic PLA"}),
        );
        m.check().unwrap();
        m.filament
            .insert("filament_settings_id".to_owned(), json!({"nobody": "x"}));
        assert!(m.check().is_err());
    }

    #[test]
    fn materials_are_grouped_by_their_class() {
        let group = |id: &str| material_group(built_in_material(id).unwrap());
        assert_eq!(group("steel"), "metals");
        assert_eq!(group("brass"), "metals");
        assert_eq!(group("pla"), "plastics");
        assert_eq!(group("glass"), "other");
        assert_eq!(group("water"), "other");
        // A material of its own with filament settings is a plastic, also
        // without them when based on one; one based on a metal a metal.
        let mut petg = built_in_material("pla").unwrap().clone();
        petg.id = "acme_petg".to_owned();
        assert_eq!(material_class(&petg), MaterialClass::Plastic);
        petg.filament.clear();
        assert_eq!(material_class(&petg), MaterialClass::Other);
        petg.based_on = Some("pla".to_owned());
        assert_eq!(material_class(&petg), MaterialClass::Plastic);
        let mut own_steel = built_in_material("steel").unwrap().clone();
        own_steel.id = "shop_steel".to_owned();
        own_steel.based_on = Some("steel".to_owned());
        assert_eq!(material_group(&own_steel), "metals");
    }

    #[test]
    fn a_stored_class_wins_and_print_settings_do_not_move_it() {
        let mut m = built_in_material("glass").unwrap().clone();
        m.id = "resin".to_owned();
        m.class = Some(MaterialClass::Plastic);
        assert_eq!(material_group(&m), "plastics");
        m.class = Some(MaterialClass::Other);
        m.filament.insert("filament_type".to_owned(), json!("PLA"));
        assert_eq!(material_group(&m), "other");
        let text = serde_json::to_value(&m).unwrap();
        assert_eq!(text["class"], json!("other"));
        let back: Material = serde_json::from_value(text).unwrap();
        assert_eq!(back.class, Some(MaterialClass::Other));
        // A file from before classes has none.
        let old: Material =
            serde_json::from_value(json!({"id": "x", "name": "X", "density": 1.0})).unwrap();
        assert_eq!(old.class, None);
        assert_eq!(material_class(&old), MaterialClass::Other);
    }
}
