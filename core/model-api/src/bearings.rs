// SPDX-License-Identifier: MIT
//! Printable bearings (mitcad#577): the `bearing_values` query (the
//! section, the element count, the walls, gaps and checks a Bearing panel
//! shows), the `bearing_sizes` query (the standard sizes, or those that
//! fit a shaft or a hole) and the parts list's purchased rows (the airsoft
//! balls to buy). See `commands.md`, "Printable bearings".

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use mitcad_model::Document;
use mitcad_model::FeatureStatus;
use mitcad_model::expr::Expr;
use mitcad_model::features::bearing_sizes::{self, BearingSize, Fit};
use mitcad_model::features::printable_bearing::{
    self, FIT_CLEARANCE, GrooveFace, Layout, PrintableBearingDef,
};
use mitcad_model::features::{FeatureDef, GeomRef, SeatFrame, ValueInput};
use mitcad_model::ids::{ComponentUid, FeatureUid};
use mitcad_model::kernel::Kernel;
use mitcad_model::parameters::{slot_unit, unit_for};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BearingValuesQuery {
    /// A printable bearing feature of the timeline.
    #[serde(default)]
    uid: Option<FeatureUid>,
    /// Or a definition as `add_feature` takes it (a panel's).
    #[serde(default)]
    def: Option<Box<PrintableBearingDef<ValueInput>>>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FitBy {
    Bore,
    Outside,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BearingSizesQuery {
    /// Only the sizes whose bore (a shaft's) or outside diameter (a
    /// hole's) is within `clearance` of `diameter`, the nearest first.
    #[serde(default)]
    fit: Option<FitBy>,
    #[serde(default)]
    diameter: Option<f64>,
    #[serde(default)]
    clearance: Option<f64>,
    /// Only the sizes of a series (`60`, `MR`, `F`, …).
    #[serde(default)]
    series: Option<String>,
}

fn size_json(size: &BearingSize) -> Value {
    json!({
        "designation": size.designation,
        "series": size.series,
        "bore": size.bore,
        "outside_diameter": size.outside_diameter,
        "width": size.width,
        "flange": size.flange.map(|(d, t)| json!({"diameter": d, "thickness": t})),
    })
}

/// A purchased part the parts list adds: what, how many per placement of
/// `component`.
pub(crate) struct Purchased {
    pub feature: FeatureUid,
    pub component: mitcad_model::ids::ComponentUid,
    pub designation: String,
    pub quantity: usize,
}

/// The document's methods of this module used across the model, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(crate) trait BearingsCrateOps<K: Kernel> {
    /// The parts to buy for the printable bearings at the marker (mitcad#577):
    /// each airsoft bearing's balls, in the component its bodies are in.
    fn purchased_parts(&self) -> Vec<Purchased>;

    /// The parts list's purchased rows: one per designation and component,
    /// the quantity the balls per bearing times the component's
    /// placements (`quantity` of its row; the root's once).
    fn purchased_rows(&self, rows: &[mitcad_model::PartsListRow]) -> Vec<Value>;
}

impl<K: Kernel> BearingsCrateOps<K> for Document<K> {
    fn purchased_parts(&self) -> Vec<Purchased> {
        let mut parts = Vec::new();
        for entry in self.features() {
            if self.status(entry.uid) != Some(&FeatureStatus::Ok) {
                continue;
            }
            let FeatureDef::PrintableBearing(def) = &entry.def else {
                continue;
            };
            let Ok(mut values) = self.bearing_values_of(entry.uid) else {
                continue;
            };
            let Some((designation, quantity)) = self
                .bearing_layout(&mut values, entry.component, &mut None)
                .ok()
                .and_then(|l| l.purchased())
            else {
                continue;
            };
            let component = if def.operation == mitcad_model::features::Operation::NewComponent {
                self.assembly().created_by(entry.uid)
            } else {
                None
            };
            parts.push(Purchased {
                feature: entry.uid,
                component: component.unwrap_or(entry.component),
                designation,
                quantity,
            });
        }
        parts
    }

    fn purchased_rows(&self, rows: &[mitcad_model::PartsListRow]) -> Vec<Value> {
        let mut out: Vec<(
            String,
            mitcad_model::ids::ComponentUid,
            usize,
            Vec<FeatureUid>,
        )> = Vec::new();
        for part in self.purchased_parts() {
            let placed = if part.component.is_root() {
                1
            } else {
                rows.iter()
                    .find(|r| r.component == part.component)
                    .map_or(0, |r| r.quantity)
            };
            let quantity = part.quantity * placed;
            match out
                .iter_mut()
                .find(|(d, c, _, _)| *d == part.designation && *c == part.component)
            {
                Some(row) => {
                    row.2 += quantity;
                    row.3.push(part.feature);
                }
                None => out.push((
                    part.designation,
                    part.component,
                    quantity,
                    vec![part.feature],
                )),
            }
        }
        out.into_iter()
            .map(|(designation, component, quantity, features)| {
                json!({
                    "component": component,
                    "name": designation,
                    "designation": designation,
                    "quantity": quantity,
                    "linked": false,
                    "library": null,
                    "version": null,
                    "config": null,
                    "license": null,
                    "authors": null,
                    "url": null,
                    "supplier": "",
                    "part_number": "",
                    "supplier_url": "",
                    "purchased": true,
                    "features": features,
                })
            })
            .collect()
    }
}

/// The document's methods of this module used across the api, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(super) trait BearingsOps<K: Kernel> {
    fn bearing_values_json(&self, query: &BearingValuesQuery) -> Result<Value, ApiError>;

    fn bearing_sizes_json(&self, query: &BearingSizesQuery) -> Result<Value, ApiError>;
}

impl<K: Kernel> BearingsOps<K> for Document<K> {
    fn bearing_values_json(&self, query: &BearingValuesQuery) -> Result<Value, ApiError> {
        let (mut values, component) = match (&query.uid, &query.def) {
            (Some(uid), None) => (
                self.bearing_values_of(*uid)?,
                self.feature(*uid)
                    .map_or(self.active_component(), |f| f.component),
            ),
            (None, Some(def)) => (self.bearing_values_in(def)?, self.active_component()),
            _ => {
                return Err(ApiError(
                    "bearing_values takes a bearing feature's uid or a definition".to_owned(),
                ));
            }
        };
        let mut seat = None;
        let layout = self.bearing_layout(&mut values, component, &mut seat);
        let mut result = match layout {
            Ok(layout) => printable_bearing::derived_values(&layout),
            Err(error) => json!({"element": values.element, "error": error}),
        };
        // On a shaft or in a hole (mitcad#578): the seat, its diameter and
        // the sizes that fit it, the nearest first.
        if let Some(frame) = seat {
            result["seat"] = frame.to_json();
            result["shaft_diameter"] = json!(frame.diameter);
            let fits: Vec<&str> = values
                .fitting_sizes(&frame)
                .iter()
                .map(|s| s.designation)
                .collect();
            result["fits"] = json!(fits);
        }
        if let Some(name) = values.component_name()
            && values.operation == mitcad_model::features::Operation::NewComponent
        {
            result["component_name"] = json!(name);
        }
        Ok(result)
    }

    fn bearing_sizes_json(&self, query: &BearingSizesQuery) -> Result<Value, ApiError> {
        let sizes: Vec<&BearingSize> = match (query.fit, query.diameter) {
            (Some(fit), Some(diameter)) => bearing_sizes::fitting(
                match fit {
                    FitBy::Bore => Fit::Bore,
                    FitBy::Outside => Fit::Outside,
                },
                diameter,
                query.clearance.unwrap_or(FIT_CLEARANCE),
            ),
            (None, None) => bearing_sizes::SIZES.iter().collect(),
            _ => {
                return Err(ApiError(
                    "bearing_sizes takes a fit (bore or outside) with a diameter".to_owned(),
                ));
            }
        };
        let sizes: Vec<Value> = sizes
            .into_iter()
            .filter(|s| {
                query
                    .series
                    .as_deref()
                    .is_none_or(|series| series.eq_ignore_ascii_case(s.series))
            })
            .map(size_json)
            .collect();
        Ok(json!({"sizes": sizes}))
    }
}

/// The document's methods used only in this module, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
trait BearingsLocal<K: Kernel> {
    /// A definition's values: numbers as they are (mm, radians), names and
    /// expressions evaluated in the slot's unit.
    fn bearing_values_in(
        &self,
        def: &PrintableBearingDef<ValueInput>,
    ) -> Result<PrintableBearingDef<f64>, ApiError>;

    /// A bearing feature's definition with the document's values.
    fn bearing_values_of(&self, uid: FeatureUid) -> Result<PrintableBearingDef<f64>, ApiError>;

    /// The face a groove is cut into, as the body at the marker has it.
    fn groove_face(&self, groove: &GeomRef) -> Result<GrooveFace, String>;

    /// A bearing's section at the marker, in `component`'s coordinates:
    /// with its seat resolved (`seat` set to it; the fitting size taken
    /// into `values` when none is given) or its groove's face.
    fn bearing_layout(
        &self,
        values: &mut PrintableBearingDef<f64>,
        component: ComponentUid,
        seat: &mut Option<SeatFrame>,
    ) -> Result<Layout, String>;
}

impl<K: Kernel> BearingsLocal<K> for Document<K> {
    fn bearing_values_in(
        &self,
        def: &PrintableBearingDef<ValueInput>,
    ) -> Result<PrintableBearingDef<f64>, ApiError> {
        let params = mitcad_model::internal::print_fits::evaluation_parameters(self.parameters())
            .map_err(|e| ApiError(e.to_string()))?;
        let context = params.context();
        def.map_params(&mut |slot, value| match value {
            ValueInput::Number(n) => Ok(*n),
            ValueInput::Name(text) => {
                let unit = unit_for(slot_unit(slot).dims(), &context)
                    .unwrap_or(mitcad_model::expr::Unit::NONE);
                let parsed = Expr::parse(text).map_err(|e| ApiError(format!("{slot}: {e}")))?;
                parsed
                    .eval_as(params.table(), &context, unit)
                    .map(|v| v.value)
                    .map_err(|e| ApiError(format!("{slot}: {e}")))
            }
        })
    }

    fn bearing_values_of(&self, uid: FeatureUid) -> Result<PrintableBearingDef<f64>, ApiError> {
        let entry = self
            .feature(uid)
            .ok_or_else(|| ApiError(format!("feature {uid} does not exist")))?;
        let FeatureDef::PrintableBearing(def) = &entry.def else {
            return Err(ApiError(format!(
                "{} is not a printable bearing",
                entry.name
            )));
        };
        let params = self.parameters();
        Ok(printable_bearing::evaluated(def, &|id| {
            params.value(id).unwrap_or(f64::NAN)
        }))
    }

    fn groove_face(&self, groove: &GeomRef) -> Result<GrooveFace, String> {
        let GeomRef::Face { body, face } = groove else {
            return Err("the raceway is cut into a face of a body".to_owned());
        };
        let shape = self
            .body_shape(*body)
            .ok_or_else(|| format!("body {body} does not exist at the marker"))?;
        let cylinder = self.kernel().face_cylinder(shape, face).map_err(|_| {
            "the raceway is cut into a cylindrical face: a bore or a shaft".to_owned()
        })?;
        Ok(GrooveFace {
            radius: cylinder.radius,
            internal: cylinder.internal,
        })
    }

    fn bearing_layout(
        &self,
        values: &mut PrintableBearingDef<f64>,
        component: ComponentUid,
        seat: &mut Option<SeatFrame>,
    ) -> Result<Layout, String> {
        // Placed by picked geometry (mitcad#656): a shaft or a hole as a
        // seat.
        if let Some(at) = values.at.clone() {
            let placed = self.place_frame(&at).map_err(|e| format!("at: {e}"))?;
            if let Some(frame) = placed.round() {
                *seat = Some(*frame);
                values.fit_to(frame)?;
            }
            return values.layout(None);
        }
        if let Some(on) = values.seat.clone() {
            let frame = self
                .seat_frame(&on, component)
                .map_err(|e| format!("seat: {e}"))?;
            *seat = Some(frame);
            values.fit_to(&frame)?;
            return values.layout(None);
        }
        let groove = match &values.groove {
            None => None,
            Some(groove) => Some(self.groove_face(groove)?),
        };
        values.layout(groove)
    }
}
