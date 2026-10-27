// SPDX-License-Identifier: MIT
//! What the imports share (mitcad#583): the rewriting of the `$k`
//! placeholders in the definitions of a candidate, the core of a candidate
//! (its definitions), and the end of an import. The formats differ in
//! everything around them (the outcomes, the reports, what a candidate
//! assumes), which stays in the formats.

use mitcad_model::{Document, FeatureUid, Kernel};
use serde_json::Value;

/// Replaces the strings `"$k"` in a definition by the uid of the k-th
/// feature added before it (the features of one candidate are added in
/// order, as `mitcad_import`'s `Importer::add_all` adds them), also as the
/// prefix of a face or body name (`"$0:hole0.wall"`, `"$1.b0"`).
///
/// A placeholder is the whole string, or the start of one whose digits are
/// followed by `:` or `.`; a string such as `"$1x"`, or a `$k` that names no
/// feature added so far, is left as it is. The digits are read in full:
/// `"$12"` is the thirteenth feature, never the second one followed by `2`.
pub fn substitute(def: &mut Value, uids: &[FeatureUid]) {
    match def {
        Value::String(s) => {
            if let Some(rest) = s.strip_prefix('$') {
                let digits = rest
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(rest.len());
                let (k, tail) = rest.split_at(digits);
                let fits = tail.is_empty() || tail.starts_with([':', '.']);
                if fits && let Some(uid) = k.parse::<usize>().ok().and_then(|k| uids.get(k)) {
                    *s = format!("{uid}{tail}");
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| substitute(v, uids)),
        Value::Object(map) => map.values_mut().for_each(|v| substitute(v, uids)),
        _ => {}
    }
}

/// What every import's candidate holds: the definitions to try for an
/// item or an object, one or more Mitcad features. A later definition may
/// refer to an earlier one as `$0`, `$1`, ... ([`substitute`]). The
/// formats' candidates add their own fields (what the candidate assumes,
/// the notes of the report, how the definitions are named) and reach the
/// definitions as `candidate.defs`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CandidateCore {
    pub defs: Vec<Value>,
}

impl CandidateCore {
    /// One definition.
    pub fn new(def: Value) -> Self {
        Self { defs: vec![def] }
    }

    /// Several, in the order they are added.
    pub fn of(defs: Vec<Value>) -> Self {
        Self { defs }
    }
}

/// The end of an import into `doc` that began at undo depth `depth`: the
/// notes (those of features that still exist; the notes of one feature
/// joined by `"; "`) go on the timeline, then the import becomes one undo
/// step labelled `undo_label` when there is one. The result is what went
/// wrong with the notes, if anything; the formats do what they did with it.
/// What follows (the check against a reference) is the format's.
pub fn finish_import<K: Kernel>(
    doc: &mut Document<K>,
    depth: usize,
    notes: Vec<(FeatureUid, String)>,
    undo_label: Option<&str>,
) -> Option<String> {
    let mut kept: Vec<(FeatureUid, String)> = notes
        .into_iter()
        .filter(|(uid, _)| doc.feature(*uid).is_some())
        .collect();
    kept.sort();
    kept.dedup_by(|a, b| {
        if a.0 == b.0 {
            b.1 = format!("{}; {}", b.1, a.1);
            true
        } else {
            false
        }
    });
    let failed = doc.set_import_notes(&kept).err().map(|e| e.to_string());
    if let Some(label) = undo_label {
        doc.merge_undo(depth, label);
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn uids(count: u64) -> Vec<FeatureUid> {
        (1..=count).map(FeatureUid).collect()
    }

    fn substituted(text: &str, uids: &[FeatureUid]) -> Value {
        let mut def = json!(text);
        substitute(&mut def, uids);
        def
    }

    #[test]
    fn a_placeholder_is_replaced_by_the_uid_of_that_feature() {
        let uids = uids(3);
        assert_eq!(substituted("$0", &uids), json!(uids[0].to_string()));
        assert_eq!(substituted("$2", &uids), json!(uids[2].to_string()));
        assert_eq!(
            substituted("$1:hole0.wall", &uids),
            json!(format!("{}:hole0.wall", uids[1]))
        );
        assert_eq!(
            substituted("$1.b0", &uids),
            json!(format!("{}.b0", uids[1]))
        );
    }

    #[test]
    fn a_placeholder_reads_all_its_digits_so_1_and_12_differ() {
        // `$1` is the second feature; `$12` the thirteenth, not the second
        // followed by a 2.
        let many = uids(13);
        assert_eq!(substituted("$1", &many), json!(many[1].to_string()));
        assert_eq!(substituted("$12", &many), json!(many[12].to_string()));
        assert_eq!(
            substituted("$12.b0", &many),
            json!(format!("{}.b0", many[12]))
        );
        // Without a thirteenth feature `$12` names none: it stays.
        let few = uids(2);
        assert_eq!(substituted("$12", &few), json!("$12"));
        assert_eq!(substituted("$12:region", &few), json!("$12:region"));
    }

    #[test]
    fn a_string_that_is_not_a_whole_placeholder_is_left_alone() {
        let uids = uids(3);
        for text in ["$", "$a", "$:region", "$1x", "$1_b0", "$1 ", "x$1", "1", ""] {
            assert_eq!(substituted(text, &uids), json!(text), "{text:?}");
        }
        // A `$k` beyond the features added so far.
        assert_eq!(substituted("$3", &uids), json!("$3"));
    }

    #[test]
    fn placeholders_are_replaced_inside_arrays_and_objects_only_in_strings() {
        let uids = uids(2);
        let mut def = json!({
            "plane": "$0",
            "profiles": [{"sketch": "$1", "region": "$1:region"}, 1, null, true],
            "name": "$1x"
        });
        substitute(&mut def, &uids);
        assert_eq!(
            def,
            json!({
                "plane": uids[0].to_string(),
                "profiles": [
                    {"sketch": uids[1].to_string(), "region": format!("{}:region", uids[1])},
                    1, null, true
                ],
                "name": "$1x"
            })
        );
    }

    #[test]
    fn a_candidate_core_keeps_its_definitions_in_order() {
        let one = CandidateCore::new(json!({"type": "sketch"}));
        assert_eq!(one.defs, vec![json!({"type": "sketch"})]);
        let two = CandidateCore::of(vec![json!(1), json!(2)]);
        assert_eq!(two.defs, vec![json!(1), json!(2)]);
    }
}
