// SPDX-License-Identifier: MIT
//! The subset of the IFC schemas this reader knows, written by hand from
//! the specification's text for IFC2X3 TC1, IFC4 ADD2 TC1 and IFC4X3 ADD2.
//!
//! The attributes the reader uses keep their positions across the three
//! schemas for the entities read here (the later schemas add attributes at
//! the end, or make earlier ones optional); where they differ, the reader
//! says so where it reads them. Products are recognised by their
//! attributes (IfcProduct's GlobalId, ObjectPlacement and Representation),
//! so that element types this table does not name are read too; the table
//! only sorts out the products that are not elements.

/// What a product is to the import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductKind {
    /// IfcSite, IfcBuilding, IfcBuildingStorey and IFC4X3's facilities and
    /// their parts: components.
    Spatial,
    /// IfcSpace, IfcSpatialZone, IfcExternalSpatialElement: not imported.
    Space,
    /// A building element, a distribution element, a furnishing, ...: its
    /// shape becomes bodies.
    Element,
    /// IfcOpeningElement, IfcOpeningStandardCase, IfcVoidingFeature: cut
    /// from the element it voids.
    Opening,
    /// IfcGrid: construction axes.
    Grid,
    /// IfcAnnotation: drawings' text and lines, not imported.
    Annotation,
    /// Ports of distribution systems: points, not imported.
    Port,
    /// The structural analysis model's members, connections and loads:
    /// not imported.
    Structural,
    /// IfcVirtualElement, IfcProxy: no physical element, not imported.
    Virtual,
}

impl ProductKind {
    /// The kind of a product's type (upper case).
    pub fn of(kind: &str) -> ProductKind {
        match kind {
            "IFCSITE"
            | "IFCBUILDING"
            | "IFCBUILDINGSTOREY"
            | "IFCFACILITY"
            | "IFCFACILITYPART"
            | "IFCFACILITYPARTCOMMON"
            | "IFCBRIDGE"
            | "IFCBRIDGEPART"
            | "IFCROAD"
            | "IFCROADPART"
            | "IFCRAILWAY"
            | "IFCRAILWAYPART"
            | "IFCMARINEFACILITY"
            | "IFCMARINEPART" => ProductKind::Spatial,
            "IFCSPACE" | "IFCSPATIALZONE" | "IFCEXTERNALSPATIALELEMENT" => ProductKind::Space,
            "IFCOPENINGELEMENT" | "IFCOPENINGSTANDARDCASE" | "IFCVOIDINGFEATURE" => {
                ProductKind::Opening
            }
            "IFCGRID" => ProductKind::Grid,
            "IFCANNOTATION" => ProductKind::Annotation,
            "IFCDISTRIBUTIONPORT" | "IFCPORT" => ProductKind::Port,
            "IFCVIRTUALELEMENT" | "IFCPROXY" => ProductKind::Virtual,
            k if k.starts_with("IFCSTRUCTURAL") => ProductKind::Structural,
            _ => ProductKind::Element,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ProductKind::Spatial => "spatial",
            ProductKind::Space => "space",
            ProductKind::Element => "element",
            ProductKind::Opening => "opening",
            ProductKind::Grid => "grid",
            ProductKind::Annotation => "annotation",
            ProductKind::Port => "port",
            ProductKind::Structural => "structural",
            ProductKind::Virtual => "virtual",
        }
    }
}

/// Whether the import keeps an entity type's attributes: everything but
/// the property values it does not read (bounded, list, table, reference
/// and complex values, templates, dependencies), and the space boundaries
/// with their connection geometry (`ReadOptions::import`). Element
/// quantities and their relationships are kept: they check the volumes;
/// property sets with their single and enumerated values become the
/// elements' properties (mitcad#496).
pub fn used_by_import(kind: &str) -> bool {
    let read_property = matches!(
        kind,
        "IFCPROPERTYSET" | "IFCPROPERTYSINGLEVALUE" | "IFCPROPERTYENUMERATEDVALUE"
    );
    read_property
        || !(kind.starts_with("IFCPROPERTY")
            || kind.starts_with("IFCRELSPACEBOUNDARY")
            || kind.starts_with("IFCCONNECTION")
            || kind.starts_with("IFCSIMPLEPROPERTYTEMPLATE")
            || kind == "IFCRELCONNECTSPATHELEMENTS"
            || kind == "IFCCOMPLEXPROPERTY"
            || kind == "IFCCLASSIFICATIONREFERENCE")
}

/// Shape representations whose items are a body, in the order the import
/// prefers them by their RepresentationIdentifier (`Body` first); `Box`, a
/// bounding box only, is the last resort.
pub const BODY_IDENTIFIERS: [&str; 4] = ["BODY", "FACETATION", "", "BOX"];

/// Representation identifiers that are not bodies (axes, footprints,
/// annotations, ...).
pub fn is_body_identifier(identifier: &str) -> bool {
    BODY_IDENTIFIERS.contains(&identifier.to_ascii_uppercase().as_str())
        || identifier.eq_ignore_ascii_case("Body-FallBack")
}

/// Representation types (RepresentationType) that hold no 3D body.
pub fn is_body_type(kind: &str) -> bool {
    !matches!(
        kind.to_ascii_uppercase().as_str(),
        "CURVE"
            | "CURVE2D"
            | "CURVE3D"
            | "POINT"
            | "POINTCLOUD"
            | "GEOMETRICCURVESET"
            | "GEOMETRICSET"
            | "ANNOTATION2D"
            | "ANNOTATION"
            | "LIGHTSOURCE"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(ProductKind::of("IFCWALL"), ProductKind::Element);
        assert_eq!(ProductKind::of("IFCWALLSTANDARDCASE"), ProductKind::Element);
        assert_eq!(ProductKind::of("IFCCOURSE"), ProductKind::Element);
        assert_eq!(ProductKind::of("IFCBRIDGEPART"), ProductKind::Spatial);
        assert_eq!(
            ProductKind::of("IFCSTRUCTURALCURVEMEMBER"),
            ProductKind::Structural
        );
        assert!(used_by_import("IFCPROPERTYSINGLEVALUE"));
        assert!(used_by_import("IFCPROPERTYSET"));
        assert!(!used_by_import("IFCPROPERTYBOUNDEDVALUE"));
        assert!(!used_by_import("IFCPROPERTYSETTEMPLATE"));
        assert!(used_by_import("IFCELEMENTQUANTITY"));
        assert!(used_by_import("IFCRELDEFINESBYPROPERTIES"));
        assert!(is_body_identifier("Body"));
        assert!(!is_body_identifier("Axis"));
    }
}
