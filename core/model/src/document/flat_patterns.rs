// SPDX-License-Identifier: MIT
//! Flat Patterns kept in the document (mitcad#208): one derived item per
//! sheet-metal body, saved in the design file. Setting or removing one is an
//! undo step that recomputes nothing; the pattern itself is evaluated from
//! the body's current sheet state at the timeline marker whenever it is
//! asked for, so edits made after it was set are always included. It is not
//! a body: occurrences, body lists and physical properties never see it.

use std::sync::Arc;

use serde_json::{Value, json};

use super::{Added, Document, ModelError, PreviewReport, invalid};
use crate::features::{FeatureDef, ValueInput};
use crate::ids::{BodyUid, ComponentUid, FeatureUid};
use crate::kernel::Kernel;
use crate::sheet::flat::{self, FlatFailure, FlatPattern, FlatPatternDef};
use crate::sheet::{PanelId, SheetDomainRef, SheetReferenceSide};

/// The last evaluation of each body's Flat Pattern, by its fingerprint. It
/// is separate from modelling results: evaluating a pattern changes no state.
pub(crate) type FlatCache<S> =
    std::sync::Mutex<std::collections::BTreeMap<BodyUid, (u128, FlatOutcome<S>)>>;
pub(crate) type FlatOutcome<S> = Arc<Result<FlatPattern<S>, FlatFailure>>;

impl<K: Kernel> Document<K> {
    /// The Flat Pattern settings, by body.
    pub fn flat_pattern_defs(&self) -> &std::collections::BTreeMap<BodyUid, FlatPatternDef> {
        &self.state.flat_patterns
    }

    /// Creates or changes the Flat Pattern of a sheet-metal body. Without a
    /// fixed panel it keeps the current one, or takes the body's first panel.
    pub fn set_flat_pattern(
        &mut self,
        body: BodyUid,
        stationary: Option<PanelId>,
        x_axis: Option<flat::FlatXAxis>,
        reference_side: Option<SheetReferenceSide>,
        shown: Option<bool>,
    ) -> Result<(), ModelError> {
        let old = self.state.flat_patterns.get(&body).cloned();
        let Some(state) = self.last_result().sheets.get(&body) else {
            return Err(invalid(format!(
                "body {body} is not a sheet-metal body at the timeline marker"
            )));
        };
        let panels: Vec<PanelId> = state
            .value
            .charts
            .iter()
            .filter_map(|p| match p.domain {
                SheetDomainRef::Panel(id) => Some(id),
                SheetDomainRef::Bend(_) => None,
            })
            .collect();
        let stationary = match stationary.or(old.as_ref().map(|d| d.stationary)) {
            Some(panel) => panel,
            None => *panels
                .iter()
                .min()
                .ok_or_else(|| invalid(format!("sheet {body} has no panel")))?,
        };
        if !panels.contains(&stationary) {
            return Err(invalid(format!(
                "panel {} slot {} is not a panel of sheet {body}",
                stationary.feature, stationary.slot
            )));
        }
        let def = FlatPatternDef {
            stationary,
            x_axis: x_axis
                .or(old.as_ref().map(|d| d.x_axis))
                .unwrap_or_default(),
            reference_side: reference_side
                .or(old.as_ref().map(|d| d.reference_side))
                .unwrap_or(SheetReferenceSide::Front),
            shown: shown.or(old.as_ref().map(|d| d.shown)).unwrap_or(false),
        };
        if old.as_ref() == Some(&def) {
            return Ok(());
        }
        let label = match &old {
            // Only the display choice changed (mitcad#210).
            Some(old)
                if FlatPatternDef {
                    shown: def.shown,
                    ..old.clone()
                } == def =>
            {
                if def.shown {
                    "Show Flat Pattern"
                } else {
                    "Show Folded Body"
                }
            }
            Some(_) => "Edit Flat Pattern",
            None => "Create Flat Pattern",
        };
        self.apply_with(|state| {
            state.flat_patterns.insert(body, def);
            Ok((label.to_owned(), (), false))
        })
    }

    pub fn remove_flat_pattern(&mut self, body: BodyUid) -> Result<(), ModelError> {
        if !self.state.flat_patterns.contains_key(&body) {
            return Err(invalid(format!("body {body} has no Flat Pattern")));
        }
        self.apply_with(|state| {
            state.flat_patterns.remove(&body);
            Ok(("Delete Flat Pattern".to_owned(), (), false))
        })
    }

    /// The evaluated Flat Pattern of a body at the marker, or why there is
    /// none. Repeated requests reuse the evaluation while nothing it
    /// depends on changed.
    pub fn flat_pattern(&self, body: BodyUid) -> FlatOutcome<K::Shape> {
        let Some(def) = self.state.flat_patterns.get(&body) else {
            return Arc::new(Err(FlatFailure {
                status: "unavailable",
                diagnostics: vec![format!("body {body} has no Flat Pattern")],
            }));
        };
        let result = self.last_result();
        let key = flat::fingerprint(result, body, def);
        if let Ok(cache) = self.flat_cache.lock()
            && let Some((cached, outcome)) = cache.get(&body)
            && *cached == key
        {
            return outcome.clone();
        }
        let outcome = Arc::new(flat::evaluate(&self.kernel, result, body, def));
        if let Ok(mut cache) = self.flat_cache.lock() {
            cache.insert(body, (key, outcome.clone()));
        }
        outcome
    }

    /// The developed solid of a body's valid Flat Pattern, for display.
    pub fn flat_pattern_shape(&self, body: BodyUid) -> Option<K::Shape> {
        match &*self.flat_pattern(body) {
            Ok(pattern) => Some(pattern.solid.clone()),
            Err(_) => None,
        }
    }

    /// The `flat_pattern` query: one body's pattern, or all of them.
    pub(crate) fn flat_pattern_json(&self, body: Option<BodyUid>) -> Value {
        let one = |body: BodyUid| {
            let mut value = match &*self.flat_pattern(body) {
                Ok(pattern) => pattern.json(),
                Err(failure) => flat::failure_json(body, failure),
            };
            if let Some(def) = self.state.flat_patterns.get(&body) {
                value["settings"] = json!(def);
            }
            value
        };
        match body {
            Some(body) => one(body),
            None => Value::Array(self.state.flat_patterns.keys().map(|b| one(*b)).collect()),
        }
    }

    /// The settings a body's Flat Pattern has, or those a new one would
    /// get (its lowest panel, the panel's x axis, the front side).
    fn flat_settings(&self, body: BodyUid) -> Result<FlatPatternDef, ModelError> {
        if let Some(def) = self.state.flat_patterns.get(&body) {
            return Ok(def.clone());
        }
        let state = self.last_result().sheets.get(&body).ok_or_else(|| {
            invalid(format!(
                "body {body} is not a sheet-metal body at the timeline marker"
            ))
        })?;
        let stationary = state
            .value
            .charts
            .iter()
            .filter_map(|p| match p.domain {
                SheetDomainRef::Panel(id) => Some(id),
                SheetDomainRef::Bend(_) => None,
            })
            .min()
            .ok_or_else(|| invalid(format!("sheet {body} has no panel")))?;
        Ok(FlatPatternDef {
            stationary,
            x_axis: flat::FlatXAxis::PanelX,
            reference_side: SheetReferenceSide::Front,
            shown: false,
        })
    }

    /// Writes the Flat Pattern of a sheet-metal body to a DXF file
    /// (mitcad#210): the cut contours and bend lines of its evaluated outline
    /// with the item's settings (or a new item's, when the body has none),
    /// viewed from `reference_side` when given. Only a valid pattern is
    /// written; otherwise the reasons are the error. Returns the drawing.
    pub fn export_flat_pattern_dxf(
        &self,
        body: BodyUid,
        path: &str,
        reference_side: Option<SheetReferenceSide>,
        options: &crate::sheet::flat_dxf::FlatDxfOptions,
        write: &mitcad_dxf::WriteOptions,
    ) -> Result<(FlatPatternDef, mitcad_dxf::Drawing), ModelError> {
        let mut def = self.flat_settings(body)?;
        // The item's own evaluation is cached; other settings are not.
        let own = self.state.flat_patterns.contains_key(&body)
            && reference_side.is_none_or(|side| side == def.reference_side);
        if let Some(side) = reference_side {
            def.reference_side = side;
        }
        let outcome = if own {
            self.flat_pattern(body)
        } else {
            Arc::new(flat::evaluate(&self.kernel, self.last_result(), body, &def))
        };
        let pattern = match &*outcome {
            Ok(pattern) => pattern,
            Err(failure) => {
                return Err(invalid(format!(
                    "the Flat Pattern of {body} is {}: {}",
                    failure.status,
                    failure.diagnostics.join("; ")
                )));
            }
        };
        let drawing = crate::sheet::flat_dxf::drawing(&pattern.regions, &pattern.bends, options)
            .map_err(|e| invalid(format!("the Flat Pattern of {body}: {e}")))?;
        mitcad_dxf::write_file(&drawing, path, write)
            .map_err(|e| invalid(format!("cannot write {path}: {e}")))?;
        Ok((def, drawing))
    }

    /// The `flat_point` query: a flat position on the body, or the flat
    /// position of a body point.
    pub(crate) fn flat_point_json(
        &self,
        body: BodyUid,
        flat: Option<[f64; 2]>,
        model: Option<[f64; 3]>,
    ) -> Result<Value, String> {
        let outcome = self.flat_pattern(body);
        let pattern = match &*outcome {
            Ok(pattern) => pattern,
            Err(failure) => return Err(failure.diagnostics.join("; ")),
        };
        let state = &self
            .last_result()
            .sheets
            .get(&body)
            .ok_or("the body has no sheet state")?
            .value;
        pattern.locate(state, flat, model)
    }

    /// The sheet state of a body at the marker and its thickness.
    fn sheet_at(&self, body: BodyUid) -> Result<(&crate::sheet::SheetState, f64), String> {
        let result = self.last_result();
        let state = result
            .sheets
            .get(&body)
            .filter(|_| result.bodies.contains_key(&body))
            .ok_or_else(|| {
                format!("body {body} is not a sheet-metal body at the timeline marker")
            })?;
        let thickness = result
            .sheet_rules
            .get(&state.value.rules)
            .map(|r| r.value.thickness)
            .ok_or_else(|| format!("the rules of sheet {body} are not available"))?;
        Ok((&state.value, thickness))
    }

    /// The `sheet_reference` query (mitcad#210): a face or edge of a sheet
    /// body picked in the view as the semantic reference sheet features
    /// take (its panel or bend, the boundary along a wall, the corner of
    /// two walls); `at` chooses among pieces of one curve.
    pub(crate) fn sheet_reference_json(
        &self,
        body: BodyUid,
        face: Option<&str>,
        edge: Option<&str>,
        at: Option<[f64; 3]>,
    ) -> Result<Value, String> {
        let (state, thickness) = self.sheet_at(body)?;
        crate::sheet::pick::reference(state, thickness, face, edge, at)
    }

    /// The `sheet_seam` query (mitcad#210): the boundaries of the seam two
    /// panels of a sheet share, which a Bend folds.
    pub(crate) fn sheet_seam_json(
        &self,
        body: BodyUid,
        stationary: PanelId,
        moving: PanelId,
    ) -> Result<Value, String> {
        let (state, _) = self.sheet_at(body)?;
        crate::sheet::pick::seam(state, stationary, moving)
    }

    /// The `sheet_bodies` query (mitcad#210): each sheet-metal body at the
    /// marker with its rules, its domains and its Flat Pattern's state, for
    /// the browser.
    pub(crate) fn sheet_bodies_json(&self) -> Value {
        let result = self.last_result();
        let rows = result
            .sheets
            .iter()
            .filter(|(body, _)| result.bodies.contains_key(body))
            .map(|(body, state)| {
                let state = &state.value;
                let rules = result.sheet_rules.get(&state.rules).map(|r| &r.value);
                let panels = state
                    .charts
                    .iter()
                    .filter(|p| matches!(p.domain, SheetDomainRef::Panel(_)))
                    .count();
                let flat = self.state.flat_patterns.get(body).map(|def| {
                    let mut value = match &*self.flat_pattern(*body) {
                        Ok(pattern) => json!({"status": "valid", "area": pattern.area,
                                              "bends": pattern.bends.len()}),
                        Err(failure) => json!({"status": failure.status,
                                               "diagnostics": failure.diagnostics}),
                    };
                    value["shown"] = json!(def.shown);
                    value["stationary"] = json!(def.stationary);
                    value["reference_side"] = json!(def.reference_side);
                    value
                });
                let k = rules.map(|r| {
                    let crate::features::sheet::Allowance::KFactor { k } = r.allowance;
                    k
                });
                json!({
                    "body": body,
                    "name": self.body_name(*body),
                    "status": state.status(),
                    "rules": state.rules,
                    "rules_name": self.feature(state.rules).map(|f| f.name.clone()),
                    "thickness": rules.map(|r| r.thickness),
                    "bend_radius": rules.map(|r| r.bend_radius),
                    "k_factor": k,
                    "panels": panels,
                    "bends": state.bends.len(),
                    "open": state.opened.len(),
                    "flat_pattern": flat,
                })
            })
            .collect();
        Value::Array(rows)
    }

    /// Adds a Face with new rules of its own (mitcad#210): the rules feature
    /// just before it, both in one undo step named after the Face. `face`
    /// makes the Face's definition for the rules' uid.
    pub fn add_face_with_rules(
        &mut self,
        rules: &FeatureDef<ValueInput>,
        face: &dyn Fn(FeatureUid) -> Result<FeatureDef<ValueInput>, ModelError>,
        name: Option<&str>,
        component: Option<ComponentUid>,
    ) -> Result<Added, ModelError> {
        let component = component.unwrap_or(self.state.assembly.active);
        self.state.editable(component)?;
        self.apply(|state| {
            let made = state.add_feature_in(rules, None, component)?;
            let def = face(made.uid)?;
            let added = state.add_feature_in(&def, name, component)?;
            Ok((format!("Add {}", added.name), added))
        })
    }

    /// Previews a Face with new rules of its own: both features on a copy,
    /// reported as the Face.
    pub fn preview_face_with_rules(
        &mut self,
        rules: &FeatureDef<ValueInput>,
        face: &dyn Fn(FeatureUid) -> Result<FeatureDef<ValueInput>, ModelError>,
        name: Option<&str>,
    ) -> Result<PreviewReport, ModelError> {
        let mut state = self.state.clone();
        let made = state.add_feature(rules, None)?;
        let def = face(made.uid)?;
        self.check_profiles(&def)?;
        let added = state.add_feature(&def, name)?;
        self.preview_state(state, added.uid)
    }
}
