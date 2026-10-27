// SPDX-License-Identifier: MIT
//! Limits of a parameter (mitcad#665): an optional minimum, maximum and
//! step. `min` and `max` are expressions in the parameter's unit and may
//! reference other parameters (`Wall * 2`); `step` is a positive value
//! (`1` for whole numbers, `0.2 mm`). By default the value is the
//! expression's result clamped to [min, max] and snapped to the step, and
//! the parameter records which limit took effect ([`Limited`]); a *strict*
//! parameter refuses a result outside its limits instead.
//!
//! The step rule: steps are counted from the minimum (from zero when there
//! is none), and a result goes to the nearest step; one halfway between
//! two steps goes to the upper one. A step above the maximum is replaced by
//! the step below it (never below the minimum), so a snapped value always
//! stays within [min, max].

use std::fmt;

use super::ast::Expr;
use super::eval::EvalError;
use super::format::{DEFAULT_DECIMALS, format_value};
use super::parser::ParseError;
use super::units::Unit;

/// Which limit of a parameter took effect, or is meant in a message. The
/// names (`min`, `max`, `step`) are the `limited` field of the `parameters`
/// query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Limited {
    Min,
    Max,
    Step,
}

impl Limited {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::Max => "max",
            Self::Step => "step",
        }
    }

    /// The word for messages: minimum, maximum, step.
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Min => "minimum",
            Self::Max => "maximum",
            Self::Step => "step",
        }
    }
}

impl fmt::Display for Limited {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The limits of a parameter as written. The field names are the names in
/// the design file, the `parameters` query and the `add_parameter` and
/// `set_parameter` commands: `min`, `max` and `step` (expression texts,
/// None when not set) and `strict` (false: clamp and snap; true: refuse).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParamLimits {
    pub min: Option<String>,
    pub max: Option<String>,
    pub step: Option<String>,
    pub strict: bool,
}

impl ParamLimits {
    /// No limits and not strict: what a parameter has by default.
    pub fn is_none(&self) -> bool {
        !self.has_bounds() && !self.strict
    }

    /// Whether any of `min`, `max` and `step` is set.
    pub fn has_bounds(&self) -> bool {
        self.min.is_some() || self.max.is_some() || self.step.is_some()
    }

    /// The text of one limit.
    pub fn text(&self, which: Limited) -> Option<&str> {
        match which {
            Limited::Min => self.min.as_deref(),
            Limited::Max => self.max.as_deref(),
            Limited::Step => self.step.as_deref(),
        }
    }

    pub(crate) fn text_mut(&mut self, which: Limited) -> &mut Option<String> {
        match which {
            Limited::Min => &mut self.min,
            Limited::Max => &mut self.max,
            Limited::Step => &mut self.step,
        }
    }
}

/// The evaluated limits in canonical units (millimetres, radians).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Bounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

impl Bounds {
    pub fn get(&self, which: Limited) -> Option<f64> {
        match which {
            Limited::Min => self.min,
            Limited::Max => self.max,
            Limited::Step => self.step,
        }
    }

    fn get_mut(&mut self, which: Limited) -> &mut Option<f64> {
        match which {
            Limited::Min => &mut self.min,
            Limited::Max => &mut self.max,
            Limited::Step => &mut self.step,
        }
    }

    pub(crate) fn set(&mut self, which: Limited, value: f64) {
        *self.get_mut(which) = Some(value);
    }
}

/// The parsed limit expressions of a parameter.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct LimitExprs {
    min: Option<Expr>,
    max: Option<Expr>,
    step: Option<Expr>,
}

/// The limits in the order they are checked and evaluated.
pub(crate) const ALL_LIMITS: [Limited; 3] = [Limited::Min, Limited::Max, Limited::Step];

impl LimitExprs {
    pub(crate) fn parse(limits: &ParamLimits) -> Result<Self, LimitError> {
        let mut exprs = Self::default();
        for which in ALL_LIMITS {
            if let Some(text) = limits.text(which) {
                let expr = Expr::parse(text).map_err(|error| LimitError::Parse {
                    limit: which,
                    error,
                })?;
                *exprs.get_mut(which) = Some(expr);
            }
        }
        Ok(exprs)
    }

    fn get_mut(&mut self, which: Limited) -> &mut Option<Expr> {
        match which {
            Limited::Min => &mut self.min,
            Limited::Max => &mut self.max,
            Limited::Step => &mut self.step,
        }
    }

    pub(crate) fn get(&self, which: Limited) -> Option<&Expr> {
        match which {
            Limited::Min => self.min.as_ref(),
            Limited::Max => self.max.as_ref(),
            Limited::Step => self.step.as_ref(),
        }
    }

    /// The set limits with their expressions.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (Limited, &Expr)> {
        ALL_LIMITS
            .into_iter()
            .filter_map(move |which| self.get(which).map(|expr| (which, expr)))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

/// What is wrong with a parameter's limits or, for a strict parameter, its
/// value. Values in messages are in the parameter's unit.
#[derive(Debug, Clone, PartialEq)]
pub enum LimitError {
    Parse {
        limit: Limited,
        error: ParseError,
    },
    /// Evaluation failed, including references to unknown names and a
    /// limit of another kind than the parameter (an angle for a length).
    Eval {
        limit: Limited,
        error: EvalError,
    },
    NotFinite(Limited),
    /// The minimum is above the maximum.
    Order {
        min: String,
        max: String,
    },
    StepNotPositive(String),
    /// A strict parameter's result is outside [min, max].
    Outside {
        value: String,
        limit: Limited,
        bound: String,
    },
    /// A strict parameter's result is not on a step.
    OffStep {
        value: String,
        step: String,
    },
    /// Limits of a text parameter, which has none (mitcad#664).
    Text,
}

impl fmt::Display for LimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { limit, error } => write!(f, "{}: {error}", limit.describe()),
            Self::Eval { limit, error } => write!(f, "{}: {error}", limit.describe()),
            Self::NotFinite(limit) => write!(f, "the {} is not a finite value", limit.describe()),
            Self::Order { min, max } => {
                write!(f, "the minimum {min} is above the maximum {max}")
            }
            Self::StepNotPositive(step) => write!(f, "the step must be positive, got {step}"),
            Self::Outside {
                value,
                limit: Limited::Min,
                bound,
            } => write!(f, "{value} is below the minimum {bound}"),
            Self::Outside { value, bound, .. } => {
                write!(f, "{value} is above the maximum {bound}")
            }
            Self::OffStep { value, step } => {
                write!(f, "{value} is not on a step of {step}")
            }
            Self::Text => f.write_str(
                "a text parameter has no minimum, maximum or step: remove them, or give it a \
                 unit of measure first",
            ),
        }
    }
}

impl std::error::Error for LimitError {}

/// Within rounding of `b`: a result that equals a limit up to the last
/// digits of a double is not limited.
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// Checks evaluated limits: finite, min not above max, a positive step.
pub(crate) fn check_bounds(bounds: &Bounds, unit: Unit) -> Result<(), LimitError> {
    for which in ALL_LIMITS {
        if bounds.get(which).is_some_and(|v| !v.is_finite()) {
            return Err(LimitError::NotFinite(which));
        }
    }
    if let (Some(min), Some(max)) = (bounds.min, bounds.max)
        && min > max
        && !near(min, max)
    {
        return Err(LimitError::Order {
            min: format_value(min, unit, DEFAULT_DECIMALS),
            max: format_value(max, unit, DEFAULT_DECIMALS),
        });
    }
    if let Some(step) = bounds.step
        && step <= 0.0
    {
        return Err(LimitError::StepNotPositive(format_value(
            step,
            unit,
            DEFAULT_DECIMALS,
        )));
    }
    Ok(())
}

/// The value a result takes under `bounds` (canonical units), and which
/// limit changed it; a strict parameter's result outside them is an
/// error. See the module's step rule.
pub(crate) fn apply(
    value: f64,
    bounds: &Bounds,
    strict: bool,
    unit: Unit,
) -> Result<(f64, Option<Limited>), LimitError> {
    check_bounds(bounds, unit)?;
    let text = |v: f64| format_value(v, unit, DEFAULT_DECIMALS);
    let mut out = value;
    let mut limited = None;
    if let Some(min) = bounds.min
        && out < min
        && !near(out, min)
    {
        if strict {
            return Err(LimitError::Outside {
                value: text(value),
                limit: Limited::Min,
                bound: text(min),
            });
        }
        out = min;
        limited = Some(Limited::Min);
    }
    if let Some(max) = bounds.max
        && out > max
        && !near(out, max)
    {
        if strict {
            return Err(LimitError::Outside {
                value: text(value),
                limit: Limited::Max,
                bound: text(max),
            });
        }
        out = max;
        limited = Some(Limited::Max);
    }
    if let Some(step) = bounds.step {
        let base = bounds.min.unwrap_or(0.0);
        // Halfway goes up; the tiny margin keeps 0.15 / 0.05 (2.9999...)
        // from going down.
        let steps = ((out - base) / step + 0.5 + 1e-9).floor();
        let mut snapped = base + steps * step;
        if let Some(max) = bounds.max
            && snapped > max
            && !near(snapped, max)
        {
            snapped -= step;
        }
        if let Some(min) = bounds.min
            && snapped < min
        {
            snapped = min;
        }
        if !near(snapped, out) {
            if strict {
                return Err(LimitError::OffStep {
                    value: text(value),
                    step: text(step),
                });
            }
            out = snapped;
            limited = limited.or(Some(Limited::Step));
        }
    }
    Ok((out, limited))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(min: Option<f64>, max: Option<f64>, step: Option<f64>) -> Bounds {
        Bounds { min, max, step }
    }

    #[test]
    fn results_are_clamped_and_snapped() {
        let b = bounds(Some(10.0), Some(300.0), None);
        assert_eq!(
            apply(372.0, &b, false, Unit::MM),
            Ok((300.0, Some(Limited::Max)))
        );
        assert_eq!(
            apply(2.0, &b, false, Unit::MM),
            Ok((10.0, Some(Limited::Min)))
        );
        assert_eq!(apply(20.0, &b, false, Unit::MM), Ok((20.0, None)));
        // Steps from the minimum: 10, 10.2, 10.4, ...; halfway goes up.
        let b = bounds(Some(10.0), Some(11.0), Some(0.2));
        let (v, limited) = apply(10.33, &b, false, Unit::MM).unwrap();
        assert!((v - 10.4).abs() < 1e-12 && limited == Some(Limited::Step));
        let (v, _) = apply(10.1, &b, false, Unit::MM).unwrap();
        assert!((v - 10.2).abs() < 1e-12);
        // From zero without a minimum.
        let b = bounds(None, None, Some(1.0));
        assert_eq!(
            apply(2.4, &b, false, Unit::NONE),
            Ok((2.0, Some(Limited::Step)))
        );
        assert_eq!(
            apply(-2.5, &b, false, Unit::NONE),
            Ok((-2.0, Some(Limited::Step)))
        );
        assert_eq!(apply(3.0, &b, false, Unit::NONE), Ok((3.0, None)));
        // A maximum off the steps: the step below it.
        let b = bounds(Some(0.0), Some(9.5), Some(2.0));
        assert_eq!(
            apply(9.4, &b, false, Unit::NONE),
            Ok((8.0, Some(Limited::Step)))
        );
        assert_eq!(
            apply(50.0, &b, false, Unit::NONE),
            Ok((8.0, Some(Limited::Max)))
        );
    }

    #[test]
    fn strict_limits_refuse_and_bad_limits_are_errors() {
        let b = bounds(Some(10.0), Some(300.0), Some(1.0));
        assert_eq!(
            apply(372.0, &b, true, Unit::MM).unwrap_err().to_string(),
            "372 mm is above the maximum 300 mm"
        );
        assert_eq!(
            apply(2.0, &b, true, Unit::MM).unwrap_err().to_string(),
            "2 mm is below the minimum 10 mm"
        );
        assert_eq!(
            apply(20.5, &b, true, Unit::MM).unwrap_err().to_string(),
            "20.5 mm is not on a step of 1 mm"
        );
        assert_eq!(apply(20.0, &b, true, Unit::MM), Ok((20.0, None)));
        assert_eq!(
            apply(1.0, &bounds(Some(5.0), Some(4.0), None), false, Unit::MM)
                .unwrap_err()
                .to_string(),
            "the minimum 5 mm is above the maximum 4 mm"
        );
        assert_eq!(
            apply(1.0, &bounds(None, None, Some(0.0)), false, Unit::MM)
                .unwrap_err()
                .to_string(),
            "the step must be positive, got 0 mm"
        );
    }
}
