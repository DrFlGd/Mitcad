// SPDX-License-Identifier: MIT
//! The elements: the ids the element table declares, and for each the
//! records the partitions hold, walked by the schema, as typed views
//! (category, class, level, family and type, name, transform, bounding
//! box), with the levels and the materials.
//!
//! Every value here is a field the schema names, read by its name:
//!
//! - the element header (`ElementHeader`): `m_categroryId` (the category,
//!   a built-in category's negative id), `m_classDef` (the element's
//!   class), `m_pBBox` (an `Outline`: the bounding box in model feet),
//!   `m_ownerViewId` (view-specific elements), `m_unplacedOwnerId`
//!   (members of a group's definition), `m_designOptionId`;
//! - the element (`Element` and its classes): `m_assocLevelId` (the
//!   level), the type (the first own ElementId field naming a type, a
//!   `Symbol`: `m_masterSymbolId` of family instances, `m_WallAttributesId`
//!   of walls, `m_floorAttributesId` of floors, ...), the instance's
//!   transform (`m_pInstanceInfo` → `m_Trf`, a 3×3 matrix and an origin);
//! - a type (`Symbol`): its name (`m_symbolInfo` → `m_name`), its family
//!   (`FamilySymbol.m_familyId` → the family's `m_name`) and its bounding
//!   box in its own coordinates (`FamilySymbol`'s `Outline`);
//! - a level (`Level`, a `DatumPlane`): its name (`m_text`) and elevation
//!   (`m_pSurface` → the plane's origin);
//! - a material (`MaterialElem.m_pMaterial` → `Material`): name, colour,
//!   transparency.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::building::{self, Building, Raw};
use crate::categories;
use crate::partition::{Partition, Slot};
use crate::schema::Schema;
use crate::walk::{self, Object, Record, Value};
use crate::{RvtError, RvtFile};

/// Feet to millimetres.
pub const FOOT_MM: f64 = 304.8;

/// An element the element table declares.
#[derive(Debug, Clone, Serialize)]
pub struct Declared {
    pub id: i64,
    /// The element that owns it (`m_OwningElementId`; -1 for none).
    pub owner: i64,
    /// Its history (`m_history`): the id it was created with
    /// (`m_originalElementId`) and the episode that created it
    /// (`m_creationDate`, an index into [`crate::RvtFile::episodes`]).
    pub original: Option<i64>,
    pub created: Option<u64>,
}

/// The element table (`Global/ElemTable`), walked by the schema: the
/// table's class and fields, then a 32-bit word.
pub fn element_table(file: &RvtFile, schema: &Schema) -> Result<Vec<Declared>, RvtError> {
    let data = file.inflated("/Global/ElemTable")?;
    let fail = |e: String| RvtError::Stream(format!("Global/ElemTable: {e}"));
    let (record, end) = walk::walk_prefix(schema, &data).map_err(|e| fail(e.to_string()))?;
    // The table, then one 32-bit word (not read).
    if end + 4 != data.len() {
        return Err(fail(format!(
            "the table's walk ends at {end} of {} bytes, not 4 before the end",
            data.len()
        )));
    }
    let list = record
        .root
        .get(schema, "m_elemArr")
        .and_then(Value::as_list)
        .ok_or_else(|| fail("no m_elemArr".into()))?;
    Ok(list
        .iter()
        .filter_map(Value::as_object)
        .map(|rec| Declared {
            id: rec.get(schema, "m_id").and_then(Value::as_id).unwrap_or(-1),
            owner: rec
                .get(schema, "m_OwningElementId")
                .and_then(Value::as_id)
                .unwrap_or(-1),
            original: history_of(schema, rec, "m_originalElementId"),
            created: history_of(schema, rec, "m_creationDate").map(|v| v as u64),
        })
        .collect())
}

/// A number of an element table record's history (`m_history`): the
/// field's id or integer, however deep its object holds it.
fn history_of(schema: &Schema, rec: &Object, field: &str) -> Option<i64> {
    fn first(v: &Value) -> Option<i64> {
        match v {
            Value::Id(id) => Some(*id),
            Value::Int(n) => Some(*n as i64),
            Value::Object(o) => o.fields.iter().find_map(|(_, _, v)| first(v)),
            _ => None,
        }
    }
    let history = rec.get(schema, "m_history")?.as_object()?;
    first(history.get(schema, field)?)
}

/// An element's header record, as read.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Header {
    pub class: Option<u16>,
    pub category: Option<i64>,
    pub family: Option<i64>,
    pub owner_view: Option<i64>,
    pub design_option: Option<i64>,
    pub unplaced_owner: Option<i64>,
    /// `RejectedViewRules.m_nVisibleViewFlags`.
    pub view_flags: Option<u64>,
    /// Min x, y, z, max x, y, z in feet.
    pub bbox: Option<[f64; 6]>,
}

fn some_id(v: Option<&Value>) -> Option<i64> {
    v.and_then(Value::as_id).filter(|id| *id != -1)
}

fn outline(schema: &Schema, record: &Record, value: Option<&Value>) -> Option<[f64; 6]> {
    let object = record.deref(value?)?;
    let v = object.get(schema, "m_minmax")?.floats();
    (v.len() == 6 && v.iter().all(|x| x.is_finite())).then(|| [v[0], v[1], v[2], v[3], v[4], v[5]])
}

impl Header {
    pub fn read(schema: &Schema, record: &Record) -> Header {
        let root = &record.root;
        let get = |name: &str| root.get(schema, name);
        Header {
            class: get("m_classDef")
                .and_then(Value::as_object)
                .and_then(|o| o.get(schema, "m_ref"))
                .and_then(|v| match v {
                    Value::Tag(t) => Some(*t),
                    _ => None,
                }),
            category: some_id(get("m_categroryId")),
            family: some_id(get("m_familyId")),
            owner_view: some_id(get("m_ownerViewId")),
            design_option: some_id(get("m_designOptionId")),
            unplaced_owner: some_id(get("m_unplacedOwnerId")),
            view_flags: get("m_viewRules")
                .and_then(Value::as_object)
                .and_then(|o| o.get(schema, "m_nVisibleViewFlags"))
                .and_then(Value::as_u64),
            bbox: outline(schema, record, get("m_pBBox")),
        }
    }
}

/// One element: what its records say.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Element {
    pub id: i64,
    /// The partition its records were read from (the last one that has
    /// them); None when no partition has a record of it.
    pub partition: Option<u32>,
    /// The element's class (the schema's name).
    pub class: Option<String>,
    pub category: Option<i64>,
    /// The built-in category's name (`OST_...`), when the category is one
    /// of [`categories::CATEGORIES`].
    pub category_name: Option<&'static str>,
    pub level: Option<i64>,
    /// Where the level was read: a field, a parameter, the host, ...
    pub level_field: Option<String>,
    pub type_id: Option<i64>,
    /// The field the type was read from.
    pub type_field: Option<String>,
    pub type_name: Option<String>,
    pub family_id: Option<i64>,
    pub family_name: Option<String>,
    /// The element's own name (types, families, levels, materials, views).
    pub name: Option<String>,
    pub owner_view: Option<i64>,
    pub design_option: Option<i64>,
    /// The group whose definition holds it (`m_unplacedOwnerId`).
    pub unplaced_owner: Option<i64>,
    pub host: Option<i64>,
    pub view_flags: Option<u64>,
    /// Bounding box in model feet: min x, y, z, max x, y, z.
    pub bbox: Option<[f64; 6]>,
    /// The instance's transform: the 3×3 matrix by rows, then the origin
    /// (feet).
    pub transform: Option<[f64; 12]>,
    /// The symbol the instance places (`InstanceInfo`'s `m_symbolId`).
    pub symbol: Option<i64>,
    /// Whether it is a type (its class derives from `Symbol`).
    pub is_type: bool,
    /// Why its records did not walk, if they did not.
    pub error: Option<String>,
    /// Its instance parameters as its record keeps them (mitcad#496), by
    /// parameter id: a built-in parameter's is negative, a project or
    /// shared parameter's names its `ParamElem`, whose definition gives its
    /// name and kind ([`crate::building::Building::param_defs`],
    /// mitcad#501).
    pub params: Vec<(i64, ParamValue)>,
}

/// An instance parameter's value (mitcad#496): text, a number in the
/// file's internal units (feet, radians, ...; its parameter's kind says
/// which, [`crate::params`]), an integer (a yes or no, a count, a choice)
/// or an element id.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ParamValue {
    Text(String),
    Number(f64),
    Integer(i64),
    Element(i64),
}

impl Element {
    /// `Family:Type:Id` when both names are known, else its own name
    /// (`Name:Id`), else `Class-Id`.
    pub fn display_name(&self) -> String {
        match (&self.family_name, &self.type_name, &self.name) {
            (Some(f), Some(t), _) => format!("{f}:{t}:{}", self.id),
            (_, _, Some(n)) => format!("{n}:{}", self.id),
            (_, Some(t), None) => format!("{t}:{}", self.id),
            _ => format!("{}-{}", self.class.as_deref().unwrap_or("Element"), self.id),
        }
    }
}

/// A level: name and elevation (feet).
#[derive(Debug, Clone, Serialize)]
pub struct Level {
    pub id: i64,
    pub name: String,
    pub elevation: f64,
    pub building_story: Option<bool>,
}

/// A material: name, colour (red, green, blue), transparency (0 to 100).
#[derive(Debug, Clone, Serialize)]
pub struct Material {
    pub id: i64,
    pub name: String,
    pub color: Option<[u8; 3]>,
    pub transparency: Option<f64>,
}

/// A partition's summary.
#[derive(Debug, Clone, Serialize)]
pub struct PartitionInfo {
    pub number: u32,
    pub format: u64,
    pub headers: usize,
    pub objects: usize,
    pub members: usize,
    pub stopped: Option<String>,
}

/// Everything read of the elements.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Model {
    pub declared: Vec<Declared>,
    pub partitions: Vec<PartitionInfo>,
    /// Every declared element (in the table's order), with what its
    /// records say.
    pub elements: Vec<Element>,
    pub levels: Vec<Level>,
    pub materials: Vec<Material>,
    /// Records of ids the table does not declare.
    pub undeclared: usize,
    /// Header records that did not walk, and object records that did not.
    pub header_errors: usize,
    pub object_errors: usize,
    /// Object records that walked to their end.
    pub objects_walked: usize,
    /// Object records whose own fields read but not all the objects
    /// after them (extensible storage data).
    pub objects_partial: usize,
    /// The types' bounding boxes in their own coordinates (feet; a
    /// family type's `m_outline`), by type id.
    #[serde(skip)]
    pub outlines: BTreeMap<i64, [f64; 6]>,
    /// The triangle meshes saved in elements' records, by element.
    #[serde(skip)]
    pub meshes: BTreeMap<i64, Vec<Mesh>>,
    /// The building elements' own parameters (walls, floors, roofs,
    /// columns, beams, openings, grids, links).
    #[serde(skip)]
    pub building: Building,
    /// The editing episodes (`Global/History`), for the GlobalIds.
    #[serde(skip)]
    pub episodes: Vec<[u8; 16]>,
    /// The geometry the file keeps of elements and types (their
    /// `GElement`s, [`crate::brep`]), by element.
    #[serde(skip)]
    pub geometry: BTreeMap<i64, crate::brep::GRep>,
    /// Geometry records that did not walk.
    pub geometry_errors: usize,
    /// Families' documents' element ids (a few), by family element.
    #[serde(skip)]
    pub family_docs: BTreeMap<i64, Vec<i64>>,
    /// Profile families' profiles (their documents' curve elements, feet,
    /// in the family's xy plane), by family element (mitcad#333).
    #[serde(skip)]
    pub profiles: BTreeMap<i64, Vec<crate::building::Curve>>,
}

/// Where an element's latest records are: (partition index, slot index).
type Latest = HashMap<i64, (usize, usize)>;

impl Model {
    pub fn read(file: &RvtFile, schema: &Schema) -> Result<Model, RvtError> {
        let mut model = Self::read_elements(file, schema)?;
        model.read_geometry(file, schema);
        model.read_profiles(file, schema);
        Ok(model)
    }

    /// The model without the geometry the file keeps and the profiles'
    /// curves ([`Model::geometry`] and [`Model::profiles`] empty): what
    /// listing a project's links needs (mitcad#413).
    pub fn read_elements(file: &RvtFile, schema: &Schema) -> Result<Model, RvtError> {
        let declared = element_table(file, schema)?;
        let partitions = file.partitions(schema)?;
        let mut model = Model::from_parts(schema, declared, &partitions);
        model.episodes = file.episodes();
        Ok(model)
    }

    pub fn from_parts(schema: &Schema, declared: Vec<Declared>, partitions: &[Partition]) -> Model {
        let mut latest: Latest = HashMap::new();
        for (p, partition) in partitions.iter().enumerate() {
            for (i, slot) in partition.headers.iter().enumerate() {
                latest.insert(slot.id, (p, i));
            }
        }
        let declared_ids: HashMap<i64, usize> = declared
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id, i))
            .collect();
        let mut model = Model {
            partitions: partitions
                .iter()
                .map(|p| PartitionInfo {
                    number: p.number,
                    format: p.format,
                    headers: p.headers.len(),
                    objects: p.objects.len(),
                    members: p.members,
                    stopped: p.stopped.clone(),
                })
                .collect(),
            undeclared: latest
                .keys()
                .filter(|id| !declared_ids.contains_key(id))
                .count(),
            ..Model::default()
        };

        // Pass 1: each element's records walked once, kept as the few
        // facts the views use.
        let mut headers: HashMap<i64, Header> = HashMap::new();
        let mut facts: HashMap<i64, Facts> = HashMap::new();
        let mut errors: HashMap<i64, String> = HashMap::new();
        for (&id, &(p, i)) in &latest {
            let partition = &partitions[p];
            match walk::walk(schema, partition.bytes(&partition.headers[i])) {
                Ok(record) => {
                    headers.insert(id, Header::read(schema, &record));
                }
                Err(e) => {
                    model.header_errors += 1;
                    errors.insert(id, format!("header: {e}"));
                }
            }
            let Some(slot) = object_slot(partition, i, id) else {
                errors
                    .entry(id)
                    .or_insert_with(|| "no object record".into());
                model.object_errors += 1;
                continue;
            };
            match walk::walk_partial(schema, partition.bytes(slot)) {
                (Some(record), end, None) if end == slot.size => {
                    model.objects_walked += 1;
                    facts.insert(id, Facts::read(schema, &record));
                }
                (Some(record), end, error) => {
                    // Its own fields read; an object after them did not
                    // (extensible storage data), or the walk ended early.
                    model.objects_partial += 1;
                    let why = error.map_or_else(
                        || {
                            format!(
                                "the walk ends {} bytes before the record's end",
                                slot.size - end
                            )
                        },
                        |e| e.to_string(),
                    );
                    errors
                        .entry(id)
                        .or_insert_with(|| format!("object, read in part: {why}"));
                    facts.insert(id, Facts::read(schema, &record));
                }
                (None, _, error) => {
                    model.object_errors += 1;
                    let why = error.map_or_else(String::new, |e| e.to_string());
                    errors.entry(id).or_insert_with(|| format!("object: {why}"));
                }
            }
        }
        let class_of = |id: i64| -> Option<u16> {
            headers
                .get(&id)
                .and_then(|h| h.class)
                .or_else(|| facts.get(&id).map(|f| f.class))
        };
        let is_type = |id: i64| class_of(id).is_some_and(|c| schema.is_a(c, "Symbol"));
        let is_level = |id: i64| class_of(id).is_some_and(|c| schema.is_a(c, "Level"));

        // Pass 2: the levels and the materials.
        for (&id, f) in &facts {
            if let Some(elevation) = f.elevation {
                model.levels.push(Level {
                    id,
                    name: f.name.clone().unwrap_or_default(),
                    elevation,
                    building_story: f.building_story,
                });
            }
            if let Some((color, transparency)) = f.material {
                model.materials.push(Material {
                    id,
                    name: f.name.clone().unwrap_or_default(),
                    color,
                    transparency,
                });
            }
        }
        model
            .levels
            .sort_by(|a, b| a.elevation.total_cmp(&b.elevation).then(a.id.cmp(&b.id)));
        model.materials.sort_by_key(|m| m.id);
        let name_of = |id: i64| facts.get(&id).and_then(|f| f.name.clone());
        let field_of = |id: i64, field: &str| -> Option<i64> {
            facts
                .get(&id)?
                .ids
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, v)| *v)
        };
        // Stair support types: carriages are named by the stair types'
        // carriage and intermediate support fields, stringers by their side
        // support fields.
        let mut stringers = std::collections::HashSet::new();
        let mut carriages = std::collections::HashSet::new();
        for f in facts
            .values()
            .filter(|f| schema.class_name(f.class) == "StairsType")
        {
            for (field, id) in &f.ids {
                match field.as_str() {
                    "m_leftSideSupportTypeId" | "m_rightSideSupportTypeId" => {
                        stringers.insert(*id);
                    }
                    "m_leftSideCarriageTypeId"
                    | "m_rightSideCarriageTypeId"
                    | "m_intermediateSupportTypeId" => {
                        carriages.insert(*id);
                    }
                    _ => {}
                }
            }
        }
        let monolithic = |id: Option<i64>| id.and_then(|id| facts.get(&id)?.monolithic);
        // A system type's family: stored nowhere, derived from its kind.
        let system_family = |type_id: i64, f: &Facts| -> Option<String> {
            let class = schema.class_name(f.class);
            let name = match class {
                "StairsRunType" => match f.monolithic {
                    Some(true) => "Monolithic Run",
                    _ => "Non-Monolithic Run",
                },
                "StairsLandingType" => match f.monolithic {
                    Some(true) => "Monolithic Landing",
                    _ => "Non-Monolithic Landing",
                },
                "StairsType" => match monolithic(field_of(type_id, "m_runTypeId")) {
                    Some(true) => "Cast-In-Place Stair",
                    _ => "Assembled Stair",
                },
                "StairsSupportType" if stringers.contains(&type_id) => "Stringer",
                "StairsSupportType" if carriages.contains(&type_id) => "Carriage",
                _ => categories::system_family(class)?,
            };
            Some(name.to_string())
        };
        let family_of_type = |type_id: i64| -> Option<(Option<i64>, Option<String>)> {
            let f = facts.get(&type_id)?;
            let name = match f.family {
                Some(family) => name_of(family),
                None => system_family(type_id, f),
            };
            Some((f.family, name))
        };

        // The level and where it was found.
        let own_level = |id: i64| -> Option<(i64, String)> {
            let f = facts.get(&id)?;
            if let Some(level) = f.level {
                return Some((level, "m_assocLevelId".into()));
            }
            if let Some((field, level)) = f.ids.iter().find(|(_, i)| is_level(*i)) {
                return Some((*level, field.clone()));
            }
            if let Some(level) = f.deep_ids.iter().copied().find(|i| is_level(*i)) {
                return Some((level, "a level its record names".into()));
            }
            f.id_params
                .iter()
                .find(|(_, v)| is_level(*v))
                .map(|(p, v)| (*v, format!("parameter {p}")))
        };
        // An element without a level of its own takes its host's, or that of
        // the first element it names that has one (a railing its stair's).
        let level_of = |id: i64| -> Option<(i64, String)> {
            let f = facts.get(&id)?;
            own_level(id)
                .or_else(|| {
                    let host = f.host?;
                    own_level(host).map(|(l, _)| (l, format!("its host {host}")))
                })
                .or_else(|| {
                    f.deep_ids
                        .iter()
                        .find_map(|i| own_level(*i).map(|(l, _)| (l, format!("element {i}"))))
                })
        };
        // The type of a part of a stair: the stair type's run or landing type.
        let part_type = |id: i64, class: &str| -> Option<(String, i64)> {
            let field = match class {
                "StairsRun" => "m_runTypeId",
                "StairsLanding" => "m_landingTypeId",
                _ => return None,
            };
            let stair_type = field_of(field_of(id, "m_stairsId")?, "m_typeId")?;
            Some((
                format!("m_stairsId.m_typeId.{field}"),
                field_of(stair_type, field)?,
            ))
        };

        // Pass 3: every declared element's view.
        for d in &declared {
            let mut element = Element {
                id: d.id,
                ..Element::default()
            };
            if let Some(&(p, _)) = latest.get(&d.id) {
                element.partition = Some(partitions[p].number);
            }
            if let Some(h) = headers.get(&d.id) {
                element.class = h.class.map(|c| schema.class_name(c).to_string());
                element.category = h.category;
                element.category_name = h.category.and_then(categories::name);
                element.owner_view = h.owner_view;
                element.design_option = h.design_option;
                element.unplaced_owner = h.unplaced_owner;
                element.view_flags = h.view_flags;
                element.bbox = h.bbox;
                element.is_type = h.class.is_some_and(|c| schema.is_a(c, "Symbol"));
            }
            element.error = errors.get(&d.id).cloned();
            if let Some(f) = facts.get(&d.id) {
                element.name = f.name.clone();
                if element.class.is_none() {
                    element.class = Some(schema.class_name(f.class).to_string());
                }
                if let Some((level, from)) = level_of(d.id) {
                    element.level = Some(level);
                    element.level_field = Some(from);
                }
                let named_type = |id: i64| is_type(id) && name_of(id).is_some();
                let class = schema.class_name(f.class);
                let found = part_type(d.id, class).or_else(|| {
                    f.ids
                        .iter()
                        .find(|(_, id)| *id != d.id && named_type(*id))
                        .cloned()
                });
                if let Some((field, id)) = found {
                    element.type_id = Some(id);
                    element.type_field = Some(field);
                    element.type_name = name_of(id);
                    if let Some((family_id, family_name)) = family_of_type(id) {
                        element.family_id = family_id;
                        element.family_name = family_name;
                    }
                }
                if element.is_type {
                    // A type's own family.
                    if let Some((family_id, family_name)) = family_of_type(d.id) {
                        element.family_id = family_id;
                        element.family_name = family_name;
                    }
                    element.type_name = element.name.clone();
                }
                element.host = f.host;
                element.symbol = f.symbol;
                element.transform = f.transform;
                // Its instance parameters (mitcad#496).
                element.params = f
                    .text_params
                    .iter()
                    .map(|(p, v)| (*p, ParamValue::Text(v.clone())))
                    .chain(
                        f.double_params
                            .iter()
                            .map(|(p, v)| (*p, ParamValue::Number(*v))),
                    )
                    .chain(
                        f.int_params
                            .iter()
                            .map(|(p, v)| (*p, ParamValue::Integer(*v))),
                    )
                    .chain(
                        f.id_params
                            .iter()
                            .map(|(p, v)| (*p, ParamValue::Element(*v))),
                    )
                    .collect();
            }
            model.elements.push(element);
        }
        model.declared = declared;
        model.outlines = facts
            .iter()
            .filter_map(|(&id, f)| Some((id, f.outline?)))
            .collect();
        model.family_docs = facts
            .iter()
            .filter(|(_, f)| !f.foreign.is_empty())
            .map(|(&id, f)| (id, f.foreign.clone()))
            .collect();
        model.meshes = facts
            .iter_mut()
            .filter(|(_, f)| !f.meshes.is_empty())
            .map(|(&id, f)| (id, std::mem::take(&mut f.meshes)))
            .collect();
        let mut link_symbols = BTreeMap::new();
        let mut ids: Vec<i64> = facts.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            if let Some(owner) = facts.get(&id).and_then(|f| f.owner) {
                model.building.owners.insert(id, owner);
            }
            match facts.get_mut(&id).and_then(|f| f.building.take()) {
                Some(Raw::LinkSymbol { name, path }) => {
                    link_symbols.insert(id, (name, path));
                }
                Some(raw) => model.building.add(id, raw),
                None => {}
            }
        }
        model.building.finish(&link_symbols);
        model
    }

    /// The geometry of the elements (not view-specific) and of the types:
    /// the record in the highest-numbered partition whose third chain has
    /// one (a partition need not repeat an element's geometry).
    pub fn read_geometry(&mut self, file: &RvtFile, schema: &Schema) {
        let wanted: std::collections::HashSet<i64> = self
            .elements
            .iter()
            .filter(|e| e.owner_view.is_none() && e.partition.is_some())
            .map(|e| e.id)
            .collect();
        for number in file.partition_numbers() {
            // (A partition that does not inflate has no geometry to give.)
            let Ok(docs) = file.documents(number, schema, false) else {
                self.geometry_errors += 1;
                continue;
            };
            let Some(own) = docs.documents.first() else {
                continue;
            };
            for slot in &own.geometry {
                if slot.size <= 2 || !wanted.contains(&slot.id) {
                    continue;
                }
                match walk::walk(schema, docs.bytes(slot)) {
                    Ok(record) => {
                        if let Some(g) = crate::brep::read(schema, &record)
                            && (!g.is_empty() || !g.curves.is_empty() || !g.notes.is_empty())
                        {
                            self.geometry.insert(slot.id, g);
                        }
                    }
                    Err(_) => self.geometry_errors += 1,
                }
            }
        }
    }

    /// The profiles of the profile families the project's types name
    /// (category -2003000): each family's document (the one after the
    /// partition's chains holding the family's document's element ids)
    /// read for its curve elements' curves (`CurveElem.m_pCurveDriver` →
    /// `m_pCrv`).
    pub fn read_profiles(&mut self, file: &RvtFile, schema: &Schema) {
        let families: std::collections::BTreeSet<i64> = self
            .elements
            .iter()
            .filter(|e| e.is_type && e.category == Some(-2_003_000))
            .filter_map(|e| e.family_id)
            .collect();
        if families.is_empty() {
            return;
        }
        // The partitions the families' records are in.
        let mut partitions: std::collections::BTreeSet<u32> = Default::default();
        for f in &families {
            if let Some(p) = self.element(*f).and_then(|e| e.partition) {
                partitions.insert(p);
            }
        }
        for number in partitions {
            let Ok(docs) = file.documents(number, schema, true) else {
                continue;
            };
            for doc in docs.documents.iter().skip(1) {
                let ids: std::collections::HashSet<i64> =
                    doc.objects.iter().map(|s| s.id).collect();
                // (Documents number their elements each in its own way: the
                // family whose ids the document holds nearly all of.)
                let share = |f: &i64| {
                    let fs = &self.family_docs[f];
                    fs.iter().filter(|x| ids.contains(x)).count() as f64 / fs.len() as f64
                };
                let Some(family) = families
                    .iter()
                    .filter(|f| !self.profiles.contains_key(f) && self.family_docs.contains_key(f))
                    .copied()
                    .max_by(|a, b| share(a).total_cmp(&share(b)))
                    .filter(|f| share(f) >= 0.8)
                else {
                    continue;
                };
                let mut curves = Vec::new();
                for slot in &doc.objects {
                    let Ok(record) = walk::walk(schema, docs.bytes(slot)) else {
                        continue;
                    };
                    if schema.class_name(record.root.class) == "CurveElem" {
                        curves.extend(building::curve_elem(schema, &record));
                    }
                }
                if !curves.is_empty() {
                    self.profiles.insert(family, curves);
                }
            }
        }
    }

    pub fn element(&self, id: i64) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }

    /// Where a multistory stair repeats an element of one of its stairs
    /// (mitcad#391): the stair it belongs to (through its owners, see
    /// [`crate::building::Building::owners`]) and per copy the level and
    /// how far up (feet) the copy lies, the level's elevation less the
    /// stair's base level's. Empty for an element of no repeated stair.
    pub fn multistory_copies(&self, id: i64) -> Option<(i64, Vec<(i64, f64)>)> {
        let b = &self.building;
        let mut at = id;
        for _ in 0..6 {
            if let Some(m) = b.multistory.get(&at) {
                let base = self.level(m.base_level)?.elevation;
                let copies = m
                    .levels
                    .iter()
                    .filter_map(|l| Some((*l, self.level(*l)?.elevation - base)))
                    .filter(|(_, dz)| dz.abs() > 1e-9)
                    .collect();
                return Some((at, copies));
            }
            at = *b.owners.get(&at)?;
        }
        None
    }

    pub fn level(&self, id: i64) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }

    /// An element's IFC GlobalId, from the episode that created it
    /// ([`crate::info::ifc_global_id`]); None when the table or the
    /// history does not give it.
    pub fn global_id(&self, id: i64) -> Option<String> {
        let d = self.declared.iter().find(|d| d.id == id)?;
        self.global_id_of(d)
    }

    /// Every declared element's GlobalId, where known.
    pub fn global_ids(&self) -> HashMap<i64, String> {
        self.declared
            .iter()
            .filter_map(|d| Some((d.id, self.global_id_of(d)?)))
            .collect()
    }

    fn global_id_of(&self, d: &Declared) -> Option<String> {
        // The history lists the episodes newest first.
        let created = usize::try_from(d.created?).ok()?;
        let episode = self
            .episodes
            .get(self.episodes.len().checked_sub(created + 1)?)?;
        Some(crate::info::ifc_global_id(
            episode,
            d.original.unwrap_or(d.id),
        ))
    }
}

/// The object record that goes with header `index` (the same index, or
/// found by id when the chains differ).
fn object_slot(partition: &Partition, index: usize, id: i64) -> Option<&Slot> {
    match partition.objects.get(index) {
        Some(slot) if slot.id == id => Some(slot),
        _ => partition.objects.iter().find(|s| s.id == id),
    }
}

/// What the views take of an element's object record.
struct Facts {
    class: u16,
    name: Option<String>,
    /// `m_assocLevelId`.
    level: Option<i64>,
    /// The ElementId fields of its own classes (not `Element`'s), in
    /// order: (field name, id).
    ids: Vec<(String, i64)>,
    /// A family type's family (`m_familyId`).
    family: Option<i64>,
    host: Option<i64>,
    symbol: Option<i64>,
    transform: Option<[f64; 12]>,
    /// A family type's box in its own coordinates.
    outline: Option<[f64; 6]>,
    /// A level's elevation.
    elevation: Option<f64>,
    building_story: Option<bool>,
    /// A material's colour and transparency.
    material: Option<(Option<[u8; 3]>, Option<f64>)>,
    /// Its ElementId parameters: (parameter, value).
    id_params: Vec<(i64, i64)>,
    /// Its text parameters: (parameter, value).
    text_params: Vec<(i64, String)>,
    /// Its number parameters: (parameter, value) (mitcad#496).
    double_params: Vec<(i64, f64)>,
    /// Its integer parameters: (parameter, value) (mitcad#501).
    int_params: Vec<(i64, i64)>,
    /// Every element id the record names (its own fields first, then the
    /// objects behind its pointers), in order, without repeats.
    deep_ids: Vec<i64>,
    /// `m_hasMonolithicSupport` of a stair run or landing type.
    monolithic: Option<bool>,
    /// A family's document's element ids (`FamilyDocument.m_big2SmallMap2`
    /// → `ForeignElemRef.m_id64`), a few.
    foreign: Vec<i64>,
    /// The triangle meshes its record holds (`GPolyMesh`).
    meshes: Vec<Mesh>,
    /// What it gives the building model.
    building: Option<Raw>,
    /// What it belongs to as an element of a stair (mitcad#391).
    owner: Option<i64>,
}

/// Element ids a value holds, appended in order.
fn collect_ids(value: &Value, out: &mut Vec<i64>) {
    if out.len() >= 512 {
        return;
    }
    match value {
        Value::Id(id) if *id > 0 && !out.contains(id) => out.push(*id),
        Value::Object(o) => o.fields.iter().for_each(|(_, _, v)| collect_ids(v, out)),
        Value::List(items) => items.iter().for_each(|v| collect_ids(v, out)),
        _ => {}
    }
}

impl Facts {
    fn read(schema: &Schema, record: &Record) -> Facts {
        let root = &record.root;
        let element = schema.tag("Element");
        let ids = root
            .fields
            .iter()
            .filter(|(class, _, _)| Some(*class) != element)
            .filter_map(|(class, index, value)| {
                let id = value.as_id().filter(|id| *id != -1)?;
                let field = schema.class(*class)?.fields.get(usize::from(*index))?;
                Some((field.name.clone(), id))
            })
            .collect();
        let mut facts = Facts {
            class: root.class,
            name: own_name(schema, record),
            level: some_id(root.get(schema, "m_assocLevelId")),
            ids,
            family: some_id(root.get_in(schema, "FamilySymbol", "m_familyId")),
            host: some_id(root.get(schema, "m_hostId")),
            symbol: None,
            transform: None,
            outline: None,
            elevation: None,
            building_story: None,
            material: None,
            id_params: Vec::new(),
            text_params: Vec::new(),
            double_params: Vec::new(),
            int_params: Vec::new(),
            deep_ids: Vec::new(),
            monolithic: root
                .get(schema, "m_hasMonolithicSupport")
                .and_then(Value::as_bool),
            meshes: Vec::new(),
            foreign: Vec::new(),
            building: None,
            owner: building::owner(schema, record),
        };
        let own_id = root.get(schema, "m_id").and_then(Value::as_id);
        if let Some(id) = own_id {
            facts.building = building::read(schema, record, id);
        }
        for (class, _, value) in &root.fields {
            if Some(*class) != element {
                collect_ids(value, &mut facts.deep_ids);
            }
        }
        for object in &record.pointees {
            object
                .fields
                .iter()
                .for_each(|(_, _, v)| collect_ids(v, &mut facts.deep_ids));
        }
        facts.deep_ids.retain(|id| Some(*id) != own_id);
        if let Some(doc) = record
            .pointees
            .iter()
            .find(|o| schema.class_name(o.class) == "FamilyDocument")
        {
            for pair in doc
                .get(schema, "m_big2SmallMap2")
                .and_then(Value::as_list)
                .unwrap_or_default()
                .iter()
                .take(32)
            {
                let id = pair
                    .as_object()
                    .and_then(|p| p.fields.get(1))
                    .and_then(|(_, _, v)| v.as_object())
                    .and_then(|r| r.fields.first())
                    .and_then(|(_, _, v)| v.as_u64());
                if let Some(id) = id {
                    facts.foreign.push(id as i64);
                }
            }
        }
        for object in record
            .pointees
            .iter()
            .filter(|o| schema.is_a(o.class, "GPolyMesh"))
        {
            if let Some(mesh) = object
                .get(schema, "m_pFacetedTopology")
                .and_then(|v| record.deref(v))
                .and_then(|t| Mesh::read(schema, t))
            {
                facts.meshes.push(mesh);
            }
        }
        let params = |field: &str| -> Vec<(i64, Value)> {
            let Some(set) = root.get(schema, field).and_then(|v| record.deref(v)) else {
                return Vec::new();
            };
            set.get(schema, "m_paramSet")
                .and_then(Value::as_list)
                .unwrap_or_default()
                .iter()
                .filter_map(|p| {
                    let p = p.as_object()?;
                    Some((
                        p.get(schema, "m_paramId")?.as_id()?,
                        p.get(schema, "m_value")?.clone(),
                    ))
                })
                .collect()
        };
        facts.id_params = params("m_pParamValueSetElementId")
            .into_iter()
            .filter_map(|(p, v)| Some((p, v.as_id().filter(|id| *id != -1)?)))
            .collect();
        facts.text_params = params("m_pParamValueSetAString")
            .into_iter()
            .filter_map(|(p, v)| Some((p, v.as_text()?.to_string())))
            .collect();
        facts.double_params = params("m_pParamValueSetDouble")
            .into_iter()
            .filter_map(|(p, v)| Some((p, v.as_f64()?)))
            .collect();
        facts.int_params = params("m_pParamValueSetInt")
            .into_iter()
            .filter_map(|(p, v)| Some((p, i64::from(v.as_u64()? as u32 as i32))))
            .collect();
        if let Some(info) = root
            .get(schema, "m_pInstanceInfo")
            .and_then(|v| record.deref(v))
        {
            facts.symbol = some_id(info.get(schema, "m_symbolId"));
            facts.transform = info
                .get(schema, "m_Trf")
                .and_then(Value::as_object)
                .and_then(|trf| {
                    let mut m = trf.get(schema, "m_3x3")?.floats();
                    m.extend(trf.get(schema, "m_or")?.floats());
                    (m.len() == 12).then(|| std::array::from_fn(|i| m[i]))
                });
        }
        if schema.is_a(root.class, "FamilySymbol") {
            facts.outline = outline(schema, record, root.get(schema, "m_outline"));
        }
        if schema.is_a(root.class, "Level") {
            let surface = root.get(schema, "m_pSurface").and_then(|v| record.deref(v));
            facts.elevation = surface
                .and_then(|s| s.get(schema, "m_origin"))
                .map(Value::floats)
                .and_then(|o| o.get(2).copied());
            facts.building_story = root
                .get(schema, "m_isBuildingStory")
                .and_then(Value::as_bool);
        }
        if schema.is_a(root.class, "MaterialElem")
            && let Some(material) = root
                .get(schema, "m_pMaterial")
                .and_then(|v| record.deref(v))
        {
            let color = material
                .get(schema, "m_color")
                .and_then(Value::as_u64)
                .map(|c| {
                    [
                        (c & 0xff) as u8,
                        ((c >> 8) & 0xff) as u8,
                        ((c >> 16) & 0xff) as u8,
                    ]
                });
            facts.material = Some((
                color,
                material
                    .get(schema, "m_transparency")
                    .and_then(Value::as_f64),
            ));
        }
        facts
    }
}

/// A triangle mesh an element's record holds (`GPolyMesh` →
/// `m_pFacetedTopology`, a `FacetedTopology` of `f32` points in model feet
/// and `u16` vertex triples).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mesh {
    pub points: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

impl Mesh {
    fn read(schema: &Schema, topology: &Object) -> Option<Mesh> {
        let triples = |field: &str| -> Option<Vec<Vec<f64>>> {
            Some(
                topology
                    .get(schema, field)?
                    .as_list()?
                    .iter()
                    .map(Value::floats)
                    .collect(),
            )
        };
        let points: Vec<[f64; 3]> = triples("m_pointsArr")?
            .into_iter()
            .filter(|p| p.len() == 3)
            .map(|p| [p[0], p[1], p[2]])
            .collect();
        let triangles: Vec<[u32; 3]> = triples("m_facetsArr")?
            .into_iter()
            .filter(|t| t.len() == 3)
            .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
            .filter(|t| t.iter().all(|i| (*i as usize) < points.len()))
            .collect();
        (!triangles.is_empty()).then_some(Mesh { points, triangles })
    }

    /// The box of its points: min x, y, z, max x, y, z.
    pub fn bounds(&self) -> Option<[f64; 6]> {
        let first = self.points.first()?;
        let mut b = [first[0], first[1], first[2], first[0], first[1], first[2]];
        for p in &self.points {
            for k in 0..3 {
                b[k] = b[k].min(p[k]);
                b[k + 3] = b[k + 3].max(p[k]);
            }
        }
        Some(b)
    }
}

/// A name the element's own class gives it: a type's (`m_symbolInfo` →
/// `m_name`), a family's (`m_name`), a level's or grid's (`m_text`), a
/// material's (`m_pMaterial` → `m_name`), a view's.
fn own_name(schema: &Schema, record: &Record) -> Option<String> {
    let root = &record.root;
    let text = |o: &Object, field: &str| {
        o.get(schema, field)
            .and_then(Value::as_text)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
    };
    if let Some(info) = root
        .get(schema, "m_symbolInfo")
        .and_then(|v| record.deref(v))
    {
        return text(info, "m_name");
    }
    if let Some(material) = root
        .get(schema, "m_pMaterial")
        .and_then(|v| record.deref(v))
    {
        return text(material, "m_name");
    }
    if schema.is_a(root.class, "DatumPlane") {
        return text(root, "m_text");
    }
    text(root, "m_name").or_else(|| text(root, "m_viewName"))
}

/// Elements by category: counts, for reports.
pub fn category_counts(elements: &[Element]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for e in elements {
        let key = match (e.category_name, e.category) {
            (Some(n), _) => n.to_string(),
            (None, Some(c)) => format!("category {c}"),
            (None, None) => "no category".to_string(),
        };
        *out.entry(key).or_default() += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testfile;

    #[test]
    fn the_written_project_reads() {
        let file = RvtFile::parse(testfile::project()).unwrap();
        let schema = file.schema().unwrap();
        let model = Model::read(&file, &schema).unwrap();
        assert_eq!(model.header_errors, 0);
        assert_eq!(model.object_errors, 0);
        let ids: Vec<i64> = model.declared.iter().map(|d| d.id).collect();
        assert_eq!(ids, testfile::DECLARED);
        assert_eq!(model.elements.len(), ids.len());
        // The levels with their elevations, lowest first.
        let levels: Vec<(&str, f64)> = model
            .levels
            .iter()
            .map(|l| (l.name.as_str(), l.elevation))
            .collect();
        assert_eq!(levels, vec![("Ground", 0.0), ("Upper", 10.0)]);
        // The column: its category, level, type, family and transform; its
        // newer record (partition 7) wins over the old one.
        let column = model.element(testfile::COLUMN).unwrap();
        assert_eq!(column.partition, Some(7));
        assert_eq!(column.class.as_deref(), Some("FamilyInstance"));
        assert_eq!(column.category, Some(categories::OST_COLUMNS));
        assert_eq!(column.category_name, Some("OST_Columns"));
        assert_eq!(column.level, Some(testfile::LEVEL_UPPER));
        assert_eq!(column.type_id, Some(testfile::SYMBOL));
        assert_eq!(column.type_field.as_deref(), Some("m_masterSymbolId"));
        assert_eq!(column.type_name.as_deref(), Some("24 x 24"));
        assert_eq!(column.family_name.as_deref(), Some("Square Column"));
        assert_eq!(
            column.display_name(),
            format!("Square Column:24 x 24:{}", testfile::COLUMN)
        );
        let t = column.transform.unwrap();
        assert_eq!(&t[9..], &[20.0, 30.0, 10.0]);
        assert_eq!(column.bbox.unwrap()[3], 21.0);
        // The wall: a system family's type, its family by the type's kind.
        let wall = model.element(testfile::WALL).unwrap();
        assert_eq!(wall.type_name.as_deref(), Some("Generic 8"));
        assert_eq!(wall.family_name.as_deref(), Some("Basic Wall"));
        assert_eq!(wall.level, Some(testfile::LEVEL_GROUND));
        assert!(wall.transform.is_none());
        // The type itself.
        let symbol = model.element(testfile::SYMBOL).unwrap();
        assert!(symbol.is_type);
        assert_eq!(symbol.family_name.as_deref(), Some("Square Column"));
        // The material.
        assert_eq!(model.materials[0].name, "Concrete");
        assert_eq!(model.materials[0].color, Some([0x80, 0x40, 0x20]));
        // A declared element without records is listed, without a class.
        let bare = model.element(testfile::NO_RECORD).unwrap();
        assert!(bare.class.is_none() && bare.partition.is_none());
        assert_eq!(model.undeclared, 1);
        // The family, its type and the column.
        assert_eq!(category_counts(&model.elements)["OST_Columns"], 3);
        // The surface's saved mesh, in model feet.
        let mesh = &model.meshes[&testfile::SURFACE];
        assert_eq!(mesh.len(), 1);
        assert_eq!(mesh[0].points.len(), 5);
        assert_eq!(mesh[0].triangles[1], [1, 2, 4]);
        assert_eq!(mesh[0].bounds(), Some([0.0, 0.0, 0.0, 4.0, 4.0, 1.0]));
    }
}
