// SPDX-License-Identifier: MIT
//! Windows: each test in a Job Object of its own. ctest starts suspended,
//! goes into the job and only then runs, so every process it starts is in
//! the job (ctest's own job for its test is nested in it): the job gives
//! the tree's CPU time and peak committed memory, holds its memory limit,
//! reports going over it through a completion port, and kills what is left
//! when it closes.

use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::ptr::{self, null, null_mut};

use super::{Exit, Spec, Usage};

type Handle = *mut c_void;
type Bool = i32;

const CREATE_SUSPENDED: u32 = 0x0000_0004;
const JOB_OBJECT_LIMIT_JOB_MEMORY: u32 = 0x0000_0200;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
const JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION: i32 = 1;
const JOB_OBJECT_ASSOCIATE_COMPLETION_PORT_INFORMATION: i32 = 7;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
const JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT: u32 = 9;
const JOB_OBJECT_MSG_JOB_MEMORY_LIMIT: u32 = 10;
const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
const THREAD_SUSPEND_RESUME: u32 = 0x0002;

#[repr(C)]
#[allow(dead_code)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Default)]
struct MemoryStatusEx {
    length: u32,
    memory_load: u32,
    total_phys: u64,
    avail_phys: u64,
    total_page_file: u64,
    avail_page_file: u64,
    total_virtual: u64,
    avail_virtual: u64,
    avail_extended_virtual: u64,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Default)]
struct BasicLimit {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Default)]
struct IoCounters {
    read_operations: u64,
    write_operations: u64,
    other_operations: u64,
    read_transfer: u64,
    write_transfer: u64,
    other_transfer: u64,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Default)]
struct ExtendedLimit {
    basic: BasicLimit,
    io: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Default)]
struct BasicAccounting {
    total_user_time: i64,
    total_kernel_time: i64,
    this_period_total_user_time: i64,
    this_period_total_kernel_time: i64,
    total_page_fault_count: u32,
    total_processes: u32,
    active_processes: u32,
    total_terminated_processes: u32,
}

#[repr(C)]
#[allow(dead_code)]
struct AssociateCompletionPort {
    key: *mut c_void,
    port: Handle,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Default)]
struct ThreadEntry32 {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_process_id: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(job: Handle, class: i32, info: *const c_void, length: u32) -> Bool;
    fn QueryInformationJobObject(
        job: Handle,
        class: i32,
        info: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> Bool;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> Bool;
    fn TerminateJobObject(job: Handle, exit_code: u32) -> Bool;
    fn CloseHandle(handle: Handle) -> Bool;
    fn CreateIoCompletionPort(file: Handle, existing: Handle, key: usize, threads: u32) -> Handle;
    fn GetQueuedCompletionStatus(
        port: Handle,
        bytes: *mut u32,
        key: *mut usize,
        overlapped: *mut *mut c_void,
        milliseconds: u32,
    ) -> Bool;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
    fn Thread32First(snapshot: Handle, entry: *mut ThreadEntry32) -> Bool;
    fn Thread32Next(snapshot: Handle, entry: *mut ThreadEntry32) -> Bool;
    fn OpenThread(access: u32, inherit: Bool, thread_id: u32) -> Handle;
    fn ResumeThread(thread: Handle) -> u32;
    fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> Bool;
    fn GlobalMemoryStatusEx(status: *mut MemoryStatusEx) -> Bool;
    fn GetDiskFreeSpaceExW(
        path: *const u16,
        available: *mut u64,
        total: *mut u64,
        free: *mut u64,
    ) -> Bool;
}

fn invalid_handle() -> Handle {
    ptr::without_provenance_mut(usize::MAX)
}

/// A handle closed on drop.
struct Owned(Handle);

impl Owned {
    fn valid(&self) -> bool {
        !self.0.is_null() && self.0 != invalid_handle()
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        if self.valid() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

/// A test's ctest in its job.
pub struct Process {
    child: Child,
    // Declared after the child: the port first, then the job, whose
    // closing kills what is left of the tree.
    port: Owned,
    job: Owned,
    limit: Option<u64>,
    over_memory: bool,
    exit: Option<Exit>,
}

fn set_information<T>(job: &Owned, class: i32, value: &T) -> bool {
    let size = size_of::<T>() as u32;
    let info = ptr::from_ref(value).cast::<c_void>();
    unsafe { SetInformationJobObject(job.0, class, info, size) != 0 }
}

fn query_information<T>(job: &Owned, class: i32, value: &mut T) -> bool {
    let size = size_of::<T>() as u32;
    let info = ptr::from_mut(value).cast::<c_void>();
    unsafe { QueryInformationJobObject(job.0, class, info, size, null_mut()) != 0 }
}

/// Resumes the main thread of a process started suspended.
fn resume_main_thread(process_id: u32) -> bool {
    let snapshot = Owned(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) });
    if !snapshot.valid() {
        return false;
    }
    let mut entry = ThreadEntry32 {
        size: size_of::<ThreadEntry32>() as u32,
        ..ThreadEntry32::default()
    };
    let mut resumed = false;
    let mut more = unsafe { Thread32First(snapshot.0, &mut entry) } != 0;
    while more {
        if entry.owner_process_id == process_id {
            let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.thread_id) };
            let thread = Owned(raw);
            if thread.valid() {
                resumed |= unsafe { ResumeThread(thread.0) } != u32::MAX;
            }
        }
        more = unsafe { Thread32Next(snapshot.0, &mut entry) } != 0;
    }
    resumed
}

/// Starts the test's ctest in a new job.
pub fn spawn(spec: &Spec) -> io::Result<Process> {
    let log = File::create(&spec.log)?;
    let errors = log.try_clone()?;
    let job = Owned(unsafe { CreateJobObjectW(null(), null()) });
    if !job.valid() {
        return Err(io::Error::last_os_error());
    }
    let mut flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if spec.limit.is_some() {
        flags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
    }
    let limits = ExtendedLimit {
        basic: BasicLimit {
            limit_flags: flags,
            ..BasicLimit::default()
        },
        job_memory_limit: spec.limit.unwrap_or(0) as usize,
        ..ExtendedLimit::default()
    };
    if !set_information(&job, JOB_OBJECT_EXTENDED_LIMIT_INFORMATION, &limits) {
        return Err(io::Error::last_os_error());
    }
    // The notices of going over the memory limit (optional).
    let raw = unsafe { CreateIoCompletionPort(invalid_handle(), null_mut(), 0, 1) };
    let port = Owned(raw);
    if port.valid() {
        let association = AssociateCompletionPort {
            key: null_mut(),
            port: port.0,
        };
        let class = JOB_OBJECT_ASSOCIATE_COMPLETION_PORT_INFORMATION;
        set_information(&job, class, &association);
    }
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(errors)
        .creation_flags(CREATE_SUSPENDED);
    let mut child = command.spawn()?;
    let handle: Handle = child.as_raw_handle();
    let assigned = unsafe { AssignProcessToJobObject(job.0, handle) } != 0;
    if !assigned || !resume_main_thread(child.id()) {
        let error = io::Error::last_os_error();
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    Ok(Process {
        child,
        port,
        job,
        limit: spec.limit,
        over_memory: false,
        exit: None,
    })
}

impl Process {
    fn drain_messages(&mut self) {
        if !self.port.valid() {
            return;
        }
        loop {
            let mut message = 0u32;
            let mut key = 0usize;
            let mut overlapped: *mut c_void = null_mut();
            let port = self.port.0;
            let got = unsafe {
                GetQueuedCompletionStatus(port, &mut message, &mut key, &mut overlapped, 0)
            };
            if got == 0 {
                break;
            }
            if message == JOB_OBJECT_MSG_JOB_MEMORY_LIMIT
                || message == JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT
            {
                self.over_memory = true;
            }
        }
    }

    /// The exit once ctest has ended; never blocks.
    pub fn poll(&mut self) -> Option<Exit> {
        self.drain_messages();
        if self.exit.is_some() {
            return self.exit;
        }
        if let Ok(Some(status)) = self.child.try_wait() {
            self.exit = Some(Exit {
                success: status.success(),
                code: status.code(),
            });
        }
        self.exit
    }

    /// The tree's working set now: not sampled here (the free budget,
    /// `--memory-budget free`, is for bare-metal Linux), so its whole
    /// prediction counts as still to come.
    pub fn current(&self) -> Option<u64> {
        None
    }

    /// Kills the whole tree.
    pub fn kill(&mut self) {
        unsafe {
            TerminateJobObject(self.job.0, 1);
        }
        let _ = self.child.kill();
    }

    /// What the tree used (after the exit).
    pub fn usage(&mut self) -> Usage {
        self.drain_messages();
        let mut accounting = BasicAccounting::default();
        let class = JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION;
        let cpu = if query_information(&self.job, class, &mut accounting) {
            let ticks = accounting.total_user_time + accounting.total_kernel_time;
            ticks as f64 / 1e7
        } else {
            0.0
        };
        let mut limits = ExtendedLimit::default();
        let class = JOB_OBJECT_EXTENDED_LIMIT_INFORMATION;
        let peak = if query_information(&self.job, class, &mut limits) {
            limits.peak_job_memory_used as u64
        } else {
            0
        };
        // A failed allocation may stop short of the limit itself.
        let at_limit = self.limit.is_some_and(|l| peak as f64 >= l as f64 * 0.95);
        Usage {
            cpu,
            peak,
            over_memory: self.over_memory || at_limit,
            tree: true,
        }
    }
}

fn ticks(time: &FileTime) -> u64 {
    (u64::from(time.high) << 32) | u64::from(time.low)
}

fn system_times() -> Option<(u64, u64)> {
    let mut idle = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
    if !ok {
        return None;
    }
    // Kernel time includes the idle time.
    let total = ticks(&kernel) + ticks(&user);
    Some((total.saturating_sub(ticks(&idle)), total))
}

/// The system's CPU load between samples.
pub struct CpuSampler {
    last: Option<(u64, u64)>,
}

impl CpuSampler {
    pub fn start() -> CpuSampler {
        CpuSampler {
            last: system_times(),
        }
    }

    /// The load since the last sample, 0 to 1.
    pub fn sample(&mut self) -> Option<f64> {
        let now = system_times()?;
        let before = self.last.replace(now)?;
        Some(super::load_between(before, now))
    }
}

fn memory_status() -> Option<MemoryStatusEx> {
    let mut status = MemoryStatusEx {
        length: size_of::<MemoryStatusEx>() as u32,
        ..MemoryStatusEx::default()
    };
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) } != 0;
    ok.then_some(status)
}

/// The machine's physical memory, bytes.
pub fn physical_memory() -> Option<u64> {
    memory_status().map(|s| s.total_phys)
}

/// The commit limit (physical memory plus the paging file), bytes. In a
/// VM with dynamic memory the physical memory is only what is assigned
/// now; the commit limit is larger and grows with it.
pub fn commit_limit() -> Option<u64> {
    memory_status().map(|s| s.total_page_file)
}

/// The physical memory available now, bytes.
pub fn available_memory() -> Option<u64> {
    memory_status().map(|s| s.avail_phys)
}

/// The free disk space of the volume the path is on, bytes.
pub fn disk_free(path: &Path) -> Option<u64> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut available = 0u64;
    let ok =
        unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, null_mut(), null_mut()) != 0 };
    ok.then_some(available)
}

/// Cgroups are Linux's.
pub fn cgroups_available() -> bool {
    false
}

/// How tests are measured here.
pub fn method(_cgroup: bool) -> &'static str {
    "a Job Object per test (tree CPU time, peak committed memory, memory limit)"
}
