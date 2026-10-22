// SPDX-License-Identifier: MIT
//! Parameters' kinds and names (mitcad#501): what a parameter's values
//! are, so that its numbers can be read in their units.
//!
//! The file keeps its numbers in internal units: lengths in feet, areas in
//! square feet, volumes in cubic feet, angles in radians, masses in
//! kilograms. What a number is follows from its parameter's definition:
//!
//! - **Project, shared and family parameters** (positive ids) are
//!   elements (`ParamElem`, `ParamElemFamily`, `ParamElemExternal`, ...)
//!   whose definition (`m_pParamDef`) names them (`m_caption`) and whose
//!   class gives the kind: a `ParamDefValue` (and its classes) by its spec
//!   (`m_specTypeId`, an identifier `<namespace>.spec.<name>-<version>`
//!   such as `….spec.aec:length-1.0.0` or `….spec.aec.piping:pipeSize-1.0.0`,
//!   from the 2021 release on), a `ParamDefYesNo` a yes or no, a
//!   `ParamDefInt` an integer, a `ParamDefString` text, a
//!   `ParamDefMaterialBrowse` or `ParamDefFamType` an element *(verified
//!   on the 2024 and 2025 projects of the test corpus)*. Older releases
//!   keep a `m_unitType` number instead of the spec; it is not read, so
//!   their numbers stay of an unknown kind.
//! - **Built-in parameters** (negative ids) are not defined in the file:
//!   the program names them. [`BUILT_IN`] lists the common ones with
//!   their kinds: the names as the test corpus's schedules head their
//!   columns (`ScheduleField`: `m_paramElemId`, `m_columnHeading`), the
//!   kinds checked against the values (a door type's height and width
//!   against its type's name, a wall's height against its faces), and the
//!   walls' heights and the parameters the import already reads by id
//!   (curtain wall mullions, structural sections) named in Mitcad's words
//!   after what their values are. Others stay `Parameter <id>` of an
//!   unknown kind.

use serde::Serialize;

/// What a parameter's values are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    /// A length (internal: feet).
    Length,
    /// An area (square feet).
    Area,
    /// A volume (cubic feet).
    Volume,
    /// An angle (radians).
    Angle,
    /// A mass (kilograms).
    Mass,
    /// A plain number without a unit: a number, a count, a currency.
    Number,
    Integer,
    /// Yes or no (stored as an integer, 1 for yes).
    Boolean,
    Text,
    /// An element (a material, a type, a level).
    Element,
    /// A kind the reader does not know or does not convert (electrical,
    /// mechanical and structural quantities, ...): its numbers stay in
    /// the file's internal units.
    Unknown,
}

impl ParamKind {
    /// How many of the model's canonical units (mm, mm², mm³, radians,
    /// kg) one internal unit of this kind is; None for kinds that are no
    /// quantities.
    pub fn to_canonical(self) -> Option<f64> {
        const FOOT_MM: f64 = 304.8;
        match self {
            Self::Length => Some(FOOT_MM),
            Self::Area => Some(FOOT_MM * FOOT_MM),
            Self::Volume => Some(FOOT_MM * FOOT_MM * FOOT_MM),
            Self::Angle | Self::Mass => Some(1.0),
            _ => None,
        }
    }
}

/// A parameter's definition: its name and kind, and its spec when the
/// file names one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParamDef {
    pub name: String,
    pub kind: ParamKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<String>,
}

/// The specs whose numbers are lengths in feet.
const LENGTH_SPECS: &[&str] = &[
    "aec:length",
    "aec:sheetLength",
    "aec.piping:pipeSize",
    "aec.piping:pipeDimension",
    "aec.piping:pipeInsulationThickness",
    "aec.hvac:ductSize",
    "aec.hvac:ductInsulationThickness",
    "aec.hvac:ductLiningThickness",
    "aec.electrical:conduitSize",
    "aec.electrical:cableTraySize",
    "aec.structural:barDiameter",
    "aec.structural:crackWidth",
    "aec.structural:displacement",
    "aec.structural:reinforcementCover",
    "aec.structural:reinforcementLength",
    "aec.structural:reinforcementSpacing",
    "aec.structural:sectionDimension",
];

/// The specs whose numbers are areas in square feet.
const AREA_SPECS: &[&str] = &[
    "aec:area",
    "aec.structural:reinforcementArea",
    "aec.structural:sectionArea",
];

/// The specs whose numbers are volumes in cubic feet.
const VOLUME_SPECS: &[&str] = &[
    "aec:volume",
    "aec.piping:pipingVolume",
    "aec.structural:reinforcementVolume",
];

/// The specs whose numbers are angles in radians.
const ANGLE_SPECS: &[&str] = &["aec:angle", "aec:siteAngle"];

/// The specs whose numbers are masses in kilograms.
const MASS_SPECS: &[&str] = &["aec.structural:mass"];

/// The specs whose numbers have no unit.
const NUMBER_SPECS: &[&str] = &["aec:number", "aec:currency"];

/// A spec identifier's name: without its namespace and version (`aec:length`
/// of `<namespace>.spec.aec:length-2.0.0`); None when it is no spec.
pub fn spec_name(spec: &str) -> Option<&str> {
    let (_, name) = spec.trim().split_once(".spec.")?;
    Some(match name.rfind('-') {
        Some(i)
            if i + 1 < name.len()
                && name[i + 1..]
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.') =>
        {
            &name[..i]
        }
        _ => name,
    })
}

/// The kind of a spec's numbers ([`ParamKind::Unknown`] for specs this
/// reader does not convert).
pub fn kind_of_spec(spec: &str) -> ParamKind {
    let Some(name) = spec_name(spec) else {
        return ParamKind::Unknown;
    };
    let is = |list: &[&str]| list.contains(&name);
    if is(LENGTH_SPECS) {
        ParamKind::Length
    } else if is(AREA_SPECS) {
        ParamKind::Area
    } else if is(VOLUME_SPECS) {
        ParamKind::Volume
    } else if is(ANGLE_SPECS) {
        ParamKind::Angle
    } else if is(MASS_SPECS) {
        ParamKind::Mass
    } else if is(NUMBER_SPECS) {
        ParamKind::Number
    } else {
        ParamKind::Unknown
    }
}

/// The kind a parameter definition's class gives (`is_a` tells whether
/// the definition is of a class or derives from it), with its spec for a
/// `ParamDefValue`.
pub fn kind_of_def(is_a: impl Fn(&str) -> bool, spec: Option<&str>) -> ParamKind {
    // Most specific first: the browsers of elements derive from the text
    // definitions, a yes or no from the combo boxes.
    if is_a("ParamDefMaterialBrowse")
        || is_a("ParamDefFamTypeBase")
        || is_a("ParamDefImageSymbolBrowseEdit")
        || is_a("ParamDefElementId")
    {
        ParamKind::Element
    } else if is_a("ParamDefYesNo") {
        ParamKind::Boolean
    } else if is_a("ParamDefInt") {
        ParamKind::Integer
    } else if is_a("ParamDefString") || is_a("ParamDefComboString") {
        ParamKind::Text
    } else if is_a("ParamDefValue") {
        spec.map_or(ParamKind::Unknown, kind_of_spec)
    } else if is_a("ParamDefLength") {
        ParamKind::Length
    } else if is_a("ParamDefArea") {
        ParamKind::Area
    } else if is_a("ParamDefVolume") {
        ParamKind::Volume
    } else if is_a("ParamDefAngle") {
        ParamKind::Angle
    } else {
        ParamKind::Unknown
    }
}

/// The common built-in parameters: (id, name, kind). The ids are the
/// program's public built-in parameter ids.
pub const BUILT_IN: &[(i64, &str, ParamKind)] = &[
    // Walls: the type's width (its layers' total), the height (verified
    // against the walls' faces), the height given when the top is not
    // attached to a level (equal to the height on the walls whose top is
    // not), the offsets of the base and top, the levels of the two.
    (-1_001_000, "Width", ParamKind::Length),
    (-1_001_101, "Height", ParamKind::Length),
    (-1_001_103, "Top Constraint", ParamKind::Element),
    (-1_001_105, "Unconnected Height", ParamKind::Length),
    (-1_001_107, "Base Constraint", ParamKind::Element),
    (-1_001_108, "Base Offset", ParamKind::Length),
    (-1_001_109, "Top Offset", ParamKind::Length),
    // Common identity data.
    (-1_001_203, "Mark", ParamKind::Text),
    (-1_001_205, "Cost", ParamKind::Number),
    (-1_001_206, "Fire Rating", ParamKind::Text),
    (-1_001_208, "Finish", ParamKind::Text),
    (-1_001_405, "Type Mark", ParamKind::Text),
    (-1_010_106, "Comments", ParamKind::Text),
    (-1_010_108, "Manufacturer", ParamKind::Text),
    (-1_010_109, "Model", ParamKind::Text),
    // Doors and windows (their types' height, width and thickness:
    // verified against the type names, e.g. `3 x 8` and `800 x 2100`).
    (-1_001_300, "Height", ParamKind::Length),
    (-1_001_301, "Width", ParamKind::Length),
    (-1_001_302, "Thickness", ParamKind::Length),
    (-1_001_362, "Head Height", ParamKind::Length),
    (-1_001_375, "Length", ParamKind::Length),
    (-1_010_300, "Height", ParamKind::Length),
    (-1_010_301, "Width", ParamKind::Length),
    // Floors, columns, levels and other elements.
    (-1_001_598, "Elevation at Top", ParamKind::Length),
    (-1_001_951, "Height Offset From Level", ParamKind::Length),
    (-1_002_063, "Base Level", ParamKind::Element),
    (-1_002_064, "Top Level", ParamKind::Element),
    (-1_002_065, "Base Offset", ParamKind::Length),
    (-1_002_066, "Top Offset", ParamKind::Length),
    (-1_004_005, "Length", ParamKind::Length),
    (-1_006_902, "Area", ParamKind::Area),
    (-1_006_917, "Perimeter", ParamKind::Length),
    (-1_007_102, "Elevation", ParamKind::Length),
    (-1_012_010, "Floor Area", ParamKind::Area),
    (-1_012_805, "Area", ParamKind::Area),
    (-1_133_501, "Origin Level Offset", ParamKind::Length),
    // Structural sections (the import builds the sections from them).
    (-1_005_502, "Section Width", ParamKind::Length),
    (-1_005_503, "Section Height", ParamKind::Length),
    (-1_005_504, "Section Diameter", ParamKind::Length),
    (
        -1_005_505,
        "Section Wall Nominal Thickness",
        ParamKind::Length,
    ),
    (
        -1_005_506,
        "Section Wall Design Thickness",
        ParamKind::Length,
    ),
    (-1_005_524, "Section Flange Thickness", ParamKind::Length),
    (-1_005_525, "Section Web Thickness", ParamKind::Length),
    (-1_005_529, "Section Inner Fillet Radius", ParamKind::Length),
    (-1_005_530, "Section Outer Fillet Radius", ParamKind::Length),
    // Curtain wall mullions (the import builds the mullions from them).
    (-1_007_300, "Width on Side 1", ParamKind::Length),
    (-1_007_301, "Width on Side 2", ParamKind::Length),
    (-1_007_304, "Thickness", ParamKind::Length),
    (-1_007_351, "Offset", ParamKind::Length),
    (-1_007_354, "Leg 1", ParamKind::Length),
    (-1_007_355, "Leg 2", ParamKind::Length),
    (-1_007_357, "Depth 1", ParamKind::Length),
    (-1_007_358, "Depth 2", ParamKind::Length),
];

/// A built-in parameter's name and kind, when [`BUILT_IN`] lists it.
pub fn built_in(id: i64) -> Option<(&'static str, ParamKind)> {
    BUILT_IN
        .iter()
        .find(|(p, _, _)| *p == id)
        .map(|(_, name, kind)| (*name, *kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_name_their_kinds() {
        // Identifiers as the files keep them, in a namespace of the
        // program's (written `ns` here).
        let spec = |name: &str| format!("ns.spec.{name}");
        assert_eq!(spec_name(&spec("aec:length-2.0.0")), Some("aec:length"));
        assert_eq!(spec_name(&spec("aec:length")), Some("aec:length"));
        assert_eq!(spec_name("ns.local.family:28173c-1.0.0"), None);
        assert_eq!(spec_name(""), None);
        for (name, kind) in [
            ("aec:length-1.0.0", ParamKind::Length),
            ("aec:length-2.0.0", ParamKind::Length),
            ("aec.piping:pipeSize-1.0.0", ParamKind::Length),
            ("aec.hvac:ductSize-1.0.0", ParamKind::Length),
            ("aec:area-1.0.0", ParamKind::Area),
            ("aec:volume-1.0.0", ParamKind::Volume),
            ("aec:angle-1.0.0", ParamKind::Angle),
            ("aec.structural:mass-1.0.0", ParamKind::Mass),
            ("aec:number-1.0.1", ParamKind::Number),
            ("aec:currency-1.0.0", ParamKind::Number),
            ("aec.electrical:current-1.0.0", ParamKind::Unknown),
            ("aec:slope-1.0.0", ParamKind::Unknown),
        ] {
            assert_eq!(kind_of_spec(&spec(name)), kind, "{name}");
        }
        assert_eq!(kind_of_spec("aec:length-1.0.0"), ParamKind::Unknown);
        assert_eq!(kind_of_spec(""), ParamKind::Unknown);
    }

    #[test]
    fn definitions_give_kinds_by_class() {
        // A class and its bases, as the schema's `is_a` answers.
        let of = |chain: &'static [&'static str]| move |name: &str| chain.contains(&name);
        let spec = Some("ns.spec.aec:length-1.0.0");
        assert_eq!(
            kind_of_def(of(&["ParamDefValue", "ParamDef"]), spec),
            ParamKind::Length
        );
        assert_eq!(
            kind_of_def(
                of(&["DuctSizeParamDefCombo", "ParamDefValue", "ParamDef"]),
                spec
            ),
            ParamKind::Length
        );
        assert_eq!(
            kind_of_def(of(&["ParamDefValue", "ParamDef"]), None),
            ParamKind::Unknown
        );
        assert_eq!(
            kind_of_def(of(&["ParamDefYesNo", "ParamDefCombo", "ParamDef"]), None),
            ParamKind::Boolean
        );
        assert_eq!(
            kind_of_def(of(&["ParamDefNoOfPoles", "ParamDefInt", "ParamDef"]), None),
            ParamKind::Integer
        );
        assert_eq!(
            kind_of_def(
                of(&[
                    "ParamDefURL",
                    "ParamDefBrowseEdit",
                    "ParamDefString",
                    "ParamDef"
                ]),
                None
            ),
            ParamKind::Text
        );
        assert_eq!(
            kind_of_def(
                of(&[
                    "ParamDefMaterialBrowse",
                    "ParamDefBrowseEdit",
                    "ParamDefString",
                    "ParamDef"
                ]),
                None
            ),
            ParamKind::Element
        );
        assert_eq!(
            kind_of_def(of(&["ParamDefColor", "ParamDef"]), None),
            ParamKind::Unknown
        );
    }

    #[test]
    fn built_in_parameters_are_listed_once_with_quantities_in_canonical_units() {
        let mut ids: Vec<i64> = BUILT_IN.iter().map(|(id, _, _)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), BUILT_IN.len(), "an id listed twice");
        assert!(ids.iter().all(|id| *id < 0));
        assert_eq!(built_in(-1_001_300), Some(("Height", ParamKind::Length)));
        assert_eq!(built_in(-1_001_301), Some(("Width", ParamKind::Length)));
        assert_eq!(built_in(-1), None);
        // 0.656 ft is 200 mm (a wall's width); one square foot in mm².
        let mm = 0.656_167_979 * ParamKind::Length.to_canonical().unwrap();
        assert!((mm - 200.0).abs() < 1e-6, "{mm}");
        assert!((ParamKind::Area.to_canonical().unwrap() - 92_903.04).abs() < 1e-6);
        assert!((ParamKind::Volume.to_canonical().unwrap() - 28_316_846.592).abs() < 1e-3);
        assert_eq!(ParamKind::Angle.to_canonical(), Some(1.0));
        assert_eq!(ParamKind::Number.to_canonical(), None);
        assert_eq!(ParamKind::Unknown.to_canonical(), None);
    }
}
