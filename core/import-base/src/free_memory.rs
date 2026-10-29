// SPDX-License-Identifier: MIT
//! When an import's memory guard (`mitcad-ffi`) gives the memory the C
//! library's allocator holds free back to the system (mitcad#885).
//!
//! Memory an import frees stays with the allocator, resident, and an
//! allocator arena of one thread does not lend it to the others: the
//! definitions evaluated and the kernel's operations on several threads
//! left a large design's import at 6.9 GB resident with 4.1 GB in use, and
//! a job's memory limit of 6 GB ended it. Given back (glibc's
//! `malloc_trim`, 30–90 ms on 5 GB of arenas), the process stays near what
//! it uses. It is given back once the process has grown by
//! [`RELEASE_STEP`], or by an eighth of its size when that is more, since
//! the last time (or since the least it has been resident since), and
//! whenever its memory is tight; at most once per [`RELEASE_INTERVAL`], as
//! it takes the arenas' locks.

use std::time::{Duration, Instant};

/// The growth of the resident memory after which the allocator's free
/// memory is given back.
pub const RELEASE_STEP: u64 = 256 << 20;

/// The least time between two releases of the allocator's free memory.
pub const RELEASE_INTERVAL: Duration = Duration::from_secs(1);

/// When to give the allocator's free memory back (module documentation).
#[derive(Debug, Clone)]
pub struct FreeMemoryRelease {
    /// The resident memory after the last release (or the least since).
    after: u64,
    /// When it was last given back.
    at: Option<Instant>,
}

impl FreeMemoryRelease {
    /// For a process resident at `resident` (bytes).
    pub fn new(resident: u64) -> Self {
        Self {
            after: resident,
            at: None,
        }
    }

    /// Whether to give the free memory back now, the process resident at
    /// `resident` (bytes) and its memory `tight` or not.
    pub fn due(&mut self, resident: u64, tight: bool, now: Instant) -> bool {
        // (Freed and given back by itself meanwhile: grown from there.)
        self.after = self.after.min(resident);
        if self
            .at
            .is_some_and(|at| now.saturating_duration_since(at) < RELEASE_INTERVAL)
        {
            return false;
        }
        let step = RELEASE_STEP.max(self.after / 8);
        tight || resident >= self.after.saturating_add(step)
    }

    /// The memory was given back at `now`: the process is resident at
    /// `resident` (bytes).
    pub fn done(&mut self, resident: u64, now: Instant) {
        self.after = resident;
        self.at = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Given back once the process has grown by 256 MiB, or an eighth when
    /// it is over 2 GiB, since it last was or since the process's least
    /// size, and when memory is tight; at most once a second.
    #[test]
    fn free_memory_is_given_back_as_the_process_grows() {
        const MIB: u64 = 1 << 20;
        let start = Instant::now();
        let at = |s: u64| start + Duration::from_secs(s);
        let mut release = FreeMemoryRelease::new(500 * MIB);
        assert!(!release.due(700 * MIB, false, at(0)));
        assert!(release.due(756 * MIB, false, at(0)));
        release.done(600 * MIB, at(0));
        // Not again within the second, however it grows.
        assert!(!release.due(2000 * MIB, false, at(0)));
        assert!(release.due(856 * MIB, false, at(2)));
        release.done(4000 * MIB, at(2));
        // Over 2 GiB, by an eighth.
        assert!(!release.due(4400 * MIB, false, at(4)));
        assert!(release.due(4500 * MIB, false, at(4)));
        release.done(4000 * MIB, at(4));
        // Down by itself meanwhile: grown from there.
        assert!(!release.due(1000 * MIB, false, at(6)));
        assert!(release.due(1256 * MIB, false, at(6)));
        release.done(1100 * MIB, at(6));
        // Tight on memory: once a second, grown or not.
        assert!(release.due(1100 * MIB, true, at(8)));
        release.done(1100 * MIB, at(8));
        assert!(!release.due(1100 * MIB, true, at(8)));
        assert!(release.due(1100 * MIB, true, at(9)));
    }
}
