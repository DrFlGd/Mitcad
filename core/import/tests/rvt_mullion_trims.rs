// SPDX-License-Identifier: MIT
//! Trimmed curtain mullions (`MullionTrimNeighbor`, mitcad#387) on the
//! corpus's 2024 projects (by content id under `MITCAD_RVT_CORPUS`; skipped
//! without it): the trims' rule (`plan::decoded_mullion_ends`) cuts every
//! trimmed end of every mullion symbol where the symbol's stored body ends,
//! within 1e-9 ft along its sides, but the ends listed in `NOT_BY_RULE`.

use std::path::PathBuf;

use mitcad_import::rvt::plan::{decoded_mullion_ends, ends_apart, stored_mullion_ends};
use mitcad_rvt::RvtFile;
use mitcad_rvt::building::{Curve, P3};
use mitcad_rvt::elements::Model;
use mitcad_vec3::{cross, dot, norm, scale, sub};

/// The projects with trimmed mullions and how many trimmed ends each has.
const FILES: [(&str, usize); 3] = [("33271010", 20), ("ff791052", 89), ("bb62c161", 21)];

/// Trimmed ends (file, symbol, end) the rule does not give: three butts of
/// `33271010` whose stored ends are not cut (two mullions nearly square to
/// a sloped border running on past them, and one border mullion at a corner
/// mullion, the mirror image of `1448308`'s end 0, which is cut).
const NOT_BY_RULE: [(&str, i64, u8); 3] = [
    ("33271010", 1443552, 0),
    ("33271010", 1443556, 0),
    ("33271010", 1448325, 1),
];

fn corpus_file(id: &str) -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("MITCAD_RVT_CORPUS")?);
    let index = std::fs::read_to_string(dir.join("index.tsv")).ok()?;
    let path = index.lines().find_map(|line| {
        let mut cols = line.split('\t');
        let path = cols.next()?;
        cols.next()?.starts_with(id).then(|| path.to_string())
    })?;
    let p = dir.join(path);
    p.is_file().then_some(p)
}

/// The unit vector along `a`, with no cut-off (the ends' sides are not zero).
fn unit(a: P3) -> P3 {
    scale(a, 1.0 / norm(a))
}

#[test]
fn the_trims_rule_cuts_the_mullions_where_their_stored_bodies_end() {
    for (id, ends) in FILES {
        let Some(path) = corpus_file(id) else {
            eprintln!("MITCAD_RVT_CORPUS has no {id}: skipped");
            continue;
        };
        let file = RvtFile::open(&path).unwrap();
        let schema = file.schema().unwrap();
        let model = Model::read(&file, &schema).unwrap();
        let b = &model.building;
        let (mut same, mut listed, mut seen) = (0, 0, 0);
        for (sid, s) in &b.symbols {
            let Some(m) = &s.mullion else { continue };
            if m.trims.is_empty() {
                continue;
            }
            let Some(Curve::Line { start, end }) = m.path else {
                continue;
            };
            let length = dot(sub(end, start), sub(end, start)).sqrt();
            let d = unit(sub(end, start));
            let a = unit(cross(unit(m.normal), d));
            // The mullion's sides.
            let sides = if m.flags & 4 != 0 {
                [0.0, m.widths[0] + m.widths[1]]
            } else {
                [-m.widths[1], m.widths[0]]
            };
            let stored = stored_mullion_ends(&model, *sid, start, a, d, length);
            let decoded = decoded_mullion_ends(b, m, start, d, length);
            for t in &m.trims {
                seen += 1;
                let k = usize::from(t.end == 1);
                let off = match (&stored, &decoded[k]) {
                    (Ok(planes), Some(Ok(cut))) => {
                        ends_apart(cut, &planes[k], start, a, sides, d, k, length)
                    }
                    _ => f64::NAN,
                };
                let known = NOT_BY_RULE.contains(&(id, *sid, t.end));
                if off <= 1e-9 {
                    assert!(
                        !known,
                        "{id}: symbol {sid} end {k} is listed but by the rule"
                    );
                    same += 1;
                } else {
                    eprintln!(
                        "{id}: symbol {sid} end {k} (flags {}): the rule is {off:e} ft off \
                         its stored end ({:?})",
                        t.flags, decoded[k]
                    );
                    assert!(known, "{id}: symbol {sid} end {k}");
                    listed += 1;
                }
            }
        }
        eprintln!("{id}: {same} / {seen} trimmed ends by the rule, {listed} listed");
        assert_eq!(seen, ends, "{id}");
    }
}
