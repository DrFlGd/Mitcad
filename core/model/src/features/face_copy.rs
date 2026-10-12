// SPDX-License-Identifier: MIT
//! The optimized compute option of a pattern or mirror of features that
//! has fillets or chamfers among them: it copies the faces the features
//! made instead of working the fillets and chamfers out again on each
//! copy.
//!
//! What the features did to a body is what lies between the body before
//! the first of them and the body after the last ([`Kernel::feature_changes`]):
//! the material they added (a boss with the roundings at its foot and on
//! its top) and the material they removed (a pocket with its chamfer),
//! each a closed solid bounded by the faces the features made. The
//! pattern moves those solids to every element, as the placed copy of a
//! feature's tool is moved (`pattern.rs`), cuts the removed ones from the
//! body and joins the added ones to it, each in one operation: the copies
//! carry the roundings' faces, no rounding is evaluated at an element (a
//! rounding that OCCT fails on at the 146th of 1078 copies of a pin is not
//! tried again there).
//!
//! A copy of added material stands on the faces under its seam (the faces
//! of the solid that lay on the body): where those faces end (a groove
//! beside the boss, the edge of a plate) the material over the part of the
//! seam that nothing lies under is left out, and a copy with nothing under
//! its seam is left out whole ([`Kernel::supported_copies`]).
//!
//! The features are taken in runs: consecutive features of the timeline
//! that no other body-changing feature separates (the boss and its two
//! roundings), one pair of bodies, before the run and after it, for each
//! body the run changes. A run whose features do not stand at the same
//! places (patterns of patterns that copy only some of them), that makes
//! or removes bodies (its changes are not a change of a body) or that
//! changes a body that did not exist before it is not a change of bodies
//! this way: [`copy_faces`] returns false and the pattern repeats the
//! features as the other compute options do.
//!
//! The copies act on the bodies the features changed; copies that touch no
//! body are left out, as Adjust leaves them out.

use super::bodies::{BodySet, apply_tool, apply_tool_strays};
use super::dressup_copies::input_shape;
use super::pattern::{Element, Placed, placed_copy, reaching};
use super::{EvalContext, Operation};
use crate::ids::{BodyUid, FeatureUid};
use crate::kernel::{AddedMaterial, Kernel};
use crate::recompute::{Change, FeatureResult};
use crate::transform::{Instance, Transform};

type Steps = Vec<(Transform, Instance)>;

/// Copies the faces of the features at `places` to `elements`; false
/// (the bodies untouched) when the features are not a change of bodies
/// this way (see the module).
pub(super) fn copy_faces<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    set: &mut BodySet<K::Shape>,
    places: &[Placed],
    elements: &[Element],
) -> Result<bool, String> {
    if elements.is_empty() {
        return Ok(false);
    }
    let history = ctx.env.history;
    // The features the places stand for, by their place in the timeline.
    let mut features: Vec<(usize, FeatureUid)> = Vec::new();
    for place in places {
        if features.iter().any(|(_, f)| *f == place.feature) {
            continue;
        }
        let Some(at) = history.iter().position(|r| r.uid == place.feature) else {
            return Ok(false);
        };
        // (The read, and the error of a feature that is suppressed or
        // failed.)
        ctx.feature_changed_bodies(place.feature)?;
        features.push((at, place.feature));
    }
    features.sort_unstable();
    let steps_of = |uid: FeatureUid| -> Vec<&Steps> {
        places
            .iter()
            .filter(|p| p.feature == uid)
            .map(|p| &p.steps)
            .collect()
    };

    // Runs of features with no body-changing feature of another kind
    // between them.
    let seeds: Vec<usize> = features.iter().map(|(at, _)| *at).collect();
    let mut runs: Vec<Vec<usize>> = Vec::new();
    for &at in &seeds {
        match runs.last_mut() {
            Some(run) if !separated(history, &seeds, run[run.len() - 1], at) => run.push(at),
            _ => runs.push(vec![at]),
        }
    }

    // What each run changed, before anything is applied.
    let mut changes = Vec::new();
    for run in &runs {
        let first_places = steps_of(history[run[0]].uid);
        if run
            .iter()
            .any(|at| steps_of(history[*at].uid) != first_places)
        {
            return Ok(false);
        }
        let mut bodies: Vec<BodyUid> = Vec::new();
        for at in run {
            let Some(output) = &history[*at].output else {
                return Ok(false);
            };
            for change in &output.changes {
                match change {
                    Change::Set(body, _) if !bodies.contains(body) => bodies.push(*body),
                    Change::Set(..) => {}
                    Change::Remove(_) => return Ok(false),
                }
            }
        }
        for body in bodies {
            let sets = |at: &&usize| history[**at].shape_set(body).is_some();
            let (Some(first), Some(last)) = (run.iter().find(sets), run.iter().rev().find(sets))
            else {
                return Ok(false);
            };
            let Some(before) = input_shape(ctx, history[*first].uid, body) else {
                return Ok(false);
            };
            let Some(after) = history[*last].shape_set(body) else {
                return Ok(false);
            };
            let found = ctx
                .kernel
                .feature_changes(&before, after)
                .map_err(|e| e.to_string())?;
            changes.push((body, first_places.clone(), found));
        }
    }

    // The copies: the removed material cut from the body, then the added
    // material, the parts of it that stand on the body, joined to it.
    let mut reached = false;
    for (body, stepsets, found) in &changes {
        let removed = copies(ctx, &found.removed, stepsets, elements)?;
        let removed = reaching(ctx, set, &[*body], removed);
        if let Some(tool) = united(ctx, removed)? {
            reached |= apply_tool(ctx, set, Operation::Cut, &[*body], &tool)?;
        }
        let mut stands = Vec::new();
        for added in &found.added {
            stands.extend(placed_added(ctx, set, *body, added, stepsets, elements)?);
        }
        if let Some(tool) = united(ctx, stands)? {
            reached |= apply_tool_strays(ctx, set, Operation::Join, &[*body], &tool, false)?;
        }
    }
    if !reached {
        let name = ctx.feature_name(features[0].1);
        return Err(format!("no copy of the faces of {name} reaches a body"));
    }
    Ok(true)
}

/// Whether a feature that is none of the `seeds` and changes bodies lies
/// between the results at `from` and `to`.
fn separated<S>(history: &[FeatureResult<S>], seeds: &[usize], from: usize, to: usize) -> bool {
    (from + 1..to).any(|at| {
        !seeds.contains(&at)
            && history[at]
                .output
                .as_ref()
                .is_some_and(|output| !output.changes.is_empty())
    })
}

/// The pieces at every place of the features and every element.
fn copies<K: Kernel>(
    ctx: &EvalContext<'_, K>,
    pieces: &[K::Shape],
    stepsets: &[&Steps],
    elements: &[Element],
) -> Result<Vec<K::Shape>, String> {
    let mut out = Vec::with_capacity(pieces.len() * stepsets.len() * elements.len());
    for piece in pieces {
        for steps in stepsets {
            for element in elements {
                out.push(placed_copy(
                    ctx,
                    piece,
                    steps,
                    &element.transform,
                    instance(ctx, element),
                    false,
                )?);
            }
        }
    }
    Ok(out)
}

/// The parts of the copies of added material that stand on the body now.
fn placed_added<K: Kernel>(
    ctx: &EvalContext<'_, K>,
    set: &BodySet<K::Shape>,
    body: BodyUid,
    added: &AddedMaterial<K::Shape>,
    stepsets: &[&Steps],
    elements: &[Element],
) -> Result<Vec<K::Shape>, String> {
    let mut copies: Vec<(K::Shape, Option<K::Shape>)> =
        Vec::with_capacity(stepsets.len() * elements.len());
    for steps in stepsets {
        for element in elements {
            let at = |shape: &K::Shape| {
                placed_copy(
                    ctx,
                    shape,
                    steps,
                    &element.transform,
                    instance(ctx, element),
                    false,
                )
            };
            let solid = at(&added.solid)?;
            let seam = added.seam.as_ref().map(at).transpose()?;
            copies.push((solid, seam));
        }
    }
    let shape = set
        .get(body)
        .ok_or_else(|| format!("body {body} does not exist at this point of the timeline"))?;
    let pairs: Vec<(&K::Shape, Option<&K::Shape>)> =
        copies.iter().map(|(s, m)| (s, m.as_ref())).collect();
    Ok(ctx
        .kernel
        .supported_copies(shape, &pairs)
        .map_err(|e| e.to_string())?
        .into_iter()
        .flatten()
        .collect())
}

fn instance<K: Kernel>(ctx: &EvalContext<'_, K>, element: &Element) -> Instance {
    Instance {
        feature: ctx.uid,
        index: element.index,
    }
}

/// The copies as one shape; None when there are none.
fn united<K: Kernel>(
    ctx: &EvalContext<'_, K>,
    mut copies: Vec<K::Shape>,
) -> Result<Option<K::Shape>, String> {
    if copies.len() <= 1 {
        return Ok(copies.pop());
    }
    let refs: Vec<&K::Shape> = copies.iter().collect();
    ctx.kernel.unite(&refs).map(Some).map_err(|e| e.to_string())
}
