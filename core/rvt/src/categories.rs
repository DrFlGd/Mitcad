// SPDX-License-Identifier: MIT
//! Built-in categories: an element header's category is a negative id of
//! the program's public `BuiltInCategory` enumeration. The file stores the
//! ids, not their names (built-in categories are named by the program), so
//! the names of the common ones are listed here, with whether the import
//! takes their elements as bodies of the building model.
//!
//! The families of system types are not stored either: a type's family is
//! derived from its kind (its class), as [`system_family`] does.

pub const OST_WALLS: i64 = -2_000_011;
pub const OST_WINDOWS: i64 = -2_000_014;
pub const OST_DOORS: i64 = -2_000_023;
pub const OST_FLOORS: i64 = -2_000_032;
pub const OST_ROOFS: i64 = -2_000_035;
pub const OST_CEILINGS: i64 = -2_000_038;
pub const OST_COLUMNS: i64 = -2_000_100;
pub const OST_ROOMS: i64 = -2_000_160;
pub const OST_LEVELS: i64 = -2_000_240;
pub const OST_TOPOGRAPHY: i64 = -2_001_340;

/// (id, name, a body of the building model).
pub const CATEGORIES: &[(i64, &str, bool)] = &[
    (OST_WALLS, "OST_Walls", true),
    (OST_WINDOWS, "OST_Windows", true),
    (OST_DOORS, "OST_Doors", true),
    (OST_FLOORS, "OST_Floors", true),
    (OST_ROOFS, "OST_Roofs", true),
    (OST_CEILINGS, "OST_Ceilings", true),
    (-2_000_045, "OST_SketchLines", false),
    (-2_000_051, "OST_Lines", false),
    (-2_000_080, "OST_Furniture", true),
    (OST_COLUMNS, "OST_Columns", true),
    (-2_000_120, "OST_Stairs", true),
    (-2_000_123, "OST_StairsStringerCarriage", true),
    (-2_000_126, "OST_StairsRailing", true),
    (-2_000_151, "OST_GenericModel", true),
    (OST_ROOMS, "OST_Rooms", false),
    (-2_000_170, "OST_CurtainWallPanels", true),
    (-2_000_171, "OST_CurtainWallMullions", true),
    (-2_000_180, "OST_Ramps", true),
    (-2_000_181, "OST_Cornices", true),
    (-2_000_220, "OST_Grids", false),
    (OST_LEVELS, "OST_Levels", false),
    (-2_000_919, "OST_StairsRuns", true),
    (-2_000_920, "OST_StairsLandings", true),
    (-2_000_946, "OST_RailingTopRail", true),
    (-2_000_947, "OST_RailingHandRail", true),
    (-2_000_948, "OST_RailingSupport", true),
    (-2_001_000, "OST_Casework", true),
    (-2_001_036, "OST_Hardscape", true),
    (-2_001_040, "OST_ElectricalEquipment", true),
    (-2_001_043, "OST_FoodServiceEquipment", true),
    (-2_001_052, "OST_VerticalCirculation", true),
    (-2_001_060, "OST_ElectricalFixtures", true),
    (-2_001_120, "OST_LightingFixtures", true),
    (-2_001_140, "OST_MechanicalEquipment", true),
    (-2_001_160, "OST_PlumbingFixtures", true),
    (-2_001_180, "OST_Parking", true),
    (-2_001_263, "OST_BuildingPad", true),
    (-2_001_300, "OST_StructuralFoundation", true),
    (-2_001_320, "OST_StructuralFraming", true),
    (-2_001_330, "OST_StructuralColumns", true),
    (OST_TOPOGRAPHY, "OST_Topography", true),
    (-2_001_350, "OST_SpecialityEquipment", true),
    (-2_001_360, "OST_Planting", true),
    (-2_001_370, "OST_Entourage", true),
    (-2_001_392, "OST_EdgeSlab", true),
    (-2_008_000, "OST_DuctCurves", true),
    (-2_008_010, "OST_DuctFitting", true),
    (-2_008_013, "OST_DuctTerminal", true),
    (-2_008_044, "OST_PipeCurves", true),
    (-2_008_049, "OST_PipeFitting", true),
    (-2_008_083, "OST_DataDevices", true),
    (-2_008_085, "OST_FireAlarmDevices", true),
    (-2_008_087, "OST_LightingDevices", true),
];

/// A built-in category's name.
pub fn name(id: i64) -> Option<&'static str> {
    CATEGORIES
        .iter()
        .find(|(c, _, _)| *c == id)
        .map(|(_, n, _)| *n)
}

/// Whether the import takes the category's elements as bodies.
pub fn is_model(id: i64) -> bool {
    CATEGORIES.iter().any(|(c, _, model)| *c == id && *model)
}

/// The family of a system type by its class (the file does not store
/// it): the system families of walls, curtain walls, floors, pads,
/// ceilings, roofs, railings, wall sweeps, slab edges, ramps and model
/// text.
pub fn system_family(type_class: &str) -> Option<&'static str> {
    Some(match type_class {
        "BasicWallType" => "Basic Wall",
        "NewCurtainWallType" => "Curtain Wall",
        "StackedWallType" => "Stacked Wall",
        "FloorAttributes" => "Floor",
        "BuildingPadType" => "Pad",
        "CompoundCeilingType" => "Compound Ceiling",
        "RoofAttributes" => "Basic Roof",
        "StairsRailingAttr" => "Railing",
        "CorniceAttr" => "Wall Sweep",
        "EdgeSlabAttr" => "Slab Edge",
        "RampAttributes" => "Ramp",
        "Text3dAttrSymbol" => "Model Text",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_kinds() {
        assert_eq!(name(OST_WALLS), Some("OST_Walls"));
        assert!(is_model(OST_DOORS));
        assert!(!is_model(OST_ROOMS));
        assert!(!is_model(-1));
        assert_eq!(system_family("BasicWallType"), Some("Basic Wall"));
        assert_eq!(system_family("FamilySymbol"), None);
        // No id twice.
        let mut ids: Vec<i64> = CATEGORIES.iter().map(|c| c.0).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATEGORIES.len());
    }
}
