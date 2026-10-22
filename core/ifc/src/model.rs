// SPDX-License-Identifier: MIT
//! The file's model: the project, the spatial structure (sites,
//! buildings, storeys, facilities), the products in it with their
//! placements, the relationships the import uses (aggregation, spatial
//! containment, openings and their fillings, types, materials, element
//! quantities), shape representations, and the colours of styles.
//!
//! Products are found by their attributes: an instance whose first
//! attribute is a string (the GlobalId), whose sixth refers to a placement
//! or is unset and whose seventh refers to a product definition shape or is
//! unset (IfcProduct's ObjectPlacement and Representation), and which is no
//! relationship. That reads every product type of the three schemas the
//! same way; [`ProductKind`] sorts them.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;

use crate::geometry::{Decoder, Path, Seg};
use crate::math::{self, Matrix};
use crate::schema::{self, ProductKind};
use crate::units::Units;
use crate::{Entity, IfcFile, Value};

/// A product.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Product {
    pub id: u64,
    /// Its type (`IFCWALL`).
    #[serde(serialize_with = "arc_str")]
    pub kind: Arc<str>,
    pub class: ProductKind,
    pub global_id: String,
    pub name: String,
    pub object_type: String,
    /// Where it is: its object coordinates in the project's (mm).
    pub placement: Matrix,
    /// Why the placement could not be read (the identity is used).
    pub placement_note: Option<String>,
    /// Its IfcProductDefinitionShape.
    pub shape: Option<u64>,
    /// The object that aggregates it (a spatial element's parent, an
    /// assembly's element).
    pub parent: Option<u64>,
    /// The objects it aggregates.
    pub parts: Vec<u64>,
    /// The spatial element that contains it (IfcRelContainedInSpatial
    /// Structure).
    pub container: Option<u64>,
    /// The elements a spatial element contains.
    pub contents: Vec<u64>,
    /// The openings that void it.
    pub openings: Vec<u64>,
    /// An opening's element.
    pub voids: Option<u64>,
    /// The opening an element fills.
    pub fills: Option<u64>,
    /// The elements that fill an opening.
    pub fillings: Vec<u64>,
    /// Its type object (IfcRelDefinesByType).
    pub type_object: Option<u64>,
    /// Its materials (IfcMaterial instances, layers and constituents
    /// resolved), its own or its type's.
    pub materials: Vec<u64>,
    /// The volumes its element quantities state (name, mm³).
    pub volumes: Vec<(String, f64)>,
    /// Its own property sets and element quantities
    /// (IfcRelDefinesByProperties; `properties::property_set` reads them,
    /// mitcad#496).
    pub property_sets: Vec<u64>,
    /// A storey's elevation (mm).
    pub elevation: Option<f64>,
}

impl Product {
    /// The name, else the type's name without `IFC`.
    pub fn label(&self) -> String {
        if self.name.trim().is_empty() {
            let k = self.kind.strip_prefix("IFC").unwrap_or(&self.kind);
            let mut s = k.to_ascii_lowercase();
            if let Some(f) = s.get_mut(0..1) {
                f.make_ascii_uppercase();
            }
            s
        } else {
            self.name.trim().to_owned()
        }
    }
}

/// A surface style's colour.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Style {
    pub name: String,
    /// sRGB, 0..1.
    pub color: [f64; 3],
    /// 0 opaque, 1 invisible.
    pub transparency: f64,
}

/// A shape representation of a product.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Representation {
    pub id: u64,
    /// RepresentationIdentifier (`Body`) and RepresentationType
    /// (`SweptSolid`).
    pub identifier: String,
    pub kind: String,
    pub items: Vec<u64>,
}

/// A grid's axes in the project's coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GridAxis {
    pub tag: String,
    pub start: [f64; 3],
    pub end: [f64; 3],
}

/// The model the import reads.
#[derive(Debug, Clone, Serialize)]
pub struct Model {
    pub units: Units,
    pub project: Option<u64>,
    pub project_name: String,
    /// Every product by id.
    pub products: BTreeMap<u64, Product>,
    /// The spatial elements the project aggregates (its sites, or its
    /// buildings without a site).
    pub roots: Vec<u64>,
    /// Styles by the representation item they colour.
    pub item_styles: HashMap<u64, Style>,
    /// Styles by material.
    pub material_styles: HashMap<u64, Style>,
    /// Material names by id.
    pub material_names: HashMap<u64, String>,
    /// Representation maps by the type objects that hold them.
    pub map_types: HashMap<u64, u64>,
    /// The property sets of type objects (HasPropertySets), by type object
    /// (mitcad#496).
    pub type_property_sets: HashMap<u64, Vec<u64>>,
    /// What could not be read of the structure.
    pub notes: Vec<String>,
}

impl Model {
    /// Reads the model of a file.
    pub fn read(file: &IfcFile) -> Model {
        let units = Units::of(file);
        let mut model = Model {
            units,
            project: None,
            project_name: String::new(),
            products: BTreeMap::new(),
            roots: Vec::new(),
            item_styles: HashMap::new(),
            material_styles: HashMap::new(),
            material_names: HashMap::new(),
            map_types: HashMap::new(),
            type_property_sets: HashMap::new(),
            notes: Vec::new(),
        };
        let projects = file.of_kind("IFCPROJECT");
        if let Some(p) = projects.first() {
            model.project = Some(p.id);
            model.project_name = p.arg(2).as_str().unwrap_or_default().to_owned();
            if projects.len() > 1 {
                model
                    .notes
                    .push(format!("{} projects: the first is read", projects.len()));
            }
        } else {
            model.notes.push("the file has no IfcProject".to_owned());
        }
        for e in file.sorted() {
            if is_product(file, e) {
                let class = ProductKind::of(&e.kind);
                model.products.insert(
                    e.id,
                    Product {
                        id: e.id,
                        kind: e.kind.clone(),
                        class,
                        global_id: e.arg(0).as_str().unwrap_or_default().to_owned(),
                        name: e.arg(2).as_str().unwrap_or_default().to_owned(),
                        object_type: e.arg(4).as_str().unwrap_or_default().to_owned(),
                        placement: math::IDENTITY,
                        placement_note: None,
                        shape: e.arg(6).as_ref(),
                        parent: None,
                        parts: Vec::new(),
                        container: None,
                        contents: Vec::new(),
                        openings: Vec::new(),
                        voids: None,
                        fills: None,
                        fillings: Vec::new(),
                        type_object: None,
                        materials: Vec::new(),
                        volumes: Vec::new(),
                        property_sets: Vec::new(),
                        elevation: None,
                    },
                );
            }
        }
        model.relationships(file);
        model.placements(file);
        model.styles(file);
        model
    }

    fn relationships(&mut self, file: &IfcFile) {
        let mut type_materials: HashMap<u64, Vec<u64>> = HashMap::new();
        let mut material_assoc: Vec<(Vec<u64>, u64)> = Vec::new();
        for e in file.sorted() {
            match &*e.kind {
                // (GlobalId, OwnerHistory, Name, Description, RelatingObject,
                // RelatedObjects)
                "IFCRELAGGREGATES" => {
                    let parent = e.arg(4).as_ref();
                    let children = e.arg(5).refs();
                    if let Some(parent) = parent {
                        if Some(parent) == self.project {
                            self.roots.extend(
                                children
                                    .iter()
                                    .copied()
                                    .filter(|c| self.products.contains_key(c)),
                            );
                        }
                        for c in &children {
                            if let Some(p) = self.products.get_mut(c) {
                                p.parent = Some(parent);
                            }
                        }
                        if let Some(p) = self.products.get_mut(&parent) {
                            p.parts.extend(children);
                        }
                    }
                }
                // (..., RelatedElements, RelatingStructure)
                "IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                    let Some(structure) = e.arg(5).as_ref() else {
                        continue;
                    };
                    let elements = e.arg(4).refs();
                    for c in &elements {
                        if let Some(p) = self.products.get_mut(c) {
                            p.container = Some(structure);
                        }
                    }
                    if let Some(p) = self.products.get_mut(&structure) {
                        p.contents.extend(elements);
                    }
                }
                // (..., RelatingBuildingElement, RelatedOpeningElement)
                "IFCRELVOIDSELEMENT" => {
                    if let (Some(host), Some(opening)) = (e.arg(4).as_ref(), e.arg(5).as_ref()) {
                        if let Some(p) = self.products.get_mut(&host) {
                            p.openings.push(opening);
                        }
                        if let Some(p) = self.products.get_mut(&opening) {
                            p.voids = Some(host);
                        }
                    }
                }
                // (..., RelatingOpeningElement, RelatedBuildingElement)
                "IFCRELFILLSELEMENT" => {
                    if let (Some(opening), Some(element)) = (e.arg(4).as_ref(), e.arg(5).as_ref()) {
                        if let Some(p) = self.products.get_mut(&opening) {
                            p.fillings.push(element);
                        }
                        if let Some(p) = self.products.get_mut(&element) {
                            p.fills = Some(opening);
                        }
                    }
                }
                // (..., RelatedObjects, RelatingType)
                "IFCRELDEFINESBYTYPE" => {
                    let Some(t) = e.arg(5).as_ref() else {
                        continue;
                    };
                    for c in e.arg(4).refs() {
                        if let Some(p) = self.products.get_mut(&c) {
                            p.type_object = Some(t);
                        }
                    }
                }
                // (..., RelatedObjects, RelatingMaterial)
                "IFCRELASSOCIATESMATERIAL" => {
                    if let Some(m) = e.arg(5).as_ref() {
                        material_assoc.push((e.arg(4).refs(), m));
                    }
                }
                // (..., RelatedObjects, RelatingPropertyDefinition)
                "IFCRELDEFINESBYPROPERTIES" => {
                    // Property sets and quantities, one or (IFC4) a set of
                    // them (mitcad#496).
                    let sets: Vec<u64> = match e.arg(5).as_ref() {
                        Some(set) => vec![set],
                        None => e.arg(5).refs(),
                    };
                    let sets: Vec<u64> = sets
                        .into_iter()
                        .filter(|s| crate::properties::is_property_set(file, *s))
                        .collect();
                    if !sets.is_empty() {
                        for c in e.arg(4).refs() {
                            if let Some(p) = self.products.get_mut(&c) {
                                p.property_sets.extend(sets.iter().copied());
                            }
                        }
                    }
                    let Some(definition) = file.deref(e.arg(5)) else {
                        continue;
                    };
                    if !definition.is("IFCELEMENTQUANTITY") {
                        continue;
                    }
                    let volumes = self.volumes(file, definition);
                    if volumes.is_empty() {
                        continue;
                    }
                    for c in e.arg(4).refs() {
                        if let Some(p) = self.products.get_mut(&c) {
                            p.volumes.extend(volumes.iter().cloned());
                        }
                    }
                }
                _ => {
                    // Type objects' representation maps: (..., HasPropertySets,
                    // RepresentationMaps, ...) at 5, 6.
                    if e.kind.ends_with("TYPE") || e.kind.ends_with("STYLE") {
                        // Its property sets (mitcad#496).
                        let sets: Vec<u64> = e
                            .arg(5)
                            .refs()
                            .into_iter()
                            .filter(|s| crate::properties::is_property_set(file, *s))
                            .collect();
                        if !sets.is_empty() {
                            self.type_property_sets.insert(e.id, sets);
                        }
                        for map in e.arg(6).refs() {
                            if file.kind(map) == Some("IFCREPRESENTATIONMAP") {
                                self.map_types.insert(map, e.id);
                            }
                        }
                    }
                }
            }
        }
        for (objects, material) in material_assoc {
            let materials = resolve_materials(file, material, 0);
            for m in &materials {
                if let Some(e) = file.get(*m) {
                    self.material_names
                        .insert(*m, e.arg(0).as_str().unwrap_or_default().to_owned());
                }
            }
            for o in objects {
                if let Some(p) = self.products.get_mut(&o) {
                    p.materials.extend(materials.iter().copied());
                } else {
                    type_materials
                        .entry(o)
                        .or_default()
                        .extend(materials.iter().copied());
                }
            }
        }
        // Elements without materials of their own take their type's.
        for p in self.products.values_mut() {
            if p.materials.is_empty()
                && let Some(t) = p.type_object
                && let Some(m) = type_materials.get(&t)
            {
                p.materials = m.clone();
            }
        }
        // Storeys' elevations: IfcBuildingStorey(..., LongName,
        // CompositionType, Elevation).
        let length = self.units.length;
        for p in self.products.values_mut() {
            if &*p.kind == "IFCBUILDINGSTOREY"
                && let Some(e) = file.get(p.id)
            {
                p.elevation = e.arg(9).as_f64().map(|v| v * length);
            }
        }
        // Spatial elements the project does not aggregate (a building
        // without the project's aggregation) are roots too.
        let mut roots: HashSet<u64> = self.roots.iter().copied().collect();
        for p in self.products.values() {
            if p.class == ProductKind::Spatial
                && p.parent
                    .is_none_or(|parent| !self.products.contains_key(&parent))
                && roots.insert(p.id)
            {
                self.roots.push(p.id);
            }
        }
        self.roots.sort_unstable();
        self.roots.dedup();
    }

    fn volumes(&self, file: &IfcFile, quantity: &Entity) -> Vec<(String, f64)> {
        // IfcElementQuantity(..., MethodOfMeasurement, Quantities)
        let mut out = Vec::new();
        for q in quantity.arg(5).refs() {
            let Some(q) = file.get(q) else {
                continue;
            };
            // IfcQuantityVolume(Name, Description, Unit, VolumeValue[, Formula])
            if q.is("IFCQUANTITYVOLUME")
                && let Some(v) = q.arg(3).as_f64()
            {
                let scale = self.units.volume_of(file, q.arg(2));
                out.push((q.arg(0).as_str().unwrap_or_default().to_owned(), v * scale));
            }
        }
        out
    }

    fn placements(&mut self, file: &IfcFile) {
        let decoder = Decoder::new(file, &self.units);
        let mut cache: HashMap<u64, Result<Matrix, String>> = HashMap::new();
        let ids: Vec<u64> = self.products.keys().copied().collect();
        for id in ids {
            let placement = file.get(id).and_then(|e| e.arg(5).as_ref());
            let result = match placement {
                Some(p) => placement_of(&decoder, self, p, &mut cache, 0),
                None => Ok(math::IDENTITY),
            };
            let product = self.products.get_mut(&id).expect("listed");
            match result {
                Ok(m) => product.placement = m,
                Err(why) => {
                    product.placement_note = Some(why);
                }
            }
        }
    }

    fn styles(&mut self, file: &IfcFile) {
        for e in file.of_kind("IFCSTYLEDITEM") {
            // (Item, Styles, Name)
            let Some(style) = style_of(file, e) else {
                continue;
            };
            if let Some(item) = e.arg(0).as_ref() {
                self.item_styles.insert(item, style);
            }
        }
        // Materials' styles: IfcMaterialDefinitionRepresentation(Name,
        // Description, Representations, RepresentedMaterial).
        for e in file.of_kind("IFCMATERIALDEFINITIONREPRESENTATION") {
            let Some(material) = e.arg(3).as_ref() else {
                continue;
            };
            'reps: for r in e.arg(2).refs() {
                let Some(rep) = file.get(r) else {
                    continue;
                };
                for item in rep.arg(3).refs() {
                    if let Some(si) = file.get(item)
                        && si.is("IFCSTYLEDITEM")
                        && let Some(style) = style_of(file, si)
                    {
                        self.material_styles.insert(material, style);
                        break 'reps;
                    }
                }
            }
        }
    }

    /// A product's shape representations, the bodies first
    /// ([`schema::BODY_IDENTIFIERS`]).
    pub fn representations(&self, file: &IfcFile, product: &Product) -> Vec<Representation> {
        let Some(shape) = product.shape.and_then(|s| file.get(s)) else {
            return Vec::new();
        };
        // IfcProductDefinitionShape(Name, Description, Representations)
        let mut out: Vec<Representation> = shape
            .arg(2)
            .refs()
            .into_iter()
            .filter_map(|r| representation(file, r))
            .collect();
        out.sort_by_key(|r| body_rank(&r.identifier));
        out
    }

    /// The representation of a product the import builds: the first body
    /// representation with 3D items.
    pub fn body(&self, file: &IfcFile, product: &Product) -> Option<Representation> {
        self.representations(file, product)
            .into_iter()
            .find(|r| schema::is_body_identifier(&r.identifier) && schema::is_body_type(&r.kind))
    }

    /// The representation a map holds: IfcRepresentationMap(MappingOrigin,
    /// MappedRepresentation).
    pub fn map_representation(&self, file: &IfcFile, map: u64) -> Option<Representation> {
        file.get(map)
            .and_then(|m| m.arg(1).as_ref())
            .and_then(|r| representation(file, r))
    }

    /// The style of a representation item, or of the material of the
    /// product.
    pub fn style(&self, item: u64, product: Option<&Product>) -> Option<&Style> {
        self.item_styles.get(&item).or_else(|| {
            product?
                .materials
                .iter()
                .find_map(|m| self.material_styles.get(m))
        })
    }

    /// The spatial element a product lies in: its container, else its
    /// aggregate's, else (an opening) its element's, else (a filling) its
    /// opening's; spatial elements are their own. None when it is in none.
    pub fn spatial_parent(&self, id: u64) -> Option<u64> {
        let mut seen = HashSet::new();
        let mut at = id;
        loop {
            if !seen.insert(at) {
                return None;
            }
            let p = self.products.get(&at)?;
            if p.class == ProductKind::Spatial && at != id {
                return Some(at);
            }
            if p.class == ProductKind::Space {
                // A space's contents go to the space's own spatial parent.
                at = p.container.or(p.parent)?;
                continue;
            }
            if let Some(c) = p.container {
                if self
                    .products
                    .get(&c)
                    .is_some_and(|c| c.class == ProductKind::Spatial)
                {
                    return Some(c);
                }
                at = c;
                continue;
            }
            if let Some(next) = p.parent.or(p.voids).or(p.fills) {
                if next == self.project.unwrap_or(u64::MAX) {
                    return None;
                }
                at = next;
                continue;
            }
            return None;
        }
    }

    /// A grid's axes (IfcGrid(..., UAxes, VAxes, WAxes)), in the project's
    /// coordinates: each axis's curve from its first point to its last.
    pub fn grid_axes(&self, file: &IfcFile, grid: &Product) -> (Vec<GridAxis>, Vec<String>) {
        let decoder = Decoder::new(file, &self.units);
        let mut axes = Vec::new();
        let mut notes = Vec::new();
        let Some(e) = file.get(grid.id) else {
            return (axes, notes);
        };
        for list in [7, 8, 9] {
            for a in e.arg(list).refs() {
                let Some(axis) = file.get(a) else {
                    continue;
                };
                // IfcGridAxis(AxisTag, AxisCurve, SameSense)
                let tag = axis.arg(0).as_str().unwrap_or_default().to_owned();
                match decoder.curve(axis.arg(1)) {
                    Ok(path) => {
                        let (Some(mut s), Some(mut t)) = (path.start(), path.end()) else {
                            notes.push(format!("grid axis {tag}: an empty curve"));
                            continue;
                        };
                        if axis.arg(2).as_bool() == Some(false) {
                            std::mem::swap(&mut s, &mut t);
                        }
                        axes.push(GridAxis {
                            tag,
                            start: math::apply(&grid.placement, s),
                            end: math::apply(&grid.placement, t),
                        });
                    }
                    Err(why) => notes.push(format!("grid axis {tag}: {why}")),
                }
            }
        }
        (axes, notes)
    }
}

fn arc_str<S: serde::Serializer>(value: &Arc<str>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(value)
}

fn body_rank(identifier: &str) -> usize {
    let upper = identifier.to_ascii_uppercase();
    schema::BODY_IDENTIFIERS
        .iter()
        .position(|b| *b == upper)
        .unwrap_or(schema::BODY_IDENTIFIERS.len())
}

fn representation(file: &IfcFile, id: u64) -> Option<Representation> {
    let r = file.get(id)?;
    // IfcShapeRepresentation(ContextOfItems, RepresentationIdentifier,
    // RepresentationType, Items)
    Some(Representation {
        id,
        identifier: r.arg(1).as_str().unwrap_or_default().to_owned(),
        kind: r.arg(2).as_str().unwrap_or_default().to_owned(),
        items: r.arg(3).refs(),
    })
}

/// Whether an instance is a product (see the module docs).
pub fn is_product(file: &IfcFile, e: &Entity) -> bool {
    if e.kind.starts_with("IFCREL")
        || e.kind.ends_with("TYPE")
        || e.kind.ends_with("STYLE")
        || e.args.len() < 7
        || e.arg(0).as_str().is_none()
    {
        return false;
    }
    let placement_ok = match e.arg(5) {
        Value::Null => true,
        v => v
            .as_ref()
            .and_then(|id| file.kind(id))
            .is_some_and(|k| k.ends_with("PLACEMENT")),
    };
    let shape_ok = match e.arg(6) {
        Value::Null => true,
        v => v
            .as_ref()
            .and_then(|id| file.kind(id))
            .is_some_and(|k| k == "IFCPRODUCTDEFINITIONSHAPE"),
    };
    placement_ok && shape_ok && !(e.arg(5).is_null() && e.arg(6).is_null() && !is_known(&e.kind))
}

/// Products that may have neither placement nor shape and still are
/// products (spatial elements, elements without geometry); other
/// instances with both unset are not taken for products.
fn is_known(kind: &str) -> bool {
    ProductKind::of(kind) != ProductKind::Element
        || kind.contains("ELEMENT")
        || matches!(
            kind,
            "IFCWALL"
                | "IFCWALLSTANDARDCASE"
                | "IFCSLAB"
                | "IFCBEAM"
                | "IFCCOLUMN"
                | "IFCDOOR"
                | "IFCWINDOW"
                | "IFCMEMBER"
                | "IFCPLATE"
                | "IFCROOF"
                | "IFCSTAIR"
                | "IFCSTAIRFLIGHT"
                | "IFCRAMP"
                | "IFCRAMPFLIGHT"
                | "IFCRAILING"
                | "IFCCOVERING"
                | "IFCCURTAINWALL"
                | "IFCFOOTING"
                | "IFCPILE"
                | "IFCFURNITURE"
                | "IFCCHIMNEY"
        )
}

/// The IfcMaterial instances a material definition stands for.
fn resolve_materials(file: &IfcFile, id: u64, depth: usize) -> Vec<u64> {
    let Some(e) = file.get(id) else {
        return Vec::new();
    };
    if depth > 8 {
        return Vec::new();
    }
    let all = |ids: Vec<u64>, depth: usize| -> Vec<u64> {
        ids.into_iter()
            .flat_map(|i| resolve_materials(file, i, depth + 1))
            .collect()
    };
    match &*e.kind {
        "IFCMATERIAL" => vec![id],
        // (ForLayerSet, ...)
        "IFCMATERIALLAYERSETUSAGE" | "IFCMATERIALPROFILESETUSAGE" => all(e.arg(0).refs(), depth),
        "IFCMATERIALPROFILESETUSAGETAPERING" => all(e.arg(0).refs(), depth),
        // IfcMaterialLayerSet(MaterialLayers, ...)
        "IFCMATERIALLAYERSET" => all(e.arg(0).refs(), depth),
        // IfcMaterialLayer(Material, ...)
        "IFCMATERIALLAYER" | "IFCMATERIALLAYERWITHOFFSETS" => all(e.arg(0).refs(), depth),
        // IfcMaterialList(Materials)
        "IFCMATERIALLIST" => all(e.arg(0).refs(), depth),
        // IfcMaterialConstituentSet(Name, Description, MaterialConstituents)
        "IFCMATERIALCONSTITUENTSET" => all(e.arg(2).refs(), depth),
        // IfcMaterialConstituent(Name, Description, Material, ...)
        "IFCMATERIALCONSTITUENT" => all(e.arg(2).refs(), depth),
        // IfcMaterialProfileSet(Name, Description, MaterialProfiles, ...)
        "IFCMATERIALPROFILESET" => all(e.arg(2).refs(), depth),
        // IfcMaterialProfile(Name, Description, Material, ...)
        "IFCMATERIALPROFILE" | "IFCMATERIALPROFILEWITHOFFSETS" => all(e.arg(2).refs(), depth),
        _ => Vec::new(),
    }
}

/// The surface colour of a styled item (IfcStyledItem(Item, Styles, Name)):
/// its first surface style's shading or rendering colour.
fn style_of(file: &IfcFile, styled: &Entity) -> Option<Style> {
    let mut todo: Vec<u64> = styled.arg(1).refs();
    let mut seen = HashSet::new();
    while let Some(s) = todo.pop() {
        if !seen.insert(s) {
            continue;
        }
        let e = file.get(s)?;
        match &*e.kind {
            // IFC2X3: IfcPresentationStyleAssignment(Styles)
            "IFCPRESENTATIONSTYLEASSIGNMENT" => todo.extend(e.arg(0).refs()),
            // IfcSurfaceStyle(Name, Side, Styles)
            "IFCSURFACESTYLE" => {
                let name = e.arg(0).as_str().unwrap_or_default().to_owned();
                for element in e.arg(2).refs() {
                    let Some(se) = file.get(element) else {
                        continue;
                    };
                    // IfcSurfaceStyleShading(SurfaceColour[, Transparency]);
                    // ...Rendering(SurfaceColour, Transparency, ...)
                    if se.is("IFCSURFACESTYLESHADING") || se.is("IFCSURFACESTYLERENDERING") {
                        let colour = file.deref(se.arg(0))?;
                        // IfcColourRgb(Name, Red, Green, Blue)
                        let c = [
                            colour.arg(1).as_f64()?,
                            colour.arg(2).as_f64()?,
                            colour.arg(3).as_f64()?,
                        ];
                        return Some(Style {
                            name,
                            color: c.map(|v| v.clamp(0.0, 1.0)),
                            transparency: se.arg(1).as_f64().unwrap_or(0.0).clamp(0.0, 1.0),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// The absolute matrix of an IfcObjectPlacement (local placements relative
/// to their placement, grid placements at their grid's intersections).
fn placement_of(
    decoder: &Decoder<'_>,
    model: &Model,
    id: u64,
    cache: &mut HashMap<u64, Result<Matrix, String>>,
    depth: usize,
) -> Result<Matrix, String> {
    if let Some(m) = cache.get(&id) {
        return m.clone();
    }
    if depth > 64 {
        return Err("placements nested too deeply (a cycle?)".to_owned());
    }
    let file = decoder.file;
    let e = file
        .get(id)
        .ok_or_else(|| format!("placement #{id} is not in the file"))?;
    let result = match &*e.kind {
        // (PlacementRelTo, RelativePlacement)
        "IFCLOCALPLACEMENT" => {
            let relative = decoder.placement(e.arg(1));
            match (relative, e.arg(0).as_ref()) {
                (Err(why), _) => Err(why),
                (Ok(rel), None) => Ok(rel),
                (Ok(rel), Some(parent)) => placement_of(decoder, model, parent, cache, depth + 1)
                    .map(|p| math::mul(&p, &rel)),
            }
        }
        // (PlacementLocation, PlacementRefDirection)
        "IFCGRIDPLACEMENT" => grid_placement(decoder, model, e, cache, depth),
        other => Err(format!("placements of type {other} are not read")),
    };
    cache.insert(id, result.clone());
    result
}

/// The point of an IfcVirtualGridIntersection(IntersectingAxes,
/// OffsetDistances) in the project's coordinates.
fn grid_point(
    decoder: &Decoder<'_>,
    model: &Model,
    intersection: &Entity,
    cache: &mut HashMap<u64, Result<Matrix, String>>,
    depth: usize,
) -> Result<[f64; 3], String> {
    let file = decoder.file;
    let axes = intersection.arg(0).refs();
    if axes.len() != 2 {
        return Err("a grid intersection without two axes".to_owned());
    }
    let offsets = intersection.arg(1).numbers().unwrap_or_default();
    // The grid of the axes: the IfcGrid that lists them.
    let grid = file
        .of_kind("IFCGRID")
        .into_iter()
        .find(|g| (7..10).any(|i| g.arg(i).refs().contains(&axes[0])))
        .ok_or("the grid of an intersection's axes is not found")?;
    let grid_matrix = match grid.arg(5).as_ref() {
        Some(p) => placement_of(decoder, model, p, cache, depth + 1)?,
        None => math::IDENTITY,
    };
    let mut lines = Vec::new();
    for (k, a) in axes.iter().enumerate() {
        let axis = file.get(*a).ok_or("a grid axis is not in the file")?;
        let path: Path = decoder.curve(axis.arg(1))?;
        let (Some(s), Some(t)) = (path.start(), path.end()) else {
            return Err("an empty grid axis".to_owned());
        };
        let (s, t) = if axis.arg(2).as_bool() == Some(false) {
            (t, s)
        } else {
            (s, t)
        };
        let d = math::normalize(math::sub(t, s)).ok_or("a grid axis of no length")?;
        // An offset moves the axis to its left (z × d).
        let left = math::cross([0.0, 0.0, 1.0], d);
        let offset = offsets.get(k).copied().unwrap_or(0.0) * model.units.length;
        lines.push((math::add(s, math::scale(left, offset)), d));
    }
    let ((p, u), (q, v)) = (lines[0], lines[1]);
    // p + s u = q + t v in the grid's plane.
    let det = u[0] * (-v[1]) - u[1] * (-v[0]);
    if det.abs() < 1e-12 {
        return Err("parallel grid axes do not intersect".to_owned());
    }
    let r = math::sub(q, p);
    let s = (r[0] * (-v[1]) - r[1] * (-v[0])) / det;
    let mut point = math::add(p, math::scale(u, s));
    point[2] = offsets.get(2).copied().unwrap_or(0.0) * model.units.length;
    Ok(math::apply(&grid_matrix, point))
}

fn grid_placement(
    decoder: &Decoder<'_>,
    model: &Model,
    e: &Entity,
    cache: &mut HashMap<u64, Result<Matrix, String>>,
    depth: usize,
) -> Result<Matrix, String> {
    let file = decoder.file;
    let location = file
        .deref(e.arg(0))
        .ok_or("a grid placement without its intersection")?;
    let origin = grid_point(decoder, model, location, cache, depth)?;
    let x = match file.deref(e.arg(1)) {
        Some(d) if d.is("IFCDIRECTION") => Some(decoder.direction(e.arg(1))?),
        Some(i) if i.is("IFCVIRTUALGRIDINTERSECTION") => Some(math::sub(
            grid_point(decoder, model, i, cache, depth)?,
            origin,
        )),
        _ => None,
    };
    Ok(math::frame(origin, None, x))
}

/// Points of a path in order, for reports.
pub fn path_points(path: &Path) -> Vec<[f64; 3]> {
    path.segments
        .iter()
        .flat_map(|s| match s {
            Seg::Line { start, .. } => vec![*start],
            other => {
                let mut v = other.sample(4);
                v.pop();
                v
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_building_model() {
        let file = IfcFile::parse(crate::testdata::BUILDING).expect("reads");
        let model = Model::read(&file);
        assert_eq!(model.project_name, "Test project");
        assert_eq!(model.products.len(), crate::testdata::BUILDING_PRODUCTS);
        assert_eq!(model.roots, vec![40]);
        let wall = &model.products[&100];
        assert_eq!(wall.class, ProductKind::Element);
        assert_eq!(wall.global_id, "1WALL00000000000000000");
        assert_eq!(wall.openings, vec![110]);
        assert_eq!(wall.volumes, vec![("NetVolume".to_owned(), 2.8e9)]);
        assert_eq!(model.spatial_parent(100), Some(44));
        // The opening by its wall, the window by its containment.
        assert_eq!(model.spatial_parent(110), Some(44));
        assert_eq!(model.spatial_parent(130), Some(44));
        assert_eq!(model.products[&110].class, ProductKind::Opening);
        assert_eq!(model.products[&130].fills, Some(110));
        assert_eq!(model.products[&130].type_object, Some(139));
        assert_eq!(model.map_types.get(&140), Some(&139));
        // Placements: the first floor 3000 up, the window in the wall.
        assert_eq!(
            math::origin(&model.products[&46].placement),
            [0.0, 0.0, 3000.0]
        );
        assert_eq!(model.products[&46].elevation, Some(3000.0));
        assert_eq!(
            math::origin(&model.products[&130].placement),
            [2000.0, 0.0, 1000.0]
        );
        let w2 = &model.products[&830].placement;
        assert_eq!(math::origin(w2), [1000.0, 0.0, 3000.0]);
        assert!((math::column(w2, 0)[1] - 1.0).abs() < 1e-12);
        assert_eq!(model.spatial_parent(700), Some(46));
        assert_eq!(model.products[&700].class, ProductKind::Space);
        // Styles and materials.
        assert_eq!(model.item_styles[&601].color, [1.0, 0.0, 0.0]);
        let slab = &model.products[&200];
        assert_eq!(slab.materials, vec![250]);
        assert_eq!(
            model.style(204, Some(slab)).map(|s| s.color),
            Some([0.5, 0.5, 0.5])
        );
        // Representations and the grid.
        let body = model.body(&file, wall).expect("a body");
        assert_eq!(body.items, vec![104]);
        let (axes, notes) = model.grid_axes(&file, &model.products[&900]);
        assert!(notes.is_empty());
        assert_eq!(axes.len(), 2);
        assert_eq!(axes[0].tag, "A");
        assert_eq!(axes[0].end, [10000.0, 0.0, 3000.0]);
        assert!(model.notes.is_empty(), "{:?}", model.notes);
    }
}
