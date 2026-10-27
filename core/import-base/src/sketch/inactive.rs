// SPDX-License-Identifier: MIT
//! Inactive constraints and dimensions of an imported sketch (mitcad#721):
//! where a sketch does not solve to the file's geometry with everything the
//! file constrains it with, the `.f3d` and FreeCAD imports find the
//! constraints and dimensions that keep it off and import those inactive
//! (kept and shown, left out of the solve), everything else active, instead
//! of dropping every constraint and driving dimension of the sketch.
//!
//! - [`fewest`] finds them: first those the file's own positions do not meet
//!   (the solve's `unmet_inactive` with everything inactive), else all;
//!   then it gives back to the solve, in halves (a bisection), every group
//!   the sketch still solves to the file's geometry with.
//! - [`mark`] writes the inactive flag and the reason into the definition's
//!   constraints and dimensions; [`summary`] names them for the import
//!   report and the sketch's import note.

use std::collections::{BTreeSet, VecDeque};

use mitcad_model::sketch::{ConstraintKind, DimensionKind, Ref};
use serde_json::{Value, json};

/// Why the import leaves one inactive (the sketch view's tooltip shows it).
pub(crate) const NOT_MET: &str = "not met in the imported file";

/// The most solves [`fewest`] tries after its first set solves: a large
/// sketch with many bad constraints keeps a few more inactive than it must
/// rather than take minutes.
const MOST_TRIES: usize = 64;

/// The ids of the constraints and dimensions of a definition, in order.
pub fn item_ids(constraints: &[Value], dimensions: &[Value]) -> Vec<String> {
    constraints
        .iter()
        .chain(dimensions)
        .filter_map(|v| v["id"].as_str().map(str::to_owned))
        .collect()
}

/// Constraints or dimensions with those of `inactive` marked inactive, with
/// the reason `NOT_MET`.
pub fn mark(items: &[Value], inactive: &BTreeSet<String>) -> Vec<Value> {
    items
        .iter()
        .map(|v| {
            let mut v = v.clone();
            if v["id"].as_str().is_some_and(|id| inactive.contains(id))
                && let Some(map) = v.as_object_mut()
            {
                map.insert("active".to_owned(), json!(false));
                map.insert("inactive_reason".to_owned(), json!(NOT_MET));
            }
            v
        })
        .collect()
}

/// Whether a constraint or dimension of a definition is active.
pub fn is_active(item: &Value) -> bool {
    item["active"] != false
}

/// The fewest of `all` to leave inactive for the sketch to solve to the
/// file's geometry, as `solves` says of a set (it adds the sketch with
/// that set inactive, checks it and takes it back); the sketch is known to
/// solve with all of them inactive and not to solve with none (the import's
/// level 0), so neither is asked again (mitcad#723). It starts from
/// `candidates` (those the file does not meet) where the sketch solves with
/// those inactive, else from all. Then it tries the groups of the set
/// active again, starting from its two halves (the whole set active again
/// is the sketch with none inactive), and keeps each group the sketch
/// still solves with; a group it does not solve with is halved and its
/// halves tried in turn, so one bad constraint among n takes about
/// 2 log2 n solves. A constraint left inactive this way is one whose
/// activation alone keeps the sketch off (in the order tried).
pub fn fewest(
    candidates: &[String],
    all: &[String],
    solves: &mut dyn FnMut(&BTreeSet<String>) -> bool,
) -> BTreeSet<String> {
    let everything: BTreeSet<String> = all.iter().cloned().collect();
    let start: BTreeSet<String> = candidates
        .iter()
        .filter(|id| everything.contains(*id))
        .cloned()
        .collect();
    let mut inactive = if !start.is_empty() && start != everything && solves(&start) {
        start
    } else {
        everything
    };
    let mut groups: VecDeque<Vec<String>> = VecDeque::new();
    groups.push_back(inactive.iter().cloned().collect());
    let mut tries = 0;
    while let Some(group) = groups.pop_front() {
        if tries >= MOST_TRIES {
            break;
        }
        let trial: BTreeSet<String> = inactive
            .iter()
            .filter(|id| !group.contains(*id))
            .cloned()
            .collect();
        // Every one left active again is the sketch with nothing inactive:
        // known not to solve, so not asked; the group is halved.
        let solved = !trial.is_empty() && {
            tries += 1;
            solves(&trial)
        };
        if solved {
            inactive = trial;
        } else if group.len() > 1 {
            let (a, b) = group.split_at(group.len() / 2);
            groups.push_back(a.to_vec());
            groups.push_back(b.to_vec());
        }
    }
    inactive
}

/// One constraint or dimension of a definition as the report names it:
/// `coincident k7 (p3 on c5)`, `length k9 (c4)`.
pub(crate) fn describe(item: &Value) -> String {
    let id = item["id"].as_str().unwrap_or("?");
    let mut fields = item.clone();
    if let Some(map) = fields.as_object_mut() {
        for key in [
            "id",
            "active",
            "inactive_reason",
            "value",
            "driven",
            "reference",
            "text",
        ] {
            map.remove(key);
        }
    }
    let refs = |refs: Vec<Ref>| -> String {
        refs.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let (kind, on) = if let Ok(c) = serde_json::from_value::<ConstraintKind>(fields.clone()) {
        let on = match &c {
            ConstraintKind::Coincident { point, entity } => {
                format!("{} on {entity}", Ref::Point(*point))
            }
            other => refs(other.refs()),
        };
        (c.type_name(), on)
    } else if let Ok(d) = serde_json::from_value::<DimensionKind>(fields) {
        (d.type_name().to_owned(), refs(d.refs()))
    } else {
        (
            item["type"].as_str().unwrap_or("constraint").to_owned(),
            String::new(),
        )
    };
    let kind = kind.replace('_', " ");
    if on.is_empty() {
        format!("{kind} {id}")
    } else {
        format!("{kind} {id} ({on})")
    }
}

/// The note on what an import left inactive, `None` when nothing:
/// "1 constraint inactive, not met in the file: coincident k7 (p3 on c5)".
/// `constraints` and `dimensions` are the definition's.
pub fn summary(
    constraints: &[Value],
    dimensions: &[Value],
    inactive: &BTreeSet<String>,
) -> Option<String> {
    let of = |items: &[Value]| -> Vec<String> {
        items
            .iter()
            .filter(|v| v["id"].as_str().is_some_and(|id| inactive.contains(id)))
            .map(describe)
            .collect()
    };
    let (k, d) = (of(constraints), of(dimensions));
    if k.is_empty() && d.is_empty() {
        return None;
    }
    let count = |n: usize, what: &str| match n {
        0 => None,
        1 => Some(format!("1 {what}")),
        n => Some(format!("{n} {what}s")),
    };
    let counts: Vec<String> = [count(k.len(), "constraint"), count(d.len(), "dimension")]
        .into_iter()
        .flatten()
        .collect();
    let mut names: Vec<String> = k.into_iter().chain(d).collect();
    const SHOWN: usize = 8;
    if names.len() > SHOWN {
        let more = names.len() - SHOWN;
        names.truncate(SHOWN);
        names.push(format!("and {more} more"));
    }
    Some(format!(
        "{} inactive, not met in the file: {}",
        counts.join(" and "),
        names.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_fewest_bad_constraints_are_found_by_halves() {
        let all: Vec<String> = (1..=40).map(|n| format!("k{n}")).collect();
        // k7 and k31 keep the sketch off: it solves only without both.
        let bad = set(&["k7", "k31"]);
        let mut tries = 0;
        let mut solves = |inactive: &BTreeSet<String>| {
            tries += 1;
            bad.is_subset(inactive)
        };
        assert_eq!(fewest(&[], &all, &mut solves), bad);
        assert!(tries < 30, "{tries} solves");
        // From the candidates the file does not meet: one more of them that
        // is fine goes back to the solve.
        let candidates: Vec<String> = ["k7", "k31", "k12"].map(str::to_owned).to_vec();
        let mut tries = 0;
        let mut solves = |inactive: &BTreeSet<String>| {
            tries += 1;
            bad.is_subset(inactive)
        };
        assert_eq!(fewest(&candidates, &all, &mut solves), bad);
        assert!(tries < 8, "{tries} solves");
        // Candidates that are not enough: from all of them.
        let mut solves = |inactive: &BTreeSet<String>| bad.is_subset(inactive);
        assert_eq!(fewest(&["k7".to_owned()], &all, &mut solves), bad);
    }

    #[test]
    fn the_search_never_asks_for_nothing_or_everything_inactive() {
        // The import knows both already (mitcad#723): nothing inactive does
        // not solve (level 0), everything inactive does (level 1's first
        // add).
        let all: Vec<String> = (1..=16).map(|n| format!("k{n}")).collect();
        let everything: BTreeSet<String> = all.iter().cloned().collect();
        let bad = set(&["k5"]);
        let mut asked: Vec<BTreeSet<String>> = Vec::new();
        let mut solves = |inactive: &BTreeSet<String>| {
            asked.push(inactive.clone());
            bad.is_subset(inactive)
        };
        assert_eq!(fewest(&[], &all, &mut solves), bad);
        assert!(
            asked.iter().all(|s| !s.is_empty() && *s != everything),
            "{asked:?}"
        );
        // One bad among 16: the halves down to it, two solves a level.
        assert!(asked.len() <= 8, "{} solves: {asked:?}", asked.len());
        // A single candidate that solves: nothing more to try.
        let mut asked = 0;
        let mut solves = |inactive: &BTreeSet<String>| {
            asked += 1;
            assert!(!inactive.is_empty());
            bad.is_subset(inactive)
        };
        assert_eq!(fewest(&["k5".to_owned()], &all, &mut solves), bad);
        assert_eq!(asked, 1, "only the candidate itself");
    }

    #[test]
    fn inactive_items_are_marked_and_named() {
        let constraints = vec![
            json!({"id": "k7", "type": "coincident", "point": "p3", "entity": "c5"}),
            json!({"id": "k8", "type": "horizontal", "line": "c4"}),
        ];
        let dimensions = vec![json!({"id": "k9", "type": "length", "line": "c4", "value": "d1"})];
        let inactive = set(&["k7", "k9"]);
        let marked = mark(&constraints, &inactive);
        assert_eq!(marked[0]["active"], false);
        assert_eq!(marked[0]["inactive_reason"], NOT_MET);
        assert!(is_active(&marked[1]) && marked[1].get("active").is_none());
        assert_eq!(
            summary(&constraints, &dimensions, &inactive).as_deref(),
            Some(
                "1 constraint and 1 dimension inactive, not met in the file: \
                 coincident k7 (p3 on c5), length k9 (c4)"
            )
        );
        assert_eq!(summary(&constraints, &dimensions, &set(&[])), None);
    }
}
