// SPDX-License-Identifier: MIT
//! The cut list (`build_cut_list`): one row per group of identical cut
//! parts (the same source, row, length, end cuts, machining and own
//! features), the stock lengths they are cut from in their cutting order,
//! and the parts list's rows for everything else.

use std::collections::BTreeMap;

use super::parts::{PartInfo, designation, part_number, part_values};
use super::{BuildItem, SourceId};
use crate::document::Document;
use crate::features::FeatureDef;
use crate::features::derived_part::Tilt;
use crate::ids::{ComponentUid, FeatureUid};
use crate::kernel::Kernel;
use crate::parameters::ParamId;

/// How the cut list is worked out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutListOptions {
    /// The saw's cut, mm.
    pub kerf: f64,
}

impl Default for CutListOptions {
    fn default() -> Self {
        Self { kerf: 3.0 }
    }
}

/// A hole in a cut part: where, its diameter, depth and thread.
#[derive(Debug, Clone, PartialEq)]
pub struct MachiningItem {
    /// The frame it is on (`end_b`, `slot_x+_1`, ...).
    pub at: String,
    pub diameter: f64,
    pub depth: Option<f64>,
    pub thread: Option<String>,
    /// Added by a connector (else the part's own).
    pub connector: Option<ComponentUid>,
}

/// A group of identical cut parts.
#[derive(Debug, Clone, PartialEq)]
pub struct CutRow {
    pub source: SourceId,
    pub designation: String,
    pub supplier: String,
    pub part_number: String,
    pub config: Option<String>,
    pub length: f64,
    /// End A's and B's cut: angle (radians) and tilt.
    pub ends: [Option<(f64, Tilt)>; 2],
    pub machining: Vec<MachiningItem>,
    /// Its own features (holes, cuts) besides machining.
    pub features: usize,
    pub quantity: usize,
    /// Each, kg.
    pub mass: Option<f64>,
    pub components: Vec<ComponentUid>,
}

/// The stock pieces one source is cut from.
#[derive(Debug, Clone, PartialEq)]
pub struct StockEntry {
    pub source: SourceId,
    pub designation: String,
    pub stock_length: f64,
    /// The pieces' lengths in cutting order, stock piece after stock piece.
    pub pieces: Vec<f64>,
    /// Where each stock piece's cuts end (indices into `pieces`).
    pub bars: Vec<usize>,
    pub needed: usize,
    pub offcut: f64,
    /// Pieces longer than the stock.
    pub too_long: Vec<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CutList {
    pub rows: Vec<CutRow>,
    pub stock: Vec<StockEntry>,
    /// The components the rows are of (the parts list lists the others).
    pub cut: Vec<ComponentUid>,
}

impl<K: Kernel> Document<K> {
    /// The holes in a part: those connectors added and its own machining.
    pub(crate) fn part_machining(&self, info: &PartInfo) -> Vec<(FeatureUid, MachiningItem)> {
        let mut added: BTreeMap<FeatureUid, Option<ComponentUid>> = BTreeMap::new();
        for c in &self.assembly().components {
            let Some(record) = &c.build else { continue };
            for item in &record.adds {
                if let BuildItem::Feature(f) = item {
                    added.insert(*f, (c.uid != info.component).then_some(c.uid));
                }
            }
        }
        let params = self.parameters();
        let value = |id: &ParamId| params.value(*id).unwrap_or(f64::NAN);
        let mut out = Vec::new();
        for entry in self.features() {
            if entry.component != info.component {
                continue;
            }
            let Some(connector) = added.get(&entry.uid) else {
                continue;
            };
            let FeatureDef::Hole(hole) = &entry.def else {
                continue;
            };
            let at = match &hole.placement {
                crate::features::HolePlacement::Plane {
                    plane: crate::features::GeomRef::Mount { name, .. },
                    ..
                } => name.clone(),
                _ => String::new(),
            };
            let depth = match &hole.extent {
                crate::features::HoleExtent::Distance { depth } => Some(value(depth)),
                _ => None,
            };
            out.push((
                entry.uid,
                MachiningItem {
                    at,
                    diameter: value(&hole.diameter),
                    depth,
                    thread: hole.thread.as_ref().map(|t| t.designation.clone()),
                    connector: *connector,
                },
            ));
        }
        out
    }

    /// The cut list.
    pub fn build_cut_list(&self, options: CutListOptions) -> CutList {
        let params = self.parameters();
        let quantities: BTreeMap<ComponentUid, usize> = self
            .parts_list()
            .into_iter()
            .map(|r| (r.component, r.quantity))
            .collect();
        let mut groups: BTreeMap<String, CutRow> = BTreeMap::new();
        let mut cut = Vec::new();
        for info in self.build_parts_info() {
            let (Some(source), Some(def), Some((_, length))) =
                (&info.source, &info.def, info.length)
            else {
                continue;
            };
            cut.push(info.component);
            let ends = [&def.ends.a, &def.ends.b].map(|e| {
                e.as_ref().and_then(|cut| {
                    let angle = params.value(cut.angle)?;
                    (angle.abs() > 1e-12).then(|| (angle, cut.tilt.unwrap_or_default()))
                })
            });
            let machining: Vec<MachiningItem> = self
                .part_machining(&info)
                .into_iter()
                .map(|(_, m)| m)
                .collect();
            let holes: Vec<FeatureUid> = self
                .part_machining(&info)
                .into_iter()
                .map(|(f, _)| f)
                .collect();
            // Its own features, with their values, for the grouping.
            let own: Vec<String> = self
                .features()
                .filter(|f| f.component == info.component)
                .filter(|f| Some(f.uid) != info.part && !holes.contains(&f.uid))
                .map(|f| {
                    let valued = f
                        .def
                        .map_params(&mut |_, id: &ParamId| {
                            Ok::<_, ()>(params.value(*id).unwrap_or(f64::NAN))
                        })
                        .expect("values never fail");
                    serde_json::to_string(&valued).unwrap_or_default()
                })
                .collect();
            let values = part_values(def, params);
            let config = def.config.as_deref();
            let key = format!(
                "{}|{:?}|{:.6}|{:?}|{:?}|{:?}",
                source.id,
                config,
                length,
                ends.map(|e| e.map(|(a, t)| (format!("{a:.9}"), t.as_str()))),
                machining
                    .iter()
                    .map(|m| (m.at.clone(), format!("{:.6}", m.diameter), m.thread.clone()))
                    .collect::<Vec<_>>(),
                own
            );
            let quantity = quantities.get(&info.component).copied().unwrap_or(1).max(1);
            let row = groups.entry(key).or_insert_with(|| CutRow {
                source: source.id,
                designation: designation(source, config, &values),
                supplier: source.supplier.clone(),
                part_number: part_number(source, config),
                config: def.config.clone(),
                length,
                ends,
                machining,
                features: own.len(),
                quantity: 0,
                mass: info
                    .meta
                    .kilograms_per_metre()
                    .map(|kg| kg * length / 1000.0),
                components: Vec::new(),
            });
            row.quantity += quantity;
            row.components.push(info.component);
        }
        let rows: Vec<CutRow> = groups.into_values().collect();
        // The stock: first fit of the longest pieces into the longest stock.
        let mut by_source: BTreeMap<SourceId, Vec<f64>> = BTreeMap::new();
        for row in &rows {
            for _ in 0..row.quantity {
                by_source.entry(row.source).or_default().push(row.length);
            }
        }
        let mut stock = Vec::new();
        for (id, mut pieces) in by_source {
            let Some(source) = self.state().library_sources.get(id) else {
                continue;
            };
            let meta = source
                .build
                .as_ref()
                .and_then(|b| super::BuildMeta::read(b).ok())
                .unwrap_or_default();
            let Some(length) = meta.length.as_ref() else {
                continue;
            };
            let Some(&stock_length) = length.stock_lengths().first() else {
                continue;
            };
            pieces.sort_by(|a, b| b.total_cmp(a));
            let mut bars: Vec<(f64, Vec<f64>)> = Vec::new();
            let mut too_long = Vec::new();
            for piece in pieces {
                if piece > stock_length + 1e-9 {
                    too_long.push(piece);
                    continue;
                }
                let need = piece + options.kerf;
                match bars.iter_mut().find(|(left, _)| {
                    *left + 1e-9 >= need || (*left + 1e-9 >= piece && !left.is_nan())
                }) {
                    Some((left, cuts)) => {
                        *left = (*left - need).max(0.0);
                        cuts.push(piece);
                    }
                    None => bars.push(((stock_length - need).max(0.0), vec![piece])),
                }
            }
            let mut flat = Vec::new();
            let mut ends = Vec::new();
            for (_, cuts) in &bars {
                flat.extend(cuts.iter().copied());
                ends.push(flat.len());
            }
            stock.push(StockEntry {
                source: id,
                designation: source.label(),
                stock_length,
                pieces: flat,
                bars: ends,
                needed: bars.len(),
                offcut: bars.iter().map(|(left, _)| *left).sum(),
                too_long,
            });
        }
        CutList { rows, stock, cut }
    }
}
