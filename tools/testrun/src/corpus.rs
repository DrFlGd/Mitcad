// SPDX-License-Identifier: MIT
//! The corpus tests' children (mitcad#832). A corpus test imports its files
//! in child processes, several at once (`core/tests/parallel_runs.hpp`,
//! `mitcad_run_parallel`). Each run records every file's time and peak in
//! the table `MITCAD_CORPUS_RECORD` names; the profiles keep them per test
//! (`TestProfile::files`), and the next run hands them back in the table
//! `MITCAD_CORPUS_FILES` names, so that the longest files start first.
//! With `--corpus-children` the runner also gives each corpus test with
//! such a history its children at once (`MITCAD_CORPUS_JOBS`) and their
//! memory limit (`MITCAD_CORPUS_MEMORY`) from it: a long test more, a short
//! one fewer, all of them within the cores and the memory budget
//! (`allocate`). A corpus test without file history keeps what its caller
//! set (tools/check-all.sh's fixed plan).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The children at once of a corpus test.
pub const JOBS: &str = "MITCAD_CORPUS_JOBS";
/// The memory a child may commit.
pub const MEMORY: &str = "MITCAD_CORPUS_MEMORY";
/// The table of the files' last times the test reads.
pub const FILES: &str = "MITCAD_CORPUS_FILES";
/// The table the test appends each child's time and peak to.
pub const RECORD: &str = "MITCAD_CORPUS_RECORD";
/// With the free budget (mitcad#872): the memory a test's children leave
/// to the rest of the machine; a child beside others starts only while
/// the memory available exceeds it by the child's expected peak.
pub const RESERVE: &str = "MITCAD_CORPUS_RESERVE";

/// A file not run for this long leaves its test's profile (60 days).
pub const FILE_AGE: u64 = 60 * 86_400;
/// The memory of a corpus test besides its children (its parent process).
pub const PARENT_MEMORY: u64 = 256 << 20;
/// A child's memory limit when the caller sets none.
pub const DEFAULT_CHILD_LIMIT: u64 = 1536 << 20;
/// The least memory predicted for a child.
const CHILD_FLOOR: u64 = 64 << 20;

/// A file's last run in its test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FileRun {
    /// The child's wall time, seconds.
    pub wall: f64,
    /// The child's peak memory, bytes.
    pub peak: u64,
    /// When it was recorded, seconds since the Unix epoch.
    #[serde(default)]
    pub when: u64,
}

/// The rows a run recorded (key, seconds, peak bytes and the result,
/// tab-separated), each as recorded at `when`; malformed lines are left
/// out.
pub fn parse_record(text: &str, when: u64) -> Vec<(String, FileRun)> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.trim_end_matches('\r').split('\t');
            let key = fields.next().filter(|k| !k.is_empty())?;
            let wall: f64 = fields.next()?.trim().parse().ok()?;
            let peak: u64 = fields.next()?.trim().parse().ok()?;
            let ok = wall.is_finite() && wall >= 0.0;
            ok.then(|| (key.to_owned(), FileRun { wall, peak, when }))
        })
        .collect()
}

/// Adds a run's rows to a test's files (each file's newest run replaces
/// its last) and drops the files not run for `FILE_AGE`. No rows at all
/// (`one_process`) drops every file.
pub fn merge(files: &mut BTreeMap<String, FileRun>, rows: &[(String, FileRun)], now: u64) {
    if rows.is_empty() {
        files.clear();
        return;
    }
    for (key, run) in rows {
        files.insert(key.clone(), *run);
    }
    files.retain(|_, run| now.saturating_sub(run.when) <= FILE_AGE);
}

/// Whether a corpus test's run shows that it no longer imports its files
/// in children: it passed and its children recorded nothing, although it
/// has files from runs before (f3d.corpus_designs, which checks its
/// designs one after another again since 0.32.0). Its files then go from
/// its profile (an empty `merge`), so that the next run plans it by its
/// own history (its whole time in one process) rather than by children it
/// does not start (it was planned at its longest design's 314 s and took
/// 498 s, with two children's memory, 7 GiB, held for it in the budget).
pub fn one_process(passed: bool, recorded: usize, known_files: usize) -> bool {
    passed && recorded == 0 && known_files > 0
}

/// The table handed to the test (`FILES`): key, seconds and peak bytes,
/// tab-separated, a line each.
pub fn table(files: &BTreeMap<String, FileRun>) -> String {
    files
        .iter()
        .map(|(key, run)| format!("{key}\t{:.2}\t{}\n", run.wall, run.peak))
        .collect()
}

/// What a corpus test with recorded files asks of the machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Demand {
    /// The child-seconds of all its files.
    pub work: f64,
    /// Its longest file, seconds.
    pub longest: f64,
    /// Its files.
    pub files: usize,
    /// Its largest file's peak, bytes (without a margin).
    pub peak: u64,
}

impl Demand {
    /// From a test's files; None without any.
    pub fn of(files: &BTreeMap<String, FileRun>) -> Option<Demand> {
        if files.is_empty() {
            return None;
        }
        Some(Demand {
            work: files.values().map(|r| r.wall).sum(),
            longest: files.values().map(|r| r.wall).fold(0.0, f64::max),
            files: files.len(),
            peak: files.values().map(|r| r.peak).max().unwrap_or(0),
        })
    }

    /// Its wall time with `children` at once: its work shared out, never
    /// below its longest file.
    pub fn wall(&self, children: usize) -> f64 {
        self.longest.max(self.work / children.max(1) as f64)
    }

    /// A child's predicted memory: the largest peak times the margin.
    pub fn child_memory(&self, margin: f64) -> u64 {
        ((self.peak as f64 * margin) as u64).max(CHILD_FLOOR)
    }

    /// A child's memory limit: the largest peak times the factor, never
    /// below the caller's limit (`MEMORY`, else `DEFAULT_CHILD_LIMIT`),
    /// which held the children before they had a history, nor above the
    /// ceiling. The limit is on committed memory (RLIMIT_DATA), which
    /// exceeds the resident peak by the threads' stacks and the
    /// allocators' reserves, hence the factor.
    pub fn child_limit(&self, factor: f64, floor: u64, ceiling: u64) -> u64 {
        let measured = (self.peak as f64 * factor.max(1.0)) as u64;
        measured.max(floor).min(ceiling.max(floor))
    }
}

/// The cores and the memory the planned corpus tests share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    pub cores: usize,
    pub memory: u64,
}

/// Each test's children at once. Every test starts with two (one with a
/// single file); then a child at a time goes to the test that would end
/// last (its predicted wall time), as long as one more shortens it (it has
/// more files than children and its work shared out is longer than its
/// longest file), the children together stay within the cores and the
/// tests' predicted memory (`PARENT_MEMORY` and `child_memory` for each
/// child) within the memory. So a long test gets more children, a short
/// one fewer, and they end at about the same time.
pub fn allocate(demands: &[Demand], child_memory: &[u64], room: Room) -> Vec<usize> {
    let mut children: Vec<usize> = demands.iter().map(|d| d.files.clamp(1, 2)).collect();
    let mut cores: usize = children.iter().sum();
    let mut memory: u64 = child_memory
        .iter()
        .zip(&children)
        .map(|(m, n)| PARENT_MEMORY + *n as u64 * *m)
        .sum();
    while cores < room.cores {
        // The test that would end last among those one more child shortens.
        let mut next: Option<usize> = None;
        for (i, d) in demands.iter().enumerate() {
            let n = children[i];
            let helps = n < d.files && d.wall(n + 1) < d.wall(n);
            if !helps || memory + child_memory[i] > room.memory {
                continue;
            }
            if next.is_none_or(|j| d.wall(n) >= demands[j].wall(children[j])) {
                next = Some(i);
            }
        }
        let Some(i) = next else {
            break;
        };
        children[i] += 1;
        cores += 1;
        memory += child_memory[i];
    }
    children
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u64 = 1 << 20;
    const GIB: u64 = 1 << 30;

    fn demand(work: f64, longest: f64, files: usize, peak: u64) -> Demand {
        Demand {
            work,
            longest,
            files,
            peak,
        }
    }

    #[test]
    fn records_parse_merge_and_go_back_as_a_table() {
        let text =
            "a.f3d\t12.50\t1048576\tok\nmalformed\nb\tsoon\t1\tok\nc/d e.ipt\t3\t2\tfailed\r\n";
        let rows = parse_record(text, 1000);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "a.f3d");
        assert_eq!(
            rows[0].1,
            FileRun {
                wall: 12.5,
                peak: MIB,
                when: 1000
            }
        );
        assert_eq!(rows[1].0, "c/d e.ipt");
        let mut files = BTreeMap::new();
        files.insert(
            "old".to_owned(),
            FileRun {
                wall: 1.0,
                peak: 1,
                when: 1,
            },
        );
        files.insert(
            "a.f3d".to_owned(),
            FileRun {
                wall: 99.0,
                peak: 1,
                when: 900,
            },
        );
        merge(&mut files, &rows, 1000 + FILE_AGE);
        // The newest run replaces the last; a file not run for 60 days goes.
        assert_eq!(files.len(), 2);
        assert_eq!(files["a.f3d"].wall, 12.5);
        assert!(!files.contains_key("old"));
        assert_eq!(table(&files), "a.f3d\t12.50\t1048576\nc/d e.ipt\t3.00\t2\n");
        // A test that passed without its children recording anything runs
        // in one process now: its files go.
        assert!(one_process(true, 0, 2));
        assert!(!one_process(false, 0, 2));
        assert!(!one_process(true, 1, 2));
        assert!(!one_process(true, 0, 0));
        merge(&mut files, &[], 2000);
        assert!(files.is_empty());
    }

    #[test]
    fn a_demand_comes_from_the_files() {
        assert_eq!(Demand::of(&BTreeMap::new()), None);
        let mut files = BTreeMap::new();
        files.insert(
            "a".to_owned(),
            FileRun {
                wall: 30.0,
                peak: GIB,
                when: 0,
            },
        );
        files.insert(
            "b".to_owned(),
            FileRun {
                wall: 10.0,
                peak: 2 * GIB,
                when: 0,
            },
        );
        let d = Demand::of(&files).unwrap();
        assert_eq!(d, demand(40.0, 30.0, 2, 2 * GIB));
        assert_eq!(d.wall(1), 40.0);
        assert_eq!(d.wall(2), 30.0);
        assert_eq!(d.child_memory(1.25), 5 * GIB / 2);
        // Twice the peak, never below the caller's limit or above the ceiling.
        assert_eq!(d.child_limit(2.0, 1536 * MIB, 64 * GIB), 4 * GIB);
        assert_eq!(
            demand(1.0, 1.0, 1, 100 * MIB).child_limit(2.0, 1536 * MIB, 64 * GIB),
            1536 * MIB
        );
        assert_eq!(d.child_limit(2.0, 1536 * MIB, 3 * GIB), 3 * GIB);
    }

    #[test]
    fn a_long_test_gets_more_children_than_a_short_one() {
        // 353 s of work in 5 files against 60 s in 30 files, 16 cores.
        let long = demand(353.0, 120.0, 5, GIB);
        let wide = demand(600.0, 5.0, 30, GIB / 2);
        let short = demand(60.0, 6.0, 30, GIB / 2);
        let room = Room {
            cores: 16,
            memory: 64 * GIB,
        };
        let n = allocate(&[long, wide, short], &[GIB, GIB / 2, GIB / 2], room);
        assert_eq!(n.iter().sum::<usize>(), 16);
        assert!(n[1] > n[2], "{n:?}");
        // The long test's children stop where its longest file decides.
        assert_eq!(n[0], 3, "{n:?}");
        let walls: Vec<f64> = [long, wide, short]
            .iter()
            .zip(&n)
            .map(|(d, &k)| d.wall(k))
            .collect();
        assert!(walls[1] <= 120.0 && walls[2] <= 120.0, "{walls:?}");
    }

    #[test]
    fn the_memory_bounds_the_children() {
        let a = demand(1000.0, 1.0, 100, GIB);
        let room = Room {
            cores: 32,
            memory: PARENT_MEMORY + 5 * GIB,
        };
        assert_eq!(allocate(&[a], &[GIB], room), [5]);
        // Too little for even the first two: they are kept.
        let tight = Room {
            cores: 32,
            memory: GIB,
        };
        assert_eq!(allocate(&[a], &[GIB], tight), [2]);
    }

    #[test]
    fn the_cores_bound_the_children_and_one_file_needs_one() {
        let a = demand(1000.0, 1.0, 100, MIB);
        let one = demand(50.0, 50.0, 1, MIB);
        let room = Room {
            cores: 10,
            memory: 64 * GIB,
        };
        assert_eq!(allocate(&[a, one], &[MIB, MIB], room), [9, 1]);
        // No more children than files, and none past the longest file.
        let few = demand(100.0, 40.0, 4, MIB);
        assert_eq!(allocate(&[few], &[MIB], room), [3]);
    }
}
