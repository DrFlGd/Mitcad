// SPDX-License-Identifier: MIT
//! Profiled boards (mitcad#477; `commands.md`, "BUILD: timber
//! construction", *Profiled boards*): tongue and groove, shiplap,
//! feather-edge and deck boards. Their `board` block ([`super::BoardMeta`])
//! gives the width they cover; a row of boards that join is laid edge to
//! edge at that width (`spacing.rs`), a board placed on another's `tongue`
//! or `groove` frame mates its own other edge there, and the cut list
//! adds their profile, cover width and the area they cover
//! (`cut_list.rs`).

use super::BuildMeta;

/// The frame kind a board mates with a picked edge frame of another: its
/// `groove` on a `tongue`, its `tongue` in a `groove`.
pub(crate) fn mating_edge(kind: &str) -> Option<&'static str> {
    match kind {
        "tongue" => Some("groove"),
        "groove" => Some("tongue"),
        _ => None,
    }
}

/// Why a board does not join another, if it does not: both profiled, of
/// one profile and thickness, and with the same tongue (its length and
/// thickness) or overlap, each within 0.01 mm.
pub(crate) fn check_match(
    new: &BuildMeta,
    new_label: &str,
    target: &BuildMeta,
    target_label: &str,
) -> Result<(), String> {
    let (Some(a), Some(b)) = (&new.board, &target.board) else {
        return Err(format!(
            "{new_label} and {target_label} are not both profiled boards"
        ));
    };
    if a.profile != b.profile {
        return Err(format!(
            "the profiles do not match: {new_label} is {}, {target_label} {}",
            a.profile_text(),
            b.profile_text()
        ));
    }
    let same = |x: Option<f64>, y: Option<f64>| match (x, y) {
        (Some(x), Some(y)) => (x - y).abs() < 0.01,
        (None, None) => true,
        _ => false,
    };
    let thickness = |m: &BuildMeta| m.section_size().map(|[_, t]| t);
    if !same(thickness(new), thickness(target)) {
        return Err(format!(
            "the profiles do not match: {new_label} is {} mm thick, {target_label} {} mm",
            super::round(thickness(new).unwrap_or(f64::NAN)),
            super::round(thickness(target).unwrap_or(f64::NAN))
        ));
    }
    let tongue = |m: &super::BoardMeta| {
        m.tongue.as_ref().map(|t| {
            (
                t.length.as_ref().and_then(super::MetaValue::millimetres),
                t.thickness.as_ref().and_then(super::MetaValue::millimetres),
            )
        })
    };
    let (ta, tb) = (tongue(a), tongue(b));
    let tongues_match = match (ta, tb) {
        (Some((la, ha)), Some((lb, hb))) => same(la, lb) && same(ha, hb),
        (None, None) => true,
        _ => false,
    };
    let overlap = |m: &super::BoardMeta| m.overlap.as_ref().and_then(super::MetaValue::millimetres);
    if !tongues_match || !same(overlap(a), overlap(b)) {
        return Err(format!(
            "the profiles do not match: the {} of {new_label} and {target_label} differ",
            if ta.is_some() || tb.is_some() {
                "tongues"
            } else {
                "overlaps"
            }
        ));
    }
    Ok(())
}
