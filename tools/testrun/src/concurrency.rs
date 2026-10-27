// SPDX-License-Identifier: MIT
//! How many UI entries run at once without disturbing each other
//! (mitcad#790 roadmap step 5), learnt from the ordinary runs, never from a
//! measuring run of its own.
//!
//! A UI entry is a test of the UI kind (`profile::Kind::Ui`: `app.ui-*`,
//! labelled `ui`), or a UI test that `tools/check-all.sh` runs itself and
//! hands over with `--record-ui`. Each of its runs records how many other UI
//! entries ran beside it at most (`Run::beside`). A UI entry that failed
//! (or timed out) with others beside it and then passed with fewer beside
//! it (the runner reruns it alone) failed for the company, not for the
//! code: a **contention failure**. From those:
//!
//! - **the machine's cap** (per platform, in the profile file): UI entries
//!   at once. It starts at the bound (`--ui-max`, else
//!   `MITCAD_UI_JOBS`; 4 for learning without either), drops by one after
//!   a run with a contention failure, and grows by one after `GROW_RUNS`
//!   clean runs that reached it, never above the bound;
//! - **a test's room** (in its profile): the other UI entries it may have
//!   beside it. A contention failure beside `b` others halves it to `b / 2`;
//!   `ROOM_GROW_PASSES` passes with a full room beside it grow it by one.
//!
//! The scheduler (`schedule::admit`) starts a UI entry only while the UI
//! entries running are fewer than the cap and neither it nor any running UI
//! entry would have more beside it than its room (`admits`). A machine opts
//! in by giving the bound: without `--ui-max` and `MITCAD_UI_JOBS` the runs
//! record and learn, but the scheduler holds UI entries to neither the cap
//! nor the rooms and reruns none alone (`scheduled_cap`, `scheduled_room`).

use std::collections::BTreeMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::options::Options;
use crate::profile::{self, ResultKind, Run};

/// Clean runs at the cap after which the cap grows by one.
pub const GROW_RUNS: u32 = 2;
/// Passes with its room full after which a test's room grows by one.
pub const ROOM_GROW_PASSES: u32 = 3;
/// The bound the cap is learnt within when neither `--ui-max` nor
/// `MITCAD_UI_JOBS` gives one (check-all's default); it is not applied.
pub const DEFAULT_BOUND: usize = 4;
/// What a contention failure takes back of the failure's +10: it counts
/// +2, as the other failures of the environment.
pub const CONTENTION_REFUND: f64 = 8.0;
/// The platform the cap is kept under in the profile file.
pub const PLATFORM: &str = std::env::consts::OS;

/// What a machine has learnt of its UI entries at once, one per platform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCap {
    /// UI entries at once.
    pub cap: usize,
    /// Clean runs that reached the cap since it last changed.
    #[serde(default)]
    pub clean_runs: u32,
}

/// The bound of the cap: `--ui-max`, else `MITCAD_UI_JOBS` (a number of at
/// least 1); None without either, and then the scheduler applies no cap.
pub fn bound(given: Option<usize>, variable: Option<&str>) -> Option<usize> {
    let from_variable = variable.and_then(|v| v.trim().parse::<usize>().ok());
    given.or(from_variable).filter(|&n| n > 0)
}

/// The bound the cap is learnt within: the given one, else
/// `DEFAULT_BOUND`.
pub fn learning_bound(bound: Option<usize>) -> usize {
    bound.unwrap_or(DEFAULT_BOUND)
}

/// The UI entries at once the scheduler holds to: the learnt cap within
/// the bound, or no limit (`usize::MAX`) without a bound.
pub fn scheduled_cap(learnt: Option<&UiCap>, bound: Option<usize>) -> usize {
    bound.map_or(usize::MAX, |b| effective_cap(learnt, b))
}

/// The room the scheduler holds a test to: its learnt room, or none
/// without a bound.
pub fn scheduled_room(room: Option<u32>, bound: Option<usize>) -> Option<usize> {
    bound.and(room).map(|r| r as usize)
}

/// The cap a machine starts with: the bound itself, at least 1, so that
/// taking the cap into use does not slow a machine down; it drops after a
/// run with a failure that was only contention.
pub fn start_cap(bound: usize) -> usize {
    bound.max(1)
}

/// The cap a run goes by: the learnt one (the start without one), within
/// the bound.
pub fn effective_cap(learnt: Option<&UiCap>, bound: usize) -> usize {
    let bound = bound.max(1);
    learnt.map_or(start_cap(bound), |l| l.cap).clamp(1, bound)
}

/// Whether a UI entry may start: the UI entries running (`running`) are
/// fewer than the cap, and neither it (`own_room`) nor any of them (the
/// smallest room among them, `running_room`) would have more UI entries
/// beside it than its room.
pub fn admits(
    running: usize,
    running_room: Option<usize>,
    own_room: Option<usize>,
    cap: usize,
) -> bool {
    let fits = |room: Option<usize>| room.is_none_or(|r| running <= r);
    running < cap && fits(own_room) && fits(running_room)
}

fn failed(result: ResultKind) -> bool {
    matches!(result, ResultKind::Failed | ResultKind::Timeout)
}

/// A contention failure: `previous` failed (or timed out) with `b` > 0
/// other UI entries beside it and `run` passed with fewer beside it. The
/// `b`, or None.
pub fn contention(previous: &Run, run: &Run) -> Option<u32> {
    let beside = previous.beside.filter(|&b| b > 0)?;
    let fewer = run.beside.is_some_and(|n| n < beside);
    let passed = run.result == ResultKind::Passed;
    (failed(previous.result) && passed && fewer).then_some(beside)
}

/// A test's room and its passes with the room full, after `run` (its run
/// before: `previous`): a contention failure beside `b` others halves the
/// room to `b / 2` (0: no other UI entry beside it); a pass with at least
/// the room beside it counts, and `ROOM_GROW_PASSES` of them grow the room
/// by one.
pub fn update_room(
    room: Option<u32>,
    passes: u32,
    previous: Option<&Run>,
    run: &Run,
) -> (Option<u32>, u32) {
    if let Some(beside) = previous.and_then(|p| contention(p, run)) {
        let halved = beside / 2;
        return (Some(room.map_or(halved, |r| r.min(halved))), 0);
    }
    match (room, run.beside) {
        (Some(r), Some(beside)) if run.result == ResultKind::Passed && beside >= r => {
            if passes + 1 >= ROOM_GROW_PASSES {
                (Some(r + 1), 0)
            } else {
                (Some(r), passes + 1)
            }
        }
        _ => (room, passes),
    }
}

/// What one run tells of its UI entries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunFacts {
    /// The most UI entries at once.
    pub at_once: usize,
    /// UI entries that failed beside others and then passed with fewer.
    pub contended: usize,
    /// UI entries that failed beside others and did not pass with fewer
    /// afterwards (a real failure, or one that fails alone too).
    pub unresolved: usize,
}

/// The facts of a run from its records in the order they ended (a test's
/// rerun after its first attempt); the runs without `beside` are not UI
/// entries.
pub fn run_facts(records: &[(String, Run)]) -> RunFacts {
    let mut by_test: BTreeMap<&str, Vec<&Run>> = BTreeMap::new();
    for (name, run) in records {
        if run.beside.is_some() {
            by_test.entry(name.as_str()).or_default().push(run);
        }
    }
    let mut facts = RunFacts::default();
    for runs in by_test.values() {
        for (k, run) in runs.iter().enumerate() {
            let beside = run.beside.unwrap_or(0) as usize;
            facts.at_once = facts.at_once.max(beside + 1);
            if !failed(run.result) || beside == 0 {
                continue;
            }
            let next = runs.get(k + 1);
            if next.is_some_and(|n| contention(run, n).is_some()) {
                facts.contended += 1;
            } else {
                facts.unresolved += 1;
            }
        }
    }
    facts
}

/// The machine's cap after a run: one less (at least 1) after a contention
/// failure; one more after `GROW_RUNS` clean runs (no UI entry failed
/// beside others) that reached the cap, up to the bound; otherwise as it
/// was. A run without UI entries leaves it.
pub fn learn(learnt: Option<&UiCap>, facts: &RunFacts, bound: usize) -> Option<UiCap> {
    if facts.at_once == 0 {
        return learnt.cloned();
    }
    let bound = bound.max(1);
    let cap = effective_cap(learnt, bound);
    let clean = learnt.map_or(0, |l| l.clean_runs);
    if facts.contended > 0 {
        let cap = cap.saturating_sub(1).max(1);
        return Some(UiCap { cap, clean_runs: 0 });
    }
    if facts.unresolved > 0 || facts.at_once < cap {
        return Some(UiCap {
            cap,
            clean_runs: clean,
        });
    }
    let clean_runs = clean + 1;
    if clean_runs >= GROW_RUNS && cap < bound {
        return Some(UiCap {
            cap: cap + 1,
            clean_runs: 0,
        });
    }
    Some(UiCap {
        cap,
        clean_runs: clean_runs.min(GROW_RUNS),
    })
}

/// Learns from a run into the profile file's caps; the cap before and
/// after.
pub fn learn_into(
    caps: &mut BTreeMap<String, UiCap>,
    platform: &str,
    facts: &RunFacts,
    bound: usize,
) -> (usize, usize) {
    let before = effective_cap(caps.get(platform), bound);
    if let Some(next) = learn(caps.get(platform), facts, bound) {
        caps.insert(platform.to_owned(), next);
    }
    (before, effective_cap(caps.get(platform), bound))
}

/// One line on what a run taught.
pub fn summary(facts: &RunFacts, before: usize, after: usize) -> String {
    let change = if after == before {
        format!("cap {after}")
    } else {
        format!("cap {before} -> {after}")
    };
    format!(
        "UI entries at once: {change}; this run at most {}, {} failed beside others and passed alone, {} failed beside others",
        facts.at_once, facts.contended, facts.unresolved
    )
}

/// The UI runs `tools/check-all.sh` hands over (`--record-ui FILE`): a line
/// per run in the order they ended, `name<TAB>exit status<TAB>seconds<TAB>
/// other UI entries beside it at most`; 0 passed, 124 and 137 (timeout's)
/// timed out, 77 skipped, others failed. Empty lines and lines starting
/// with `#` are left out.
pub fn parse_ui_runs(text: &str, when: u64) -> Result<Vec<(String, Run)>, String> {
    let mut runs = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [name, status, seconds, beside] = fields[..] else {
            return Err(format!(
                "not a UI run (name, status, seconds, beside): {line}"
            ));
        };
        let bad = || format!("not a UI run: {line}");
        let status: i32 = status.trim().parse().map_err(|_| bad())?;
        let wall: f64 = seconds.trim().parse().map_err(|_| bad())?;
        let beside: u32 = beside.trim().parse().map_err(|_| bad())?;
        let result = match status {
            0 => ResultKind::Passed,
            77 => ResultKind::Skipped,
            124 | 137 => ResultKind::Timeout,
            _ => ResultKind::Failed,
        };
        let run = Run {
            when,
            result,
            wall,
            cpu: 0.0,
            peak: 0,
            load: 0.0,
            limit: None,
            beside: Some(beside),
        };
        runs.push((name.trim().to_owned(), run));
    }
    Ok(runs)
}

/// `--ui-cap` (prints the machine's cap within the bound) and
/// `--record-ui FILE` (adds check-all's UI runs to the profiles, learns
/// from them, prints what it learnt): the output.
pub fn command(o: &Options) -> Result<String, String> {
    let variable = std::env::var("MITCAD_UI_JOBS").ok();
    let bound = learning_bound(bound(o.ui_max, variable.as_deref()));
    let path = o.profiles.clone().or_else(profile::default_path);
    let Some(file) = &o.record_ui else {
        let profiles = match &path {
            Some(path) => profile::load(path)?,
            None => profile::Profiles::default(),
        };
        return Ok(effective_cap(profiles.ui_caps.get(PLATFORM), bound).to_string());
    };
    let text =
        fs::read_to_string(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    let runs = parse_ui_runs(&text, since.map_or(0, |d| d.as_secs()))?;
    let facts = run_facts(&runs);
    let Some(path) = path.filter(|_| o.record) else {
        let cap = effective_cap(None, bound);
        return Ok(summary(&facts, cap, cap));
    };
    let load = o.sensitive_load;
    let (before, after) = profile::update(&path, |profiles| {
        for (name, run) in &runs {
            let test = profiles.tests.entry(name.clone()).or_default();
            profile::record(test, run.clone(), load);
        }
        learn_into(&mut profiles.ui_caps, PLATFORM, &facts, bound)
    })?;
    Ok(summary(&facts, before, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ui(result: ResultKind, beside: u32) -> Run {
        Run {
            when: 0,
            result,
            wall: 10.0,
            cpu: 5.0,
            peak: 1 << 28,
            load: 0.5,
            limit: None,
            beside: Some(beside),
        }
    }

    fn records(runs: &[(&str, Run)]) -> Vec<(String, Run)> {
        runs.iter()
            .map(|(name, run)| ((*name).to_owned(), run.clone()))
            .collect()
    }

    const PASSED: ResultKind = ResultKind::Passed;
    const FAILED: ResultKind = ResultKind::Failed;

    #[test]
    fn the_bound_comes_from_the_option_or_the_variable() {
        assert_eq!(bound(Some(6), Some("12")), Some(6));
        assert_eq!(bound(None, Some("12")), Some(12));
        assert_eq!(bound(None, Some(" 3 ")), Some(3));
        assert_eq!(bound(None, Some("many")), None);
        assert_eq!(bound(None, Some("0")), None);
        assert_eq!(bound(None, None), None);
        // Learning goes on without one, within the default.
        assert_eq!(learning_bound(None), DEFAULT_BOUND);
        assert_eq!(learning_bound(Some(12)), 12);
    }

    #[test]
    fn without_a_bound_the_scheduler_holds_ui_entries_to_nothing() {
        let learnt = UiCap {
            cap: 2,
            clean_runs: 0,
        };
        // No --ui-max, no MITCAD_UI_JOBS: no cap and no rooms, as before.
        assert_eq!(scheduled_cap(Some(&learnt), None), usize::MAX);
        assert_eq!(scheduled_cap(None, None), usize::MAX);
        assert_eq!(scheduled_room(Some(1), None), None);
        assert!(admits(40, None, scheduled_room(Some(0), None), usize::MAX));
        // With a bound: the learnt cap within it, the rooms as learnt.
        assert_eq!(scheduled_cap(Some(&learnt), Some(12)), 2);
        assert_eq!(scheduled_cap(None, Some(12)), 12);
        assert_eq!(scheduled_room(Some(1), Some(12)), Some(1));
        assert_eq!(scheduled_room(None, Some(12)), None);
    }

    #[test]
    fn a_machine_starts_at_the_bound() {
        assert_eq!(start_cap(12), 12);
        assert_eq!(start_cap(4), 4);
        assert_eq!(start_cap(3), 3);
        assert_eq!(start_cap(1), 1);
        assert_eq!(start_cap(0), 1);
        assert_eq!(effective_cap(None, 12), 12);
        // A learnt cap within the bound; a lowered bound holds it down.
        let learnt = UiCap {
            cap: 9,
            clean_runs: 0,
        };
        assert_eq!(effective_cap(Some(&learnt), 12), 9);
        assert_eq!(effective_cap(Some(&learnt), 4), 4);
    }

    #[test]
    fn rooms_and_the_cap_admit_ui_entries() {
        // The cap counts the UI entries running.
        assert!(admits(1, None, None, 2));
        assert!(!admits(2, None, None, 2));
        // Its own room: with room 1 it may join one UI entry, not two.
        assert!(admits(1, None, Some(1), 4));
        assert!(!admits(2, None, Some(1), 4));
        // Room 0: only with no other UI entry running.
        assert!(admits(0, None, Some(0), 4));
        assert!(!admits(1, None, Some(0), 4));
        // A running entry's room holds back the others too.
        assert!(!admits(1, Some(0), None, 4));
        assert!(admits(1, Some(1), None, 4));
        assert!(!admits(2, Some(1), None, 4));
    }

    #[test]
    fn a_failure_beside_others_then_a_pass_alone_is_contention() {
        assert_eq!(contention(&ui(FAILED, 5), &ui(PASSED, 0)), Some(5));
        let timeout = ui(ResultKind::Timeout, 3);
        assert_eq!(contention(&timeout, &ui(PASSED, 1)), Some(3));
        // Failing alone, failing again alone, passing with as many beside:
        // not the company's fault.
        assert_eq!(contention(&ui(FAILED, 0), &ui(PASSED, 0)), None);
        assert_eq!(contention(&ui(FAILED, 5), &ui(FAILED, 0)), None);
        assert_eq!(contention(&ui(FAILED, 5), &ui(PASSED, 5)), None);
        // Not UI entries (no `beside`).
        let mut plain = ui(FAILED, 0);
        plain.beside = None;
        assert_eq!(contention(&plain, &ui(PASSED, 0)), None);
    }

    #[test]
    fn contention_halves_a_room_and_full_passes_grow_it() {
        let failed = ui(FAILED, 11);
        let alone = ui(PASSED, 0);
        // ui-pick failed beside 11 others and passed alone: room 5.
        assert_eq!(update_room(None, 0, Some(&failed), &alone), (Some(5), 0));
        // Again beside 5: room 2; a smaller room stays.
        let five = ui(FAILED, 5);
        assert_eq!(update_room(Some(5), 2, Some(&five), &alone), (Some(2), 0));
        assert_eq!(update_room(Some(1), 0, Some(&five), &alone), (Some(1), 0));
        // Beside 1: room 0, alone among the UI entries.
        let one = ui(FAILED, 1);
        assert_eq!(update_room(Some(2), 0, Some(&one), &alone), (Some(0), 0));
        // Passes with the room full count; three grow the room by one.
        let mut state = (Some(2), 0);
        for _ in 0..ROOM_GROW_PASSES {
            state = update_room(state.0, state.1, None, &ui(PASSED, 2));
        }
        assert_eq!(state, (Some(3), 0));
        // Passes with fewer beside, and other results, do not.
        assert_eq!(update_room(Some(3), 1, None, &ui(PASSED, 1)), (Some(3), 1));
        assert_eq!(update_room(Some(3), 1, None, &ui(FAILED, 3)), (Some(3), 1));
        // A room 0 grows after passes alone, so that it is tried again.
        let mut zero = (Some(0), 0);
        for _ in 0..ROOM_GROW_PASSES {
            zero = update_room(zero.0, zero.1, None, &ui(PASSED, 0));
        }
        assert_eq!(zero, (Some(1), 0));
        // Without a mark nothing changes.
        assert_eq!(update_room(None, 0, None, &ui(PASSED, 8)), (None, 0));
    }

    #[test]
    fn a_run_tells_its_most_at_once_and_its_failures() {
        // Today's Linux check: twelve at once, ui-pick and ui-render failed
        // and passed alone, ui-x failed beside others and alone.
        let runs = records(&[
            ("ui-a", ui(PASSED, 11)),
            ("ui-pick", ui(FAILED, 11)),
            ("ui-render", ui(FAILED, 9)),
            ("ui-x", ui(FAILED, 4)),
            ("ui-pick", ui(PASSED, 0)),
            ("ui-render", ui(PASSED, 0)),
            ("ui-x", ui(FAILED, 0)),
        ]);
        let facts = run_facts(&runs);
        assert_eq!(
            facts,
            RunFacts {
                at_once: 12,
                contended: 2,
                unresolved: 1
            }
        );
        // Tests that are not UI entries tell nothing.
        let mut plain = ui(FAILED, 0);
        plain.beside = None;
        let facts = run_facts(&records(&[("core.model", plain)]));
        assert_eq!(facts, RunFacts::default());
        // A failure alone is no failure beside others.
        let facts = run_facts(&records(&[("ui-a", ui(FAILED, 0))]));
        assert_eq!((facts.at_once, facts.unresolved), (1, 0));
    }

    #[test]
    fn the_cap_drops_on_contention_and_grows_slowly_when_clean() {
        let bound = 12;
        let facts = |at_once, contended, unresolved| RunFacts {
            at_once,
            contended,
            unresolved,
        };
        // A first run starts at the bound, 12: clean there, it stays.
        let mut cap = learn(None, &facts(12, 0, 0), bound);
        assert_eq!(
            cap,
            Some(UiCap {
                cap: 12,
                clean_runs: 1
            })
        );
        // A contention failure: one less each time, the clean runs start
        // again.
        cap = learn(cap.as_ref(), &facts(12, 1, 0), bound);
        assert_eq!(
            cap,
            Some(UiCap {
                cap: 11,
                clean_runs: 0
            })
        );
        cap = learn(cap.as_ref(), &facts(11, 1, 0), bound);
        assert_eq!(
            cap,
            Some(UiCap {
                cap: 10,
                clean_runs: 0
            })
        );
        // Slow growth: one clean run at the cap is not enough, two are.
        cap = learn(cap.as_ref(), &facts(10, 0, 0), bound);
        assert_eq!(
            cap,
            Some(UiCap {
                cap: 10,
                clean_runs: 1
            })
        );
        cap = learn(cap.as_ref(), &facts(10, 0, 0), bound);
        assert_eq!(
            cap,
            Some(UiCap {
                cap: 11,
                clean_runs: 0
            })
        );
        // A run below the cap or with an unresolved failure tells nothing.
        let same = learn(cap.as_ref(), &facts(3, 0, 0), bound);
        assert_eq!(same, cap);
        let same = learn(cap.as_ref(), &facts(11, 0, 1), bound);
        assert_eq!(same, cap);
        // Never below 1.
        let one = UiCap {
            cap: 1,
            clean_runs: 1,
        };
        let low = learn(Some(&one), &facts(1, 1, 0), bound);
        assert_eq!(low.map(|c| c.cap), Some(1));
        // Never above the bound.
        let top = UiCap {
            cap: 12,
            clean_runs: 1,
        };
        let held = learn(Some(&top), &facts(12, 0, 0), bound);
        assert_eq!(held.map(|c| c.cap), Some(12));
        // A run without UI entries leaves it as it was (or unset).
        assert_eq!(learn(None, &facts(0, 0, 0), bound), None);
        assert_eq!(learn(Some(&top), &facts(0, 0, 0), bound), Some(top));
    }

    #[test]
    fn caps_are_kept_per_platform() {
        let mut caps = BTreeMap::new();
        let clean = RunFacts {
            at_once: 2,
            ..RunFacts::default()
        };
        let contended = RunFacts {
            at_once: 4,
            contended: 1,
            ..RunFacts::default()
        };
        // Windows starts at its bound, 4, and drops after contention.
        assert_eq!(learn_into(&mut caps, "windows", &contended, 4), (4, 3));
        assert_eq!(caps["windows"].cap, 3);
        assert!(!caps.contains_key("linux"));
        // Linux keeps its own: a clean run below its start leaves it at 12,
        // and Windows' cap stays where it was.
        assert_eq!(learn_into(&mut caps, "linux", &clean, 12), (12, 12));
        assert_eq!((caps["linux"].cap, caps["windows"].cap), (12, 3));
        let line = summary(&contended, 4, 3);
        assert!(line.contains("cap 4 -> 3"), "{line}");
    }

    #[test]
    fn check_all_hands_over_its_ui_runs() {
        let text = "# name\tstatus\tseconds\tbeside\n\
                    ui-pick\t1\t40.5\t11\n\
                    \n\
                    ui-render\t124\t300\t9\r\n\
                    ui-pick\t0\t38\t0\n\
                    ui-skip\t77\t1\t2\n";
        let runs = parse_ui_runs(text, 7).unwrap();
        let kinds: Vec<(&str, ResultKind, Option<u32>)> = runs
            .iter()
            .map(|(n, r)| (n.as_str(), r.result, r.beside))
            .collect();
        assert_eq!(
            kinds,
            [
                ("ui-pick", FAILED, Some(11)),
                ("ui-render", ResultKind::Timeout, Some(9)),
                ("ui-pick", PASSED, Some(0)),
                ("ui-skip", ResultKind::Skipped, Some(2)),
            ]
        );
        assert_eq!((runs[0].1.when, runs[0].1.wall), (7, 40.5));
        assert_eq!(run_facts(&runs).contended, 1);
        assert!(parse_ui_runs("ui-pick\t0\t1\n", 0).is_err());
        assert!(parse_ui_runs("ui-pick\tok\t1\t0\n", 0).is_err());
    }
}
