// SPDX-License-Identifier: MIT
//! Insert Point on a spline (mitcad#836): a point where the spline is
//! nearest to a place picked near it.
//!
//! - A fitted spline gets a fit point there, between its two neighbours in
//!   the curve's order (a closed one between its last point and its first
//!   where the pick is on that stretch). The curve still passes through
//!   every fit point; between the new point's neighbours its shape may
//!   change a little, as the interpolation now has one more point.
//! - A spline by control points gets a control point by knot insertion
//!   ([`Nurbs::insert_knot`]): the same curve, exactly, with one more
//!   control point. The control points next to the new one move to keep
//!   the shape; their ids, and so their constraints and dimensions, stay.
//!
//! Closed splines work alike. Every existing point keeps its id, its
//! constraints and its dimensions.

use super::EntityKind;
use super::edit::SketchEdit;
use super::geometry::{Curve2, Nurbs, P2, dist};
use super::solve::Solved;
use crate::ids::EntityUid;

impl SketchEdit<'_> {
    /// Inserts a point on the spline `id` where it is nearest to `at` and
    /// returns the new point (see the module documentation). Refused for a
    /// curve that is no spline, a projected spline or one a derived offset
    /// computes, a place on an existing fit point or at the ends of a
    /// spline by control points, and where the knot is there as often as
    /// the degree (the curve passes through a control point there already).
    pub fn insert_spline_point(&mut self, id: EntityUid, at: P2) -> Result<EntityUid, String> {
        if !at.iter().all(|v| v.is_finite()) {
            return Err(format!("the place must be finite, got {at:?}"));
        }
        let kind = self.curve(id)?.kind.clone();
        if !matches!(
            kind,
            EntityKind::Spline { .. } | EntityKind::FittedSpline { .. }
        ) {
            return Err(format!(
                "c{} is a {}: points are inserted into splines only",
                id.0,
                kind.type_name()
            ));
        }
        self.shape_editable(id)?;
        let solved = self.solved()?;
        let curve = solved
            .curves
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("curve c{} is not solved", id.0))?;
        let Curve2::Nurbs(spline) = &curve else {
            return Err(format!("c{} is not solved as a spline", id.0));
        };
        let (t, _) = curve.closest(at);
        match kind {
            EntityKind::FittedSpline { points, closed } => {
                self.insert_fit_point(id, &solved, spline, &points, closed, t)
            }
            EntityKind::Spline { control, .. } => {
                self.insert_control_point(id, spline, &control, t)
            }
            _ => unreachable!("a spline"),
        }
    }

    /// A fit point at the parameter `t` of the solved fitted spline.
    fn insert_fit_point(
        &mut self,
        id: EntityUid,
        solved: &Solved,
        spline: &Nurbs,
        points: &[EntityUid],
        closed: bool,
        t: f64,
    ) -> Result<EntityUid, String> {
        if points.len() < 2 {
            return Err(format!("c{} has fewer than two fit points", id.0));
        }
        let mut fit = points
            .iter()
            .map(|p| {
                solved
                    .points
                    .get(p)
                    .copied()
                    .ok_or_else(|| format!("point p{} is not solved", p.0))
            })
            .collect::<Result<Vec<P2>, String>>()?;
        if closed {
            // On from the last point back to the first.
            fit.push(fit[0]);
        }
        // The fit points' parameters: chord lengths over the domain, as the
        // solver interpolates them (`mitcad_solver`'s `interpolate`).
        let mut total = 0.0;
        let mut lengths = vec![0.0];
        for w in fit.windows(2) {
            total += dist(w[0], w[1]);
            lengths.push(total);
        }
        if total <= 0.0 {
            return Err(format!("c{} has no length", id.0));
        }
        let (lo, hi) = spline.domain();
        let params: Vec<f64> = lengths.iter().map(|s| lo + (hi - lo) * s / total).collect();
        // The stretch between the fit points `i` and `i + 1` that holds `t`.
        let i = params[1..]
            .iter()
            .position(|end| t < *end)
            .unwrap_or(params.len() - 2);
        let on = spline.eval(t)[0];
        let tolerance = 1e-9 * total.max(1.0);
        if dist(on, fit[i]) <= tolerance || dist(on, fit[i + 1]) <= tolerance {
            return Err("a fit point is there already".to_owned());
        }
        let point = self.add_point(on);
        if let Some(EntityKind::FittedSpline { points, .. }) =
            self.def.entity_mut(id).map(|e| &mut e.kind)
        {
            // After the last point of a closed spline is before its first.
            points.insert(i + 1, point);
        }
        Ok(point)
    }

    /// A control point by knot insertion at the parameter `t` of the solved
    /// spline.
    fn insert_control_point(
        &mut self,
        id: EntityUid,
        spline: &Nurbs,
        control: &[EntityUid],
        t: f64,
    ) -> Result<EntityUid, String> {
        if control.len() != spline.control.len() {
            return Err(format!("c{} is not solved with its control points", id.0));
        }
        let (lo, hi) = spline.domain();
        if (t - lo).abs() <= 1e-9 * (hi - lo) || (hi - t).abs() <= 1e-9 * (hi - lo) {
            return Err(
                "the place is at an end of the spline (where a closed one closes): \
                 pick a place between its ends"
                    .to_owned(),
            );
        }
        let refined = spline
            .insert_knot(t)
            .ok_or_else(|| "the spline passes through a control point there already".to_owned())?;
        // The old points that move: one the spline uses twice cannot be in
        // two places.
        let moved = &control[refined.first..refined.last];
        if let Some(twice) = moved
            .iter()
            .find(|p| control.iter().filter(|q| q == p).count() > 1)
        {
            return Err(format!(
                "p{} is twice a control point of c{} and cannot move to keep its shape",
                twice.0, id.0
            ));
        }
        let new = &refined.spline;
        // Of the new places, the one nearest to the curve at the knot is
        // the new point's; the moved points keep their order around it.
        let on = new.eval(refined.knot)[0];
        let fresh = (refined.first..=refined.last)
            .min_by(|a, b| dist(new.control[*a], on).total_cmp(&dist(new.control[*b], on)))
            .expect("one new place at least");
        let point = self.add_point(new.control[fresh]);
        let mut ids = Vec::with_capacity(control.len() + 1);
        ids.extend_from_slice(&control[..fresh]);
        ids.push(point);
        ids.extend_from_slice(&control[fresh..]);
        for (uid, at) in ids
            .iter()
            .zip(&new.control)
            .take(refined.last + 1)
            .skip(refined.first)
        {
            if let Some(EntityKind::Point { at: place }) =
                self.def.entity_mut(*uid).map(|e| &mut e.kind)
            {
                *place = *at;
            }
        }
        let e = self.def.entity_mut(id).expect("a curve");
        if let EntityKind::Spline {
            control,
            weights,
            knots,
            ..
        } = &mut e.kind
        {
            *control = ids;
            *weights = new.weights.clone();
            *knots = new.knots.clone();
        }
        Ok(point)
    }
}
