// SPDX-License-Identifier: MIT
//! Materials kept in the document (mitcad#640, part 2), next to the
//! built-in ones (`crate::material`), and what a body is made of.
//!
//! A body names its material by id (`set_body_material`); one without
//! uses its component's default material (`set_component_material`): the
//! nearest component up the occurrence path that places it, then the
//! root's, then the built-in default, steel. Without a path (a body's mass
//! in the `properties` query) only the body's own component and the root
//! count.
//!
//! Setting a body's material also sets its appearance to the material's
//! (`appearance: "replace"`), keeps the body's (`"keep"`), or, when left
//! out, replaces it unless the body's appearance was set by hand: one that
//! is not its current material's. The commands are undo steps that
//! recompute nothing; the design file keeps the document's materials and
//! the components' defaults.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Value, json};

use super::{DocState, Document, ModelError, invalid};
use crate::ids::{BodyUid, ComponentUid, FeatureUid, OccurrenceUid};
use crate::kernel::Kernel;
use crate::material::{self, DEFAULT_MATERIAL, Material, MaterialChange};
use crate::print_library::{BasedOn, ItemData, ItemKind};
use crate::print_profile::new_item_name;

/// What `set_body_material` does with the body's appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppearanceChoice {
    /// The material's appearance replaces the body's.
    Replace,
    /// The body keeps its appearance.
    Keep,
    /// Replace unless the body's appearance was set by hand.
    #[default]
    Auto,
}

impl AppearanceChoice {
    pub fn parse(text: &str) -> Result<Self, ModelError> {
        match text {
            "replace" => Ok(Self::Replace),
            "keep" => Ok(Self::Keep),
            _ => Err(invalid(format!(
                "appearance must be \"replace\" or \"keep\", got \"{text}\""
            ))),
        }
    }
}

/// Where a body's material comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialSource {
    /// Its own.
    Body,
    /// A component's default: the body's own component's or one up the
    /// occurrence path (the root's last).
    Component(ComponentUid),
    /// None is set anywhere: the built-in default.
    Default,
}

/// What deleting a material changed: the bodies and the components whose
/// default it was, which use another one now (the built-in of the same id
/// when the deleted one took its place, else none of their own).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedMaterial {
    pub bodies: Vec<BodyUid>,
    pub components: Vec<ComponentUid>,
}

impl DocState {
    /// A material of the document or the built-in one by id; the
    /// document's takes the place of a built-in of the same id.
    pub(crate) fn material(&self, id: &str) -> Option<&Material> {
        self.materials
            .iter()
            .find(|m| m.id == id)
            .or_else(|| material::built_in_material(id))
    }

    /// Copies the document materials `needed` (by id) from another
    /// document's state: an id the target has with other fields gets a new
    /// one (`id_1`, ...), a name taken another; returns the source's ids
    /// with the copies' where they differ.
    pub(crate) fn copy_materials(
        &mut self,
        source: &DocState,
        needed: &BTreeSet<String>,
    ) -> BTreeMap<String, String> {
        let mut mapped = BTreeMap::new();
        for id in needed {
            let Some(theirs) = source.materials.iter().find(|m| &m.id == id) else {
                continue;
            };
            if self.materials.iter().any(|m| m == theirs) {
                continue;
            }
            let mut copy = theirs.clone();
            if self.materials.iter().any(|m| m.id == copy.id) {
                copy.id = (1..)
                    .map(|n| format!("{id}_{n}"))
                    .find(|candidate| self.materials.iter().all(|m| &m.id != candidate))
                    .expect("a free id");
                mapped.insert(id.clone(), copy.id.clone());
            }
            if self.material_name_taken(&copy.name, &copy.id, None) {
                let base = copy.name.clone();
                copy.name = (1..)
                    .map(|n| format!("{base} ({n})"))
                    .find(|name| !self.material_name_taken(name, &copy.id, None))
                    .expect("a free name");
            }
            self.materials.push(copy);
        }
        mapped
    }

    /// Whether another material shows the name: a document material (but
    /// the one at `own`), or a built-in that `id` does not replace.
    pub(crate) fn material_name_taken(&self, name: &str, id: &str, own: Option<usize>) -> bool {
        let overridden: Vec<&str> = self.materials.iter().map(|m| m.id.as_str()).collect();
        material::built_in()
            .iter()
            .filter(|m| m.id != id && !overridden.contains(&m.id.as_str()))
            .any(|m| m.name == name)
            || self
                .materials
                .iter()
                .enumerate()
                .any(|(j, m)| Some(j) != own && m.id != id && m.name == name)
    }
}

impl<K: Kernel> Document<K> {
    /// The document's own materials, in the order they were made.
    pub fn custom_materials(&self) -> &[Material] {
        &self.state.materials
    }

    /// A material of the document or a built-in one by id.
    pub fn material(&self, id: &str) -> Option<&Material> {
        self.state.material(id)
    }

    /// A component's default material id (C0: the root's).
    pub fn component_material(&self, component: ComponentUid) -> Option<&str> {
        self.state
            .component_materials
            .get(&component)
            .map(String::as_str)
    }

    /// The material a body is made of, and where it comes from: its own,
    /// else the nearest component's default up `path` (the occurrences from
    /// the root to the body's component), else the root's, else steel.
    /// Without a path, the body's own component and the root.
    pub fn body_material(
        &self,
        uid: BodyUid,
        path: Option<&[OccurrenceUid]>,
    ) -> (&Material, MaterialSource) {
        if let Some(found) = self
            .state
            .body_attributes
            .get(&uid)
            .and_then(|a| a.material.as_deref())
            .and_then(|id| self.material(id))
        {
            return (found, MaterialSource::Body);
        }
        let mut chain: Vec<ComponentUid> = match path {
            Some(path) => path
                .iter()
                .rev()
                .filter_map(|o| self.state.assembly.occurrence(*o).map(|o| o.component))
                .collect(),
            None => self
                .body_component(uid)
                .or_else(|| self.feature_component(uid.feature))
                .into_iter()
                .collect(),
        };
        chain.push(ComponentUid::ROOT);
        for component in chain {
            if let Some(found) = self
                .component_material(component)
                .and_then(|id| self.material(id))
            {
                return (found, MaterialSource::Component(component));
            }
        }
        (
            self.material(DEFAULT_MATERIAL)
                .expect("the default material is built in"),
            MaterialSource::Default,
        )
    }

    /// The component of a body's creating feature (for bodies not at the
    /// marker).
    fn feature_component(&self, feature: FeatureUid) -> Option<ComponentUid> {
        self.state.entry(feature).map(|f| f.component)
    }

    /// Whether a body's appearance was set by hand: it has one, and it is
    /// not the appearance of the material it is made of.
    pub fn appearance_set_by_hand(&self, uid: BodyUid) -> bool {
        let Some(own) = self
            .state
            .body_attributes
            .get(&uid)
            .and_then(|a| a.appearance.as_deref())
        else {
            return false;
        };
        self.body_material(uid, None).0.appearance.as_deref() != Some(own)
    }

    /// Sets a body's material by id (None: its component's default), and
    /// its appearance as `choice` says: one undo step, which recomputes
    /// nothing, unless nothing changes.
    pub fn set_body_material_with(
        &mut self,
        uid: BodyUid,
        material: Option<&str>,
        choice: AppearanceChoice,
    ) -> Result<(), ModelError> {
        if let Some(id) = material
            && self.material(id).is_none()
        {
            let mut known: Vec<&str> = material::built_in().iter().map(|m| m.id.as_str()).collect();
            for m in &self.state.materials {
                if !known.contains(&m.id.as_str()) {
                    known.push(&m.id);
                }
            }
            return Err(invalid(format!(
                "unknown material '{id}' (known: {})",
                known.join(", ")
            )));
        }
        if self.body_shape(uid).is_none() {
            return Err(invalid(format!(
                "body {uid} does not exist at the timeline marker"
            )));
        }
        let replace = match choice {
            AppearanceChoice::Replace => true,
            AppearanceChoice::Keep => false,
            AppearanceChoice::Auto => !self.appearance_set_by_hand(uid),
        };
        let mut attributes = self.body_attributes(uid);
        attributes.material = material.map(str::to_owned);
        if replace {
            // The appearance of the material it is made of now: its own,
            // else its components'.
            attributes.appearance = match &attributes.material {
                Some(id) => self.material(id).and_then(|m| m.appearance.clone()),
                None => self.inherited_material(uid).appearance.clone(),
            };
        }
        if attributes == self.body_attributes(uid) {
            return Ok(());
        }
        let label = format!("Set Material of {}", self.body_name(uid));
        self.apply_with(|state| {
            if attributes.is_default() {
                state.body_attributes.remove(&uid);
            } else {
                state.body_attributes.insert(uid, attributes);
            }
            Ok((label, (), false))
        })
    }

    /// The bodies of `bodies` whose appearance was set by hand and would
    /// change if they got `material` (None: their component's default)
    /// with its appearance: those the application asks about before
    /// replacing.
    pub fn appearance_conflicts(
        &self,
        bodies: &[BodyUid],
        material: Option<&str>,
    ) -> Result<Vec<BodyUid>, ModelError> {
        let new = match material {
            Some(id) => Some(
                self.material(id)
                    .ok_or_else(|| invalid(format!("unknown material '{id}'")))?,
            ),
            None => None,
        };
        Ok(bodies
            .iter()
            .copied()
            .filter(|uid| {
                let own = self
                    .state
                    .body_attributes
                    .get(uid)
                    .and_then(|a| a.appearance.as_deref());
                let target = match new {
                    Some(m) => m.appearance.as_deref(),
                    None => self.inherited_material(*uid).appearance.as_deref(),
                };
                self.appearance_set_by_hand(*uid) && own != target
            })
            .collect())
    }

    /// The material a body has from its components (not its own).
    fn inherited_material(&self, uid: BodyUid) -> &Material {
        let component = self
            .body_component(uid)
            .or_else(|| self.feature_component(uid.feature));
        component
            .into_iter()
            .chain([ComponentUid::ROOT])
            .find_map(|c| self.component_material(c).and_then(|id| self.material(id)))
            .or_else(|| self.material(DEFAULT_MATERIAL))
            .expect("the default material is built in")
    }

    /// Sets a component's default material (C0: the root's; None takes it
    /// away): one undo step, which recomputes nothing, unless nothing
    /// changes.
    pub fn set_component_material(
        &mut self,
        component: ComponentUid,
        material: Option<&str>,
    ) -> Result<(), ModelError> {
        if !component.is_root() && self.state.assembly.component(component).is_none() {
            return Err(invalid(format!("there is no component {component}")));
        }
        if let Some(id) = material
            && self.material(id).is_none()
        {
            return Err(invalid(format!("unknown material '{id}'")));
        }
        if self.component_material(component) == material {
            return Ok(());
        }
        let label = format!(
            "Set Default Material of {}",
            self.state.assembly.name(component)
        );
        let material = material.map(str::to_owned);
        self.apply_with(|state| {
            match material {
                Some(id) => {
                    state.component_materials.insert(component, id);
                }
                None => {
                    state.component_materials.remove(&component);
                }
            }
            Ok((label, (), false))
        })
    }

    /// The `materials` query: the built-in materials the document does not
    /// replace, then the document's, each with its fields, `built_in`,
    /// `replaces_built_in` (a document material with a built-in's id), its
    /// `class` (stored or worked out, mitcad#655), the bodies that name it
    /// and the components whose default it is.
    pub(crate) fn materials_json(&self) -> Value {
        let replaced: Vec<&str> = self.state.materials.iter().map(|m| m.id.as_str()).collect();
        let built_in = material::built_in()
            .iter()
            .filter(|m| !replaced.contains(&m.id.as_str()))
            .map(|m| (m, true));
        let own = self.state.materials.iter().map(|m| (m, false));
        Value::Array(
            built_in
                .chain(own)
                .map(|(m, built_in)| {
                    let mut value = serde_json::to_value(m).expect("materials serialize");
                    value["built_in"] = json!(built_in);
                    value["class"] = json!(material::material_class(m).id());
                    if !built_in && material::built_in_material(&m.id).is_some() {
                        value["replaces_built_in"] = json!(true);
                    }
                    if let Some(color) = m
                        .appearance
                        .as_deref()
                        .and_then(|a| self.appearance(a))
                        .map(|a| a.display_color())
                    {
                        value["display_color"] = json!(color);
                    }
                    value["bodies"] = json!(self.bodies_with_material(&m.id));
                    value["components"] = json!(self.components_with_material(&m.id));
                    value
                })
                .collect(),
        )
    }

    /// The `body_material` query: the material a body is made of (in the
    /// occurrence `path` when given), its fields and where it comes from.
    pub(crate) fn body_material_json(&self, uid: BodyUid, path: Option<&[OccurrenceUid]>) -> Value {
        let (m, source) = self.body_material(uid, path);
        let mut value = serde_json::to_value(m).expect("materials serialize");
        match source {
            MaterialSource::Body => value["source"] = json!("body"),
            MaterialSource::Component(c) => {
                value["source"] = json!("component");
                value["component"] = json!(c);
            }
            MaterialSource::Default => value["source"] = json!("default"),
        }
        value["appearance_set_by_hand"] = json!(self.appearance_set_by_hand(uid));
        value
    }

    pub(super) fn bodies_with_material(&self, id: &str) -> Vec<BodyUid> {
        self.state
            .body_attributes
            .iter()
            .filter(|(_, a)| a.material.as_deref() == Some(id))
            .map(|(uid, _)| *uid)
            .collect()
    }

    pub(super) fn components_with_material(&self, id: &str) -> Vec<ComponentUid> {
        self.state
            .component_materials
            .iter()
            .filter(|(_, m)| m.as_str() == id)
            .map(|(c, _)| *c)
            .collect()
    }

    /// Makes a material of the document from `based_on`'s fields (a
    /// material of the document or built in, or a library's item with the
    /// appearance it carries, mitcad#655; none: `New Material` of 1 g/cm³)
    /// and the given ones. Without an id it is `custom1`, `custom2`,
    /// ... A built-in's id makes the document's take its place. Returns
    /// the id.
    pub fn create_material(&mut self, change: &MaterialChange) -> Result<String, ModelError> {
        if change.library.is_some() {
            return Err(invalid(
                "a material of a library is made there (create_library_material)",
            ));
        }
        let base = match change.based_on.as_ref() {
            Some(BasedOn::Design(id)) => {
                let mut base = self
                    .material(id)
                    .cloned()
                    .ok_or_else(|| invalid(format!("there is no material '{id}'")))?;
                base.based_on = Some(id.to_owned());
                base
            }
            Some(BasedOn::Library(item)) => {
                // A library's item duplicated into the design: with the
                // appearance it carries, in the same undo step.
                let libraries = self.print_libraries(change.project.as_deref().map(Path::new));
                let found = super::library_items::library_item(
                    &libraries,
                    super::library_kind(&item.library)?,
                    ItemKind::Material,
                    &item.id,
                )?;
                let ItemData::Material {
                    material,
                    appearance,
                } = found.data.clone()
                else {
                    unreachable!("a material's folder")
                };
                let mut base = material;
                base.based_on = Some(item.id.clone());
                return self.as_one_step(|doc| {
                    if let Some(look) = appearance {
                        let label = format!("Add Appearance {}", look.name);
                        let id = doc.apply_with(|state| {
                            Ok((label, state.add_item_appearance(&look), false))
                        })?;
                        base.appearance = Some(id);
                    }
                    doc.create_material_from(Some(base), change)
                });
            }
            None => {
                return self.create_material_from(None, change);
            }
        };
        self.create_material_from(Some(base), change)
    }

    /// [`Document::create_material`] from a material whose fields it
    /// starts from (its `based_on` set), or none.
    fn create_material_from(
        &mut self,
        base: Option<Material>,
        change: &MaterialChange,
    ) -> Result<String, ModelError> {
        // The libraries' names count too (mitcad#655): the lists never show
        // two materials alike.
        let libraries = self.print_libraries(change.project.as_deref().map(Path::new));
        let library_names: Vec<String> = libraries
            .items
            .iter()
            .filter(|i| i.data.kind() == ItemKind::Material && !i.shadowed)
            .map(|i| i.data.name().to_lowercase())
            .collect();
        let taken = |name: &str| {
            self.state.material_name_taken(name, "", None)
                || library_names.contains(&name.to_lowercase())
        };
        let free = (1..)
            .find(|n| {
                self.material(&format!("custom{n}")).is_none()
                    && !taken(&new_item_name("Material", *n))
            })
            .expect("a free number");
        // A copy keeps its source's class (mitcad#655).
        let source_class = base.as_ref().map(material::material_class);
        let mut new = match base {
            Some(mut base) => {
                base.origin = None;
                base.version = None;
                base
            }
            None => Material {
                id: String::new(),
                name: String::new(),
                density: 1.0,
                appearance: None,
                filament: BTreeMap::new(),
                slicer: BTreeMap::new(),
                based_on: None,
                class: None,
                version: None,
                origin: None,
            },
        };
        new.id = match change.id.as_deref().map(str::trim) {
            Some(id) => id.to_owned(),
            None => format!("custom{free}"),
        };
        // A copy is named after its source, `<name> 2` (mitcad#655: the
        // user's own copy of a built-in or library material), a new one
        // `New Material`.
        new.name = match source_class {
            Some(_) => (2..)
                .map(|n| format!("{} {n}", new.name))
                .find(|name| !taken(name))
                .expect("a free name"),
            None => new_item_name("Material", free),
        };
        change.apply_to(&mut new);
        // The class is stored, so that print settings set later never move
        // the material (mitcad#655): the one chosen, else the source's of a
        // copy, else the one its fields give (a filament: plastic).
        new.class = Some(
            change
                .class
                .or(source_class)
                .unwrap_or_else(|| material::material_class(&new)),
        );
        if self.state.materials.iter().any(|m| m.id == new.id) {
            return Err(invalid(format!("a material '{}' exists", new.id)));
        }
        self.check_material(&new, None)?;
        let label = format!("Create Material {}", new.name);
        let id = new.id.clone();
        self.apply_with(|state| {
            state.materials.push(new);
            Ok((label, (), false))
        })?;
        Ok(id)
    }

    /// Changes the given fields of a document material; a built-in one
    /// cannot change (`create_material` with `based_on`, or with its id,
    /// makes one the design can change). A change that changes nothing is
    /// no undo step.
    pub fn edit_material(&mut self, id: &str, change: &MaterialChange) -> Result<(), ModelError> {
        if change.based_on.is_some() {
            return Err(invalid("based_on is only for create_material"));
        }
        if change.id.as_deref().is_some_and(|new| new != id) {
            return Err(invalid("a material's id cannot change"));
        }
        let i = self.custom_material_index(id)?;
        let mut edited = self.state.materials[i].clone();
        change.apply_to(&mut edited);
        if edited == self.state.materials[i] {
            return Ok(());
        }
        self.check_material(&edited, Some(i))?;
        let label = format!("Edit Material {}", self.state.materials[i].name);
        self.apply_with(|state| {
            state.materials[i] = edited;
            Ok((label, (), false))
        })
    }

    /// Deletes a document material: the bodies and components that name it
    /// use the built-in of the same id when the deleted one took its place,
    /// else they have none of their own. Returns them.
    pub fn delete_material(&mut self, id: &str) -> Result<DeletedMaterial, ModelError> {
        let i = self.custom_material_index(id)?;
        let bodies = self.bodies_with_material(id);
        let components = self.components_with_material(id);
        let keep = material::built_in_material(id).is_some();
        let label = format!("Delete Material {}", self.state.materials[i].name);
        self.apply_with(|state| {
            state.materials.remove(i);
            if !keep {
                for attributes in state.body_attributes.values_mut() {
                    if attributes.material.as_deref() == Some(id) {
                        attributes.material = None;
                    }
                }
                state.body_attributes.retain(|_, a| !a.is_default());
                state.component_materials.retain(|_, m| m != id);
            }
            Ok((label, (), false))
        })?;
        Ok(DeletedMaterial { bodies, components })
    }

    /// The fields are valid, the appearance exists and the name is not
    /// another material's (`own`: the index of the one being edited).
    pub(super) fn check_material(
        &self,
        m: &Material,
        own: Option<usize>,
    ) -> Result<(), ModelError> {
        m.check().map_err(invalid)?;
        if let Some(a) = &m.appearance
            && self.appearance(a).is_none()
        {
            return Err(invalid(format!(
                "material {}: there is no appearance '{a}'",
                m.id
            )));
        }
        if self.state.material_name_taken(&m.name, &m.id, own) {
            return Err(invalid(format!("a material is named '{}'", m.name)));
        }
        Ok(())
    }

    fn custom_material_index(&self, id: &str) -> Result<usize, ModelError> {
        self.state
            .materials
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| {
                invalid(if material::built_in_material(id).is_some() {
                    format!(
                        "'{id}' is a built-in material, which cannot change: \
                         create_material with based_on copies it"
                    )
                } else {
                    format!("there is no material '{id}'")
                })
            })
    }
}
