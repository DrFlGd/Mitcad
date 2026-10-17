// SPDX-License-Identifier: MIT
//! The profile regions an extrusion or revolution selected, from the loops
//! the stream decoder reads (`_f3d_profile_loops` of its detail, mitcad#96;
//! the grammar in `mitcad_f3d`'s `design/build/profiles.rs`).
//!
//! [`mapped_regions`] maps them onto Mitcad's regions of the sketch, so
//! that the import tries those regions first and the history's guesses
//! after them (*What the history settles* in the README). It is a function
//! of its own so that a better mapping can replace it.
//!
//! The rule *(found on the learning dump of the private corpora: it gives
//! the regions the history accepted for 1473 of 1552 extrusions, most of
//! the others sets the history cannot tell from them)*: a listed profile
//! is a face, its outer loops less its hole loops; a region is selected
//! when its inside point lies inside a profile by the even-odd rule
//! against the listed curve pieces. A record names piece `piece` of
//! `pieces` of a curve cut at its crossings with the other curves of the
//! same loop, numbered along the curve's parameter (a line from its start,
//! an arc from its start angle, a circle from its x axis counter-clockwise,
//! the piece holding that point first). The geometry is the file's (sketch
//! space, cm), so that the pieces are numbered as the file numbers them;
//! curves the sketch no longer has (a profile older than the sketch's last
//! edit) are left out.
//!
//! A curve cut into as many pieces as the file counts is numbered as the
//! file numbers it; where the cuts of the record's loop do not give the
//! file's count, the profile's other curves and then the sketch's are tried.
//! A hole whose records name no curve of the sketch any more is dropped.
//! Ellipses, elliptical arcs and splines ([`curves`]) take part like lines,
//! arcs and circles; where their cuts contradict the file's count the
//! rule gives way to the next one.
//!
//! Where a profile has curves the rule cannot use (a kind it does not follow,
//! or cuts the file contradicts) or selects nothing, its loops select the regions whose outer
//! loops share most of their curves (at least half of the two together),
//! the pieces of a profile split by curves inside it (its children) each
//! their own; a sketch of one region gives that one.

mod curves;

use std::collections::{BTreeSet, HashMap, HashSet};

use mitcad_f3d::design::ir::Geometry;
use serde_json::Value;

use curves::{Curve, closes, inside, pieces};

/// One record of a loop: the curve's sketch-local id (where the sketch
/// has it) and its ids in the file, and which piece.
#[derive(Debug, Clone, PartialEq)]
struct Piece {
    id: Option<String>,
    key: (u64, u64),
    piece: usize,
    /// How many pieces the file says the curve is cut into.
    pieces: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct Loop {
    outer: bool,
    pieces: Vec<Piece>,
}

#[derive(Debug, Clone, PartialEq)]
struct Profile {
    loops: Vec<Loop>,
    children: Vec<Vec<Loop>>,
}

fn loops_of(v: &Value) -> Option<Vec<Loop>> {
    v.as_array()?
        .iter()
        .map(|l| {
            let pieces = l["curves"]
                .as_array()?
                .iter()
                .map(|c| {
                    Some(Piece {
                        id: c["id"].as_str().map(str::to_owned),
                        key: (c["primary"].as_u64()?, c["secondary"].as_u64()?),
                        piece: c["piece"].as_u64()? as usize,
                        pieces: c["pieces"].as_u64()? as usize,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Loop {
                outer: l["outer"].as_bool()?,
                pieces,
            })
        })
        .collect()
}

fn profiles_of(v: &Value) -> Option<Vec<Profile>> {
    v.as_array()?
        .iter()
        .map(|p| {
            Some(Profile {
                loops: loops_of(&p["loops"])?,
                children: p["children"]
                    .as_array()?
                    .iter()
                    .map(loops_of)
                    .collect::<Option<Vec<_>>>()?,
            })
        })
        .collect()
}

/// How many curves the decoded loops (`_f3d_profile_loops`) name when
/// none of their records names a curve the sketch has (the profile is
/// older than the sketch's last edit, which removed its curves): what the
/// note of an item that does not give its state says (mitcad#379). None
/// when a record names a curve of the sketch or the loops do not read.
pub(crate) fn gone_curves(loops: &Value) -> Option<usize> {
    let profiles = profiles_of(loops)?;
    let records = || {
        profiles
            .iter()
            .flat_map(|p| p.loops.iter().chain(p.children.iter().flatten()))
            .flat_map(|l| l.pieces.iter())
    };
    if records().next().is_none() || records().any(|r| r.id.is_some()) {
        return None;
    }
    let keys: BTreeSet<(u64, u64)> = records().map(|r| r.key).collect();
    Some(keys.len())
}

/// A region of Mitcad's sketch as [`mapped_regions`] sees it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RegionView {
    /// A point inside it (sketch mm).
    pub inside: Option<[f64; 2]>,
    /// The file's ids of the curves of its outer loop; None when one is
    /// Mitcad's own.
    pub outer: Option<Vec<String>>,
}

/// The regions (indices into `regions`) the decoded loops `loops`
/// (`_f3d_profile_loops`) select, by the module's rule; `curves` gives the
/// file's geometry of the sketch's curves by their ids. None when the loops
/// do not read or select nothing.
#[cfg(test)]
pub(crate) fn mapped_regions(
    loops: &Value,
    curves: &HashMap<String, Geometry>,
    regions: &[RegionView],
) -> Option<Vec<usize>> {
    mapped(loops, curves, regions).map(|(regions, _)| regions)
}

/// [`mapped_with`] without construction curves (for the tests).
#[cfg(test)]
pub(crate) fn mapped(
    loops: &Value,
    curves: &HashMap<String, Geometry>,
    regions: &[RegionView],
) -> Option<(Vec<usize>, bool)> {
    mapped_with(loops, curves, &HashSet::new(), regions)
}

/// The selected regions and whether the selection is *exact* (mitcad#186):
/// every profile's regions came from the even-odd rule alone, with every
/// curve piece the profile lists found in the file's sketch and numbered
/// (no listed curve missing, no piece beyond the curve's cuts, no
/// by-curve-overlap fallback); only an exact selection counts as read.
/// `construction` holds the ids of the sketch's construction curves, which cut
/// no profile.
pub(crate) fn mapped_with(
    loops: &Value,
    curves: &HashMap<String, Geometry>,
    construction: &HashSet<String>,
    regions: &[RegionView],
) -> Option<(Vec<usize>, bool)> {
    let profiles = profiles_of(loops)?;
    let mut out = BTreeSet::new();
    let mut exact = true;
    for p in &profiles {
        match by_geometry(p, curves, construction, regions).filter(|(f, _)| !f.is_empty()) {
            Some((found, complete)) => {
                exact &= complete;
                out.extend(found);
            }
            None => {
                exact = false;
                out.extend(by_curves(p, regions));
            }
        }
    }
    // A sketch with one region has one choice: whatever the profile's
    // records say (a profile older than the sketch's last edit lists curves
    // the sketch has since replaced), the selection is that region.
    let exact = exact || (regions.len() == 1 && !out.is_empty());
    (!out.is_empty()).then(|| (out.into_iter().collect(), exact))
}

/// The regions inside a profile's loops, and whether every record of its
/// loops took part (found in the sketch, its piece numbered); None when a
/// curve it lists that the sketch has is no line, arc or circle.
fn by_geometry(
    p: &Profile,
    curves: &HashMap<String, Geometry>,
    construction: &HashSet<String>,
    regions: &[RegionView],
) -> Option<(Vec<usize>, bool)> {
    // The curves of the whole profile, and of the whole sketch, for the
    // records whose curve the other curves of their loop do not cut into
    // as many pieces as the file counts (built when one needs them).
    let mut profile_cutters: Option<Vec<(String, Curve)>> = None;
    let mut sketch_cutters: Option<Vec<(String, Curve)>> = None;
    let mut listed = Vec::new();
    let mut complete = true;
    for l in &p.loops {
        // A hole whose curves are all gone from the sketch is no hole any
        // more: the profile is what its other loops enclose now (the
        // sketch was edited after the profile; the material inside it is
        // selected with the rest).
        if !l.outer && !l.pieces.is_empty() && l.pieces.iter().all(|r| r.id.is_none()) {
            if crate::tracing() {
                eprintln!(
                    "import: profile hole of {} records names no curve of the sketch: dropped",
                    l.pieces.len()
                );
            }
            continue;
        }
        let mut known: Vec<(&Piece, Curve)> = Vec::new();
        // Records of curves the sketch no longer has (the profile is older
        // than the sketch's last edit); see `closes` below.
        let mut ghosts = false;
        let from = listed.len();
        for r in &l.pieces {
            let Some(id) = &r.id else {
                if crate::tracing() {
                    eprintln!(
                        "import: profile record {:?} has no curve id in the sketch",
                        r.key
                    );
                }
                ghosts = true;
                if crate::tracing() {
                    eprintln!("import:   a profile loop piece without a curve id");
                }
                continue;
            };
            let Some(g) = curves.get(id) else {
                if crate::tracing() {
                    eprintln!(
                        "import: profile record {:?} names curve {id}, not in the file's sketch",
                        r.key
                    );
                }
                ghosts = true;
                if crate::tracing() {
                    eprintln!(
                        "import:   a profile loop names curve {id}, which the sketch does not have"
                    );
                }
                continue;
            };
            let Some(curve) = Curve::of(g) else {
                if crate::tracing() {
                    eprintln!(
                        "import: profile curve {id} is a {:?}, not a line, arc or circle",
                        g.geometry_type
                    );
                }
                return None;
            };
            known.push((r, curve));
        }
        for (r, c) in &known {
            let others: Vec<&Curve> = known
                .iter()
                .filter(|(o, _)| o.id != r.id)
                .map(|(_, c)| c)
                .collect();
            let mut all = pieces(c, &others);
            if all.len() != r.pieces {
                let mut matched = "no cuts";
                let own = r.id.as_deref();
                let by_profile = profile_cutters.get_or_insert_with(|| {
                    let mut seen = HashSet::new();
                    p.loops
                        .iter()
                        .chain(p.children.iter().flatten())
                        .flat_map(|l| l.pieces.iter())
                        .filter_map(|r| r.id.as_deref())
                        .filter(|id| seen.insert(*id))
                        .filter_map(|id| Some((id.to_owned(), Curve::of(curves.get(id)?)?)))
                        .collect()
                });
                let set: Vec<&Curve> = by_profile
                    .iter()
                    .filter(|(id, _)| Some(id.as_str()) != own)
                    .map(|(_, c)| c)
                    .collect();
                let alt = pieces(c, &set);
                if alt.len() == r.pieces {
                    all = alt;
                    matched = "the profile's curves";
                } else {
                    let by_sketch = sketch_cutters.get_or_insert_with(|| {
                        curves
                            .iter()
                            .filter(|(id, _)| !construction.contains(*id))
                            .filter_map(|(id, g)| Some((id.clone(), Curve::of(g)?)))
                            .collect()
                    });
                    let set: Vec<&Curve> = by_sketch
                        .iter()
                        .filter(|(id, _)| Some(id.as_str()) != own)
                        .map(|(_, c)| c)
                        .collect();
                    let alt = pieces(c, &set);
                    if alt.len() == r.pieces {
                        all = alt;
                        matched = "the sketch's curves";
                    }
                }
                if crate::tracing() {
                    eprintln!(
                        "import: profile record {:?} of curve {:?}: the file counts {} pieces, the loop's cuts give {}; {}",
                        r.key,
                        r.id,
                        r.pieces,
                        pieces(c, &others).len(),
                        matched
                    );
                }
                // Cuts found numerically that disagree with the file's own count
                // number the pieces unreliably: the loops are left to the
                // curve-overlap rule.
                if matched == "no cuts" && (c.is_numeric() || others.iter().any(|o| o.is_numeric()))
                {
                    return None;
                }
            }
            let (t0, t1) = match all.get(r.piece.wrapping_sub(1)) {
                Some(&range) => range,
                None => {
                    if crate::tracing() {
                        eprintln!(
                            "import: profile record {:?} names piece {} of curve {:?}, which has {}",
                            r.key,
                            r.piece,
                            r.id,
                            all.len()
                        );
                    }
                    complete = false;
                    if crate::tracing() {
                        eprintln!(
                            "import:   a profile loop names piece {} of curve {:?}, which has {}",
                            r.piece,
                            r.id,
                            all.len()
                        );
                    }
                    (0.0, 1.0)
                }
            };
            listed.push((c.clone(), t0, t1));
        }
        // A record of a curve the sketch no longer has is left out where
        // the loop's other pieces close up without it: the loop's inside
        // is what they enclose, and the file's own reading of the profile
        // lists no curve of it (the original application's dumps of such
        // profiles list the closed loop's curves only).
        if ghosts && !closes(&listed[from..]) {
            complete = false;
        }
    }
    if listed.is_empty() {
        return Some((Vec::new(), false));
    }
    Some((
        regions
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.inside
                    .is_some_and(|q| inside(&listed, [q[0] / 10.0, q[1] / 10.0]))
            })
            .map(|(i, _)| i)
            .collect(),
        complete,
    ))
}

/// The regions whose outer loops share most curves with the profile's
/// outer loops (its children's, where it has any).
fn by_curves(p: &Profile, regions: &[RegionView]) -> Vec<usize> {
    let key = |r: &Piece| match &r.id {
        Some(id) => id.clone(),
        None => format!("{}:{}", r.key.0, r.key.1),
    };
    let sets: Vec<Option<HashSet<&str>>> = regions
        .iter()
        .map(|r| {
            r.outer
                .as_ref()
                .map(|o| o.iter().map(String::as_str).collect())
        })
        .collect();
    let groups: Vec<&Vec<Loop>> = if p.children.is_empty() {
        vec![&p.loops]
    } else {
        p.children.iter().collect()
    };
    let mut out = BTreeSet::new();
    for l in groups.into_iter().flatten().filter(|l| l.outer) {
        let listed: HashSet<String> = l.pieces.iter().map(key).collect();
        let mut best = 0.0;
        let mut chosen: Vec<usize> = Vec::new();
        for (i, set) in sets.iter().enumerate() {
            let Some(set) = set else { continue };
            let common = set.iter().filter(|c| listed.contains(**c)).count();
            let union = listed.len() + set.len() - common;
            let j = if union == 0 {
                0.0
            } else {
                common as f64 / union as f64
            };
            if j > best + 1e-9 {
                best = j;
                chosen = vec![i];
            } else if (j - best).abs() <= 1e-9 && j > 0.0 {
                chosen.push(i);
            }
        }
        if best >= 0.5 {
            out.insert(chosen[0]);
            // Tied regions of other curves too; of the same curves (the
            // halves of a disc cut by its diameter) the first.
            let distinct: HashSet<Vec<&str>> = chosen
                .iter()
                .filter_map(|&i| {
                    let mut v: Vec<&str> = sets[i].as_ref()?.iter().copied().collect();
                    v.sort_unstable();
                    Some(v)
                })
                .collect();
            if distinct.len() > 1 {
                out.extend(chosen);
            }
        } else if regions.len() == 1 {
            out.insert(0);
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn line(s: [f64; 2], e: [f64; 2]) -> Geometry {
        Geometry {
            geometry_type: Some("Line3D".into()),
            start_point: Some([s[0], s[1], 0.0]),
            end_point: Some([e[0], e[1], 0.0]),
            ..Geometry::default()
        }
    }

    fn circle(c: [f64; 2], r: f64) -> Geometry {
        Geometry {
            geometry_type: Some("Circle3D".into()),
            center: Some([c[0], c[1], 0.0]),
            radius: Some(r),
            normal: Some([0.0, 0.0, 1.0]),
            ..Geometry::default()
        }
    }

    fn rec_n(id: &str, piece: u64, pieces: u64) -> Value {
        json!({"id": id, "primary": 1, "secondary": 0, "piece": piece, "pieces": pieces})
    }

    fn rec(id: &str, piece: u64) -> Value {
        json!({"id": id, "primary": 1, "secondary": 0, "piece": piece, "pieces": 1})
    }

    /// A square of 4 cm with a circle of radius 1 cm in its middle and a
    /// diagonal line through the circle: regions are the square's two
    /// triangles less the circle, and the circle's two halves.
    fn sketch() -> (HashMap<String, Geometry>, Vec<RegionView>) {
        let curves = HashMap::from([
            ("c0".to_owned(), line([0.0, 0.0], [4.0, 0.0])),
            ("c1".to_owned(), line([4.0, 0.0], [4.0, 4.0])),
            ("c2".to_owned(), line([4.0, 4.0], [0.0, 4.0])),
            ("c3".to_owned(), line([0.0, 4.0], [0.0, 0.0])),
            ("c4".to_owned(), circle([2.0, 2.0], 1.0)),
            ("c5".to_owned(), line([0.0, 0.0], [4.0, 4.0])),
        ]);
        let region = |x: f64, y: f64, outer: &[&str]| RegionView {
            inside: Some([x * 10.0, y * 10.0]),
            outer: Some(outer.iter().map(|s| s.to_string()).collect()),
        };
        let regions = vec![
            region(3.5, 0.5, &["c0", "c1", "c5", "c4"]),
            region(0.5, 3.5, &["c2", "c3", "c5", "c4"]),
            region(2.5, 1.8, &["c4", "c5"]),
            region(1.5, 2.2, &["c4", "c5"]),
        ];
        (curves, regions)
    }

    #[test]
    fn pieces_follow_the_curve() {
        let c = Curve::of(&circle([0.0, 0.0], 1.0)).unwrap();
        let l = Curve::of(&line([-2.0, 0.5], [2.0, 0.5])).unwrap();
        let p = pieces(&c, &[&l]);
        // Cut at 30° and 150°: the piece holding 0° (from 150° round) first.
        assert_eq!(p.len(), 2);
        assert!((p[0].0 - 5.0 / 12.0).abs() < 1e-9 && (p[0].1 - 13.0 / 12.0).abs() < 1e-9);
        assert!((p[1].0 - 1.0 / 12.0).abs() < 1e-9 && (p[1].1 - 5.0 / 12.0).abs() < 1e-9);
        let q = pieces(&l, &[&c]);
        assert_eq!(q.len(), 3);
        assert!((q[1].0 - (2.0 - 3f64.sqrt() / 2.0) / 4.0).abs() < 1e-9);
    }

    #[test]
    fn a_face_selects_the_regions_inside_it() {
        let (curves, regions) = sketch();
        // The square less the circle: the two triangles' material, cut by
        // nothing inside the square (the diagonal is not in the loop).
        let square = json!([{"loops": [
            {"outer": true, "curves": [rec("c0", 1), rec("c1", 1), rec("c2", 1), rec("c3", 1)]},
            {"outer": false, "curves": [rec("c4", 1)]}], "children": []}]);
        assert_eq!(mapped_regions(&square, &curves, &regions), Some(vec![0, 1]));
        // The circle's lower right half: piece 1 of the circle cut by the
        // diagonal (from 225° round to 45°) and the diagonal's middle piece.
        let half = json!([{"loops": [
            {"outer": true, "curves": [rec("c4", 1), rec("c5", 2)]}], "children": []}]);
        assert_eq!(mapped_regions(&half, &curves, &regions), Some(vec![2]));
        // The whole circle and the square without its hole: everything.
        let all = json!([{"loops": [{"outer": true, "curves": [rec("c4", 1)]}], "children": []},
            {"loops": [{"outer": true,
                        "curves": [rec("c0", 1), rec("c1", 1), rec("c2", 1), rec("c3", 1)]}],
             "children": []}]);
        assert_eq!(
            mapped_regions(&all, &curves, &regions),
            Some(vec![0, 1, 2, 3])
        );
        // Every record found in the sketch: exact (mitcad#186).
        for loops in [&square, &half, &all] {
            assert!(mapped(loops, &curves, &regions).unwrap().1);
        }
        // A record naming a curve the sketch no longer has is left out
        // where the loop's other curves close up without it (the square):
        // exact. Where they do not (the square's three sides), or where a
        // record names a piece the curve does not have, the selection is
        // inexact.
        let stale = json!([{"loops": [
            {"outer": true, "curves": [rec("c0", 1), rec("c1", 1), rec("c2", 1), rec("c3", 1),
                                       rec("c9", 1)]},
            {"outer": false, "curves": [rec("c4", 1)]}], "children": []}]);
        let (found, exact) = mapped(&stale, &curves, &regions).unwrap();
        assert_eq!(found, vec![0, 1]);
        assert!(exact);
        let open = json!([{"loops": [
            {"outer": true, "curves": [rec("c0", 1), rec("c1", 1), rec("c2", 1), rec("c9", 1)]},
            {"outer": false, "curves": [rec("c4", 1)]}], "children": []}]);
        assert!(!mapped(&open, &curves, &regions).unwrap().1);
        let beyond = json!([{"loops": [
            {"outer": true, "curves": [rec("c4", 1), rec("c5", 7)]}], "children": []}]);
        assert!(!mapped(&beyond, &curves, &regions).unwrap().1);
        // A hole whose curves are all gone is dropped: the square's
        // regions, the circle's too, are inside the profile, exactly. A
        // hole of a curve the sketch has and one it does not is the
        // circle, which closes alone; an outer loop that lost a curve of
        // its square does not close.
        let no_curve = json!({"primary": 9, "secondary": 0, "piece": 1, "pieces": 1});
        let outer = json!({"outer": true,
            "curves": [rec("c0", 1), rec("c1", 1), rec("c2", 1), rec("c3", 1)]});
        let gone = json!([{"loops": [outer.clone(),
            {"outer": false, "curves": [no_curve.clone()]}], "children": []}]);
        let (found, exact) = mapped(&gone, &curves, &regions).unwrap();
        assert_eq!(found, vec![0, 1, 2, 3]);
        assert!(exact);
        let partly = json!([{"loops": [outer.clone(),
            {"outer": false, "curves": [rec("c4", 1), no_curve.clone()]}], "children": []}]);
        let (found, exact) = mapped(&partly, &curves, &regions).unwrap();
        assert_eq!(found, vec![0, 1]);
        assert!(exact);
        let lost = json!([{"loops": [
            {"outer": true, "curves": [rec("c0", 1), rec("c1", 1), rec("c2", 1), no_curve]}],
            "children": []}]);
        assert!(mapped(&lost, &curves, &regions).is_none_or(|(_, exact)| !exact));
    }

    #[test]
    fn a_profile_whose_curves_are_all_gone_counts_them() {
        let gone =
            |primary: u64| json!({"primary": primary, "secondary": 0, "piece": 1, "pieces": 1});
        let one = json!([{"loops": [{"outer": true, "curves": [gone(136)]}], "children": []}]);
        assert_eq!(gone_curves(&one), Some(1));
        let two = json!([{"loops": [{"outer": true, "curves": [gone(7), gone(8), gone(7)]}],
                          "children": []}]);
        assert_eq!(gone_curves(&two), Some(2));
        // A record of a curve the sketch has: not gone.
        let kept = json!([{"loops": [{"outer": true, "curves": [
            {"id": "c0", "primary": 1, "secondary": 0, "piece": 1, "pieces": 1}, gone(9)]}],
            "children": []}]);
        assert_eq!(gone_curves(&kept), None);
        assert_eq!(gone_curves(&json!([])), None);
    }

    #[test]
    fn a_sketch_of_one_region_selects_it_whatever_the_records_say() {
        // A profile older than the sketch's last edit: its records name
        // curves the sketch no longer has. The one region is the choice.
        let region = |outer: &[&str]| RegionView {
            inside: Some([0.0, 0.0]),
            outer: Some(outer.iter().map(|s| s.to_string()).collect()),
        };
        let curves = HashMap::from([("c0".to_owned(), circle([0.0, 0.0], 1.0))]);
        let gone =
            |primary: u64| json!({"primary": primary, "secondary": 0, "piece": 1, "pieces": 1});
        let stale = json!([{"loops": [{"outer": true, "curves": [gone(7), gone(8)]}],
                            "children": []}]);
        assert_eq!(
            mapped(&stale, &curves, &[region(&["c0"])]),
            Some((vec![0], true))
        );
        // Two regions: nothing says which; not exact.
        let two = [region(&["c0"]), region(&["c0"])];
        assert!(mapped(&stale, &curves, &two).is_none_or(|(_, exact)| !exact));
    }

    #[test]
    fn without_geometry_the_curves_decide() {
        let (mut curves, regions) = sketch();
        // A spline stands in for the circle: the rule by geometry does not
        // apply, the outer loop's curves select the triangle.
        curves.insert(
            "c4".into(),
            Geometry {
                geometry_type: Some("NurbsCurve3D".into()),
                ..Geometry::default()
            },
        );
        let triangle = json!([{"loops": [
            {"outer": true, "curves": [rec("c0", 1), rec("c1", 1), rec("c5", 1), rec("c4", 1)]}],
            "children": []}]);
        assert_eq!(mapped_regions(&triangle, &curves, &regions), Some(vec![0]));
        // By the curves: not exact.
        assert!(!mapped(&triangle, &curves, &regions).unwrap().1);
        // Children: each piece selects its own region.
        let split = json!([{"loops": [{"outer": true, "curves": [rec("c4", 1)]}], "children": [
            [{"outer": true, "curves": [rec("c4", 1), rec("c5", 1)]}]]}]);
        assert_eq!(mapped_regions(&split, &curves, &regions), Some(vec![2]));
        // Nothing that reads, or selects nothing: none.
        assert_eq!(mapped_regions(&json!({}), &curves, &regions), None);
        let stale = json!([{"loops": [{"outer": true, "curves": [
            {"primary": 9, "secondary": 0, "piece": 1, "pieces": 1}]}], "children": []}]);
        assert_eq!(mapped_regions(&stale, &curves, &regions), None);
    }

    fn ellipse(c: [f64; 2], major: [f64; 2], a: f64, b: f64) -> Geometry {
        Geometry {
            geometry_type: Some("Ellipse3D".into()),
            center: Some([c[0], c[1], 0.0]),
            normal: Some([0.0, 0.0, 1.0]),
            major_axis: Some([major[0], major[1], 0.0]),
            major_radius: Some(a),
            minor_radius: Some(b),
            ..Geometry::default()
        }
    }

    /// The rational quadratic quarter of the unit circle from (1, 0) to
    /// (0, 1), as a sketch spline stores it.
    fn quarter_spline() -> Geometry {
        Geometry {
            geometry_type: Some("NurbsCurve3D".into()),
            nurbs: Some(json!({
                "degree": 2, "isRational": true, "isPeriodic": false,
                "controlPointCount": 3,
                "knots": [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                "controlPoints": [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
                "weights": [1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0],
            })),
            ..Geometry::default()
        }
    }

    #[test]
    fn ellipses_are_cut_and_numbered_like_circles() {
        let e = Curve::of(&ellipse([0.0, 0.0], [1.0, 0.0], 2.0, 1.0)).unwrap();
        let l = Curve::of(&line([-3.0, 0.5], [3.0, 0.5])).unwrap();
        // The line meets the ellipse at 30° and 150° of its parameter: the
        // piece holding the start (150° round to 30°) comes first.
        let p = pieces(&e, &[&l]);
        assert_eq!(p.len(), 2);
        assert!((p[0].0 - 5.0 / 12.0).abs() < 1e-9 && (p[0].1 - 13.0 / 12.0).abs() < 1e-9);
        assert!((p[1].0 - 1.0 / 12.0).abs() < 1e-9 && (p[1].1 - 5.0 / 12.0).abs() < 1e-9);
        // The line is cut at x = ±√3 of the 6 cm.
        let q = pieces(&l, &[&e]);
        assert_eq!(q.len(), 3);
        assert!((q[1].0 - (3.0 - 3f64.sqrt()) / 6.0).abs() < 1e-9);
        // A rotated ellipse: its parameter starts at the major axis end.
        let r = Curve::of(&ellipse([1.0, 1.0], [0.0, 1.0], 2.0, 1.0)).unwrap();
        let s = r.point(0.0);
        assert!((s[0] - 1.0).abs() < 1e-9 && (s[1] - 3.0).abs() < 1e-9);
        let t = r.point(0.25);
        assert!((t[0] - 0.0).abs() < 1e-9 && (t[1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_spline_is_cut_where_the_curves_cross_it() {
        let s = Curve::of(&quarter_spline()).unwrap();
        for k in 0..=10 {
            let p = s.point(k as f64 / 10.0);
            assert!((p[0].hypot(p[1]) - 1.0).abs() < 1e-12);
        }
        let d = Curve::of(&line([0.0, 0.0], [2.0, 2.0])).unwrap();
        let p = pieces(&s, &[&d]);
        assert_eq!(p.len(), 2);
        assert!((p[0].1 - 0.5).abs() < 1e-9 && (p[1].0 - 0.5).abs() < 1e-9);
        // A circle that the spline lies along is cut at the ends of the
        // stretch only.
        let c = Curve::of(&circle([0.0, 0.0], 1.0)).unwrap();
        let p = pieces(&c, &[&s]);
        assert_eq!(p.len(), 2);
        assert!((p[0].1 - 0.25).abs() < 1e-6);
        // A line ending on the spline cuts it there.
        let t = Curve::of(&line([0.0, 0.0], [0.6, 0.8])).unwrap();
        let p = pieces(&s, &[&t]);
        assert_eq!(p.len(), 2);
        let want = s.param([0.6, 0.8]);
        assert!((p[0].1 - want).abs() < 1e-9);
    }

    /// A quarter disc of the unit circle (a spline for the arc) cut in two
    /// by a diagonal, and a quarter ellipse beside it.
    #[test]
    fn profiles_of_ellipses_and_splines_select_exactly() {
        let curves = HashMap::from([
            ("c0".to_owned(), line([0.0, 0.0], [1.0, 0.0])),
            ("c1".to_owned(), quarter_spline()),
            ("c2".to_owned(), line([0.0, 1.0], [0.0, 0.0])),
            ("c3".to_owned(), line([0.0, 0.0], [1.0, 1.0])),
            ("c4".to_owned(), ellipse([5.0, 0.0], [1.0, 0.0], 2.0, 1.0)),
            ("c5".to_owned(), line([3.0, 0.0], [7.0, 0.0])),
        ]);
        let region = |x: f64, y: f64| RegionView {
            inside: Some([x * 10.0, y * 10.0]),
            outer: None,
        };
        let regions = vec![
            region(0.6, 0.2),
            region(0.2, 0.6),
            region(5.0, 0.5),
            region(5.0, -0.5),
        ];
        // The lower half of the quarter disc: the first piece of the spline
        // and of the diagonal, and the line along the x axis.
        let lower = json!([{"loops": [{"outer": true, "curves": [
            rec_n("c0", 1, 1), rec_n("c1", 1, 2), rec_n("c3", 1, 2)]}], "children": []}]);
        // c1 is cut by c3 once (2 pieces): the loop lists piece 1 of the
        // spline and, the diagonal being cut by the spline's end, 1 of 1.
        let (found, exact) = mapped(&lower, &curves, &regions).unwrap();
        assert_eq!(found, vec![0]);
        assert!(exact);
        // The ellipse cut by the line along its major axis: the upper half
        // is the ellipse's first piece, from its start (0°) to 180°.
        let upper = json!([{"loops": [{"outer": true, "curves": [
            rec_n("c4", 1, 2), rec_n("c5", 1, 1)]}], "children": []}]);
        let (found, exact) = mapped(&upper, &curves, &regions).unwrap();
        assert_eq!(found, vec![2]);
        assert!(exact);
        // Pieces numbered by numeric cuts that the file's count contradicts
        // are not trusted: no exact selection.
        let miscounted = json!([{"loops": [{"outer": true, "curves": [
            rec_n("c0", 1, 1), rec_n("c1", 1, 7), rec_n("c3", 1, 2)]}], "children": []}]);
        assert!(mapped(&miscounted, &curves, &regions).is_none_or(|(_, exact)| !exact));
        // A piece the curve does not have is not exact.
        let beyond = json!([{"loops": [{"outer": true, "curves": [
            rec_n("c4", 5, 2), rec_n("c5", 1, 1)]}], "children": []}]);
        assert!(!mapped(&beyond, &curves, &regions).unwrap().1);
    }

    #[test]
    fn a_curve_cut_by_curves_outside_the_loop_is_numbered_by_the_file_count() {
        let (curves, regions) = sketch();
        let rec_of = |id: &str, piece: u64, pieces: u64| json!({"id": id, "primary": 1, "secondary": 0, "piece": piece, "pieces": pieces});
        // The circle is cut in two by the diagonal c5, which the loop does
        // not list: the file's count of two pieces says the cuts of the
        // sketch's other curves number the circle.
        let by_sketch = json!([{"loops": [{"outer": true, "curves": [rec_of("c4", 2, 2)]}],
                                "children": []}]);
        assert!(mapped(&by_sketch, &curves, &regions).unwrap().1);
        // A count that no cuts of the sketch give: the piece is not found.
        let none = json!([{"loops": [{"outer": true, "curves": [rec_of("c4", 2, 5)]}],
                           "children": []}]);
        assert!(!mapped(&none, &curves, &regions).unwrap().1);
    }
}
