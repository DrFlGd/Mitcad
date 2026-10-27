// SPDX-License-Identifier: MIT
//! The file's named values as Mitcad parameters: user parameters, the
//! model parameters of sketch dimensions (`d1`) and the parameters of
//! feature inputs (`param_names`). Each keeps its name where Mitcad allows
//! it (a name Mitcad reserves gets `_` appended) and its expression,
//! translated.
//!
//! A dimension's or input's parameter is made by its slot (`d3 = height`):
//! it belongs to the Mitcad feature, as in the file. A value is made when
//! it is first needed: a slot's expression names it, or the file's
//! parameters are all made at the end. One needed before the feature that
//! owns it (a user parameter that names a later dimension) becomes a user
//! parameter, which that slot then uses by name.

use std::collections::{HashMap, HashSet};

use mitcad_model::expr::Unit;
use mitcad_model::{Document, Kernel};
use mitcad_solidcraft::Design;
use mitcad_solidcraft::expr::{references, translate};
use mitcad_solidcraft::inputs::{self, Kind};

use super::report::{ParameterReport, SolidcraftReport};

/// A value of the file by name.
#[derive(Debug, Clone)]
struct Def {
    expr: String,
    unit: String,
    comment: String,
    kind: &'static str,
}

pub(crate) struct Params {
    defs: HashMap<String, Def>,
    /// The file's order of its parameters (user ones first).
    order: Vec<String>,
    /// The file's names made, and the Mitcad parameter each is.
    made: HashMap<String, String>,
    /// Being made (a cycle is an error).
    making: HashSet<String>,
    /// Slots of the feature being added that name parameters: made when
    /// the feature is.
    pending: Vec<(String, String, String)>,
    pub reports: Vec<ParameterReport>,
    /// The next number of a name the import gives a slot the file does not
    /// name (`d<n>` past every `d<n>` of the file), so that no name Mitcad
    /// would give it collides with one of the file's.
    next: usize,
}

impl Params {
    pub fn new(design: &Design) -> Self {
        let mut defs = HashMap::new();
        let mut order = Vec::new();
        for p in &design.params {
            order.push(p.name.clone());
            defs.insert(
                p.name.clone(),
                Def {
                    expr: p.expr.clone(),
                    unit: p.unit.clone(),
                    comment: p.comment.clone(),
                    kind: if p.model { "dimension" } else { "user" },
                },
            );
        }
        for f in &design.features {
            let ins = inputs::inputs(&f.def);
            for (name, input) in f.param_names.iter().zip(ins) {
                if name.is_empty() || defs.contains_key(name) {
                    continue;
                }
                order.push(name.clone());
                defs.insert(
                    name.clone(),
                    Def {
                        expr: input.expr,
                        unit: match input.kind {
                            Kind::Length => "mm",
                            Kind::Angle => "deg",
                            Kind::Unitless => "",
                        }
                        .to_owned(),
                        comment: design.param_comments.get(name).cloned().unwrap_or_else(|| {
                            format!("{} {}", f.name, input.label.to_lowercase())
                        }),
                        kind: "input",
                    },
                );
            }
        }
        let next = defs
            .keys()
            .filter_map(|n| n.strip_prefix('d')?.parse::<usize>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        Self {
            defs,
            order,
            made: HashMap::new(),
            making: HashSet::new(),
            pending: Vec::new(),
            reports: Vec::new(),
            next,
        }
    }

    /// The Mitcad name of a file's name.
    pub fn mitcad_name(&self, name: &str) -> String {
        if let Some(m) = self.made.get(name) {
            return m.clone();
        }
        valid_name(name)
    }

    /// `expr` in Mitcad's language, every value it names made first.
    pub fn expression<K: Kernel>(
        &mut self,
        doc: &mut Document<K>,
        expr: &str,
    ) -> Result<String, String> {
        for r in references(expr) {
            self.ensure(doc, &r)?;
        }
        let made = &self.made;
        translate(expr, &|n| {
            made.get(n).cloned().unwrap_or_else(|| valid_name(n))
        })
    }

    /// [`Params::expression`] with names of its own (`Thickness` in a
    /// plastic rule, mitcad#459) standing for the Mitcad names given,
    /// unless the file has a value of that name.
    pub fn expression_with<K: Kernel>(
        &mut self,
        doc: &mut Document<K>,
        expr: &str,
        local: &[(&str, String)],
    ) -> Result<String, String> {
        let local: Vec<(String, String)> = local
            .iter()
            .filter(|(n, _)| !self.defs.contains_key(*n))
            .map(|(n, m)| ((*n).to_owned(), m.clone()))
            .collect();
        for r in references(expr) {
            if !local.iter().any(|(n, _)| *n == r) {
                self.ensure(doc, &r)?;
            }
        }
        let made = &self.made;
        translate(expr, &|n| {
            local
                .iter()
                .find(|(l, _)| l == n)
                .map(|(_, m)| m.clone())
                .or_else(|| made.get(n).cloned())
                .unwrap_or_else(|| valid_name(n))
        })
    }

    /// The value of a slot: `name = expression` for a named value the
    /// feature makes, the name for one made before, else the expression.
    pub fn slot<K: Kernel>(
        &mut self,
        doc: &mut Document<K>,
        name: Option<&str>,
        expr: &str,
    ) -> Result<String, String> {
        let e = self.expression(doc, expr)?;
        let Some(name) = name.filter(|n| !n.is_empty()) else {
            return Ok(self.unnamed(doc, &e));
        };
        if let Some(m) = self.made.get(name) {
            return Ok(m.clone());
        }
        let m = valid_name(name);
        self.claim(doc, &m)?;
        self.pending
            .push((name.to_owned(), m.clone(), expr.to_owned()));
        Ok(format!("{m} = {e}"))
    }

    /// A slot of a Mitcad expression the file does not name: a name of
    /// the import's own (`d<n>` past the file's), so that no name Mitcad
    /// gives it collides with one of the file's in the same command.
    pub fn unnamed<K: Kernel>(&mut self, doc: &Document<K>, e: &str) -> String {
        let name = loop {
            let n = format!("d{}", self.next);
            self.next += 1;
            if doc.parameters().find(&n).is_none() && !self.pending.iter().any(|p| p.1 == n) {
                break n;
            }
        };
        self.pending
            .push((String::new(), name.clone(), e.to_owned()));
        format!("{name} = {e}")
    }

    /// A Mitcad name for a value of the file: a parameter that has it and
    /// is not the file's (one Mitcad named itself) is renamed out of the
    /// way.
    fn claim<K: Kernel>(&self, doc: &mut Document<K>, m: &str) -> Result<(), String> {
        if doc.parameters().find(m).is_none() || self.made.values().any(|v| v == m) {
            return Ok(());
        }
        let mut k = 2;
        let free = loop {
            let n = format!("{m}_{k}");
            if doc.parameters().find(&n).is_none() {
                break n;
            }
            k += 1;
        };
        doc.rename_parameter(m, &free).map_err(|e| e.to_string())
    }

    /// A slot whose Mitcad value is `f` of the file's value: the file's
    /// named value is made as a user parameter (so its value stays the
    /// file's), and the slot is `f(name)`.
    pub fn slot_of<K: Kernel>(
        &mut self,
        doc: &mut Document<K>,
        name: Option<&str>,
        expr: &str,
        f: impl Fn(&str) -> String,
    ) -> Result<String, String> {
        match name.filter(|n| !n.is_empty()) {
            Some(n) => {
                self.ensure(doc, n)?;
                let m = self.mitcad_name(n);
                let g = f(&m);
                Ok(self.unnamed(doc, &g))
            }
            None => {
                let e = self.expression(doc, expr)?;
                let g = f(&format!("({e})"));
                Ok(self.unnamed(doc, &g))
            }
        }
    }

    /// The feature was added: the names its slots made are made.
    pub fn adopt<K: Kernel>(&mut self, doc: &Document<K>, _added: &mitcad_model::Added) {
        for (name, m, expr) in self.pending.drain(..) {
            if name.is_empty() {
                continue;
            }
            if doc.parameters().find(&m).is_some() {
                self.reports.push(ParameterReport {
                    name: name.clone(),
                    expression: expr,
                    mitcad: m.clone(),
                    renamed: (m != name).then(|| m.clone()),
                    kind: self.defs.get(&name).map_or("input", |d| d.kind),
                    error: None,
                });
                self.made.insert(name, m);
            }
        }
    }

    /// The slots of a feature that did not come in.
    pub fn forget_pending(&mut self) {
        self.pending.clear();
    }

    /// A value made, as a user parameter when nothing made it yet.
    pub fn ensure<K: Kernel>(&mut self, doc: &mut Document<K>, name: &str) -> Result<(), String> {
        if self.made.contains_key(name) {
            return Ok(());
        }
        let Some(def) = self.defs.get(name).cloned() else {
            // Not a value of the file: a Mitcad parameter of that name (the
            // translation keeps it), else an error when it is evaluated.
            return Ok(());
        };
        if !self.making.insert(name.to_owned()) {
            return Err(format!("parameter {name} refers to itself"));
        }
        let result = self.make_user(doc, name, &def);
        self.making.remove(name);
        result
    }

    fn make_user<K: Kernel>(
        &mut self,
        doc: &mut Document<K>,
        name: &str,
        def: &Def,
    ) -> Result<(), String> {
        let e = match self.expression(doc, &def.expr) {
            Ok(e) => e,
            Err(error) => {
                self.report_error(name, def, &error);
                return Err(error);
            }
        };
        let m = valid_name(name);
        if let Err(error) = self.claim(doc, &m) {
            self.report_error(name, def, &error);
            return Err(error);
        }
        let unit = unit(&def.unit);
        let comment = if def.kind == "user" {
            def.comment.clone()
        } else if def.comment.is_empty() {
            "Imported dimension".to_owned()
        } else {
            def.comment.clone()
        };
        let made = doc
            .add_parameter_expression(&m, &e, Some(unit), &comment)
            .or_else(|_| doc.add_parameter_expression(&m, &e, None, &comment));
        match made {
            Ok(()) => {
                self.reports.push(ParameterReport {
                    name: name.to_owned(),
                    expression: def.expr.clone(),
                    mitcad: e,
                    renamed: (m != name).then(|| m.clone()),
                    kind: def.kind,
                    error: None,
                });
                self.made.insert(name.to_owned(), m);
                Ok(())
            }
            Err(error) => {
                let error = error.to_string();
                self.report_error(name, def, &error);
                Err(error)
            }
        }
    }

    fn report_error(&mut self, name: &str, def: &Def, error: &str) {
        if self.reports.iter().any(|r| r.name == name) {
            return;
        }
        self.reports.push(ParameterReport {
            name: name.to_owned(),
            expression: def.expr.clone(),
            mitcad: String::new(),
            renamed: None,
            kind: def.kind,
            error: Some(error.to_owned()),
        });
    }

    /// The file's parameters nothing made: user parameters.
    pub fn make_rest<K: Kernel>(&mut self, doc: &mut Document<K>, report: &mut SolidcraftReport) {
        for name in self.order.clone() {
            let _ = self.ensure(doc, &name);
        }
        let mut reports = std::mem::take(&mut self.reports);
        reports.sort_by_key(|r| {
            self.order
                .iter()
                .position(|n| *n == r.name)
                .unwrap_or(usize::MAX)
        });
        report.parameters = reports;
    }

    /// Changes a value of the file after the import.
    pub fn set<K: Kernel>(
        &mut self,
        doc: &mut Document<K>,
        name: &str,
        expr: &str,
    ) -> Result<(), String> {
        let m = self
            .made
            .get(name)
            .cloned()
            .ok_or_else(|| format!("no parameter {name} came in"))?;
        let e = self.expression(doc, expr)?;
        doc.set_parameter_expression(&m, &e, None)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// The value of a file's expression in mm or radians (`kind`), its
    /// names made first.
    pub fn value<K: Kernel>(
        &mut self,
        doc: &mut Document<K>,
        expr: &str,
        kind: Kind,
    ) -> Result<f64, String> {
        let e = self.expression(doc, expr)?;
        let unit = match kind {
            Kind::Length => Unit::MM,
            Kind::Angle => unit("deg"),
            Kind::Unitless => Unit::NONE,
        };
        doc.parameters()
            .table()
            .evaluate(&e, unit)
            .map(|q| q.value)
            .map_err(|e| e.to_string())
    }
}

/// A name Mitcad takes as a parameter's: the file's, or with `_` appended
/// where Mitcad reserves it (units, functions, constants).
pub(crate) fn valid_name(name: &str) -> String {
    if mitcad_model::expr::is_valid_name(name) {
        name.to_owned()
    } else {
        let n = format!("{name}_");
        if mitcad_model::expr::is_valid_name(&n) {
            n
        } else {
            name.chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
                + "_"
        }
    }
}

/// A file's parameter unit as Mitcad's (`mm`, `deg`, empty: unitless).
fn unit(u: &str) -> Unit {
    match u.trim() {
        "" | "none" | "unitless" => Unit::NONE,
        "°" | "degree" | "degrees" => Unit::parse("deg").unwrap_or(Unit::NONE),
        // (`Text`, a text parameter's unit since mitcad#664, is no unit of
        // these files' numbers.)
        other => Unit::parse(other)
            .ok()
            .filter(|u| !u.is_text())
            .unwrap_or(Unit::MM),
    }
}
