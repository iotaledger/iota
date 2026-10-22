// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Resident-memory readings for the memory-cost calibration: the profile's
//! memory counters are the predictors, and peak resident set size is the
//! response variable their scale factors are fitted against.

/// Peak resident set size of this process since it started, in bytes.
/// Returns 0 if the reading fails.
pub fn peak_rss_bytes() -> u64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
        return 0;
    }
    let raw = usage.ru_maxrss as u64;
    // macOS reports bytes; Linux reports kilobytes.
    if cfg!(target_os = "macos") {
        raw
    } else {
        raw * 1024
    }
}

/// Current resident set size of this process, in bytes. Returns 0 on
/// platforms where no reading is implemented or if the reading fails.
#[cfg(target_os = "linux")]
pub fn current_rss_bytes() -> u64 {
    let Ok(statm) = std::fs::read_to_string("/proc/self/statm") else {
        return 0;
    };
    let Some(resident_pages) = statm.split_whitespace().nth(1) else {
        return 0;
    };
    let Ok(pages) = resident_pages.parse::<u64>() else {
        return 0;
    };
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    pages * page_size.max(0) as u64
}

/// Current resident set size of this process, in bytes. Returns 0 if the
/// reading fails.
#[cfg(target_os = "macos")]
pub fn current_rss_bytes() -> u64 {
    let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
    let read = unsafe {
        libc::proc_pidinfo(
            std::process::id() as libc::c_int,
            libc::PROC_PIDTASKINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        )
    };
    if read == size {
        info.pti_resident_size
    } else {
        0
    }
}

/// Current resident set size, unavailable on this platform.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn current_rss_bytes() -> u64 {
    0
}
