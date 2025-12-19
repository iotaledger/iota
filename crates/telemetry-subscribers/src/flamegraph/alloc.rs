// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    alloc::{GlobalAlloc, System},
    fmt,
};

#[derive(Default)]
pub struct CounterAlloc<A = System>(A);
impl<A> CounterAlloc<A> {
    pub const fn new(alloc: A) -> Self {
        CounterAlloc(alloc)
    }
}

#[thread_local]
static mut THREAD_METRICS: AllocMetrics = AllocMetrics::new();

pub fn get_alloc_metrics() -> AllocMetrics {
    unsafe { THREAD_METRICS }
}

fn update_metrics(old: usize, new: usize) {
    unsafe {
        THREAD_METRICS.alloc += new;
    }
    unsafe {
        THREAD_METRICS.dealloc += old;
    }
    unsafe {
        THREAD_METRICS.peak = THREAD_METRICS
            .alloc
            .abs_diff(THREAD_METRICS.dealloc)
            .max(THREAD_METRICS.peak);
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for CounterAlloc<A> {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        update_metrics(0, layout.size());
        unsafe { self.0.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        update_metrics(layout.size(), 0);
        unsafe { self.0.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
        update_metrics(layout.size(), new_size);
        unsafe { self.0.realloc(ptr, layout, new_size) }
    }
    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        update_metrics(0, layout.size());
        unsafe { self.0.alloc_zeroed(layout) }
    }
}

#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct AllocMetrics {
    pub alloc: usize,
    pub dealloc: usize,
    pub peak: usize,
}
impl AllocMetrics {
    pub const fn new() -> Self {
        Self {
            alloc: 0,
            dealloc: 0,
            peak: 0,
        }
    }
    pub fn delta(&self, other: Self) -> Self {
        Self {
            alloc: self.alloc.saturating_sub(other.alloc),
            dealloc: self.dealloc.saturating_sub(other.dealloc),
            peak: self.peak.saturating_sub(other.peak),
        }
    }
    pub fn merge(&mut self, other: Self) {
        self.alloc += other.alloc;
        self.dealloc += other.dealloc;
        self.peak = self.peak.max(other.peak);
    }
}
struct Iec(usize);
impl fmt::Display for Iec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 < 1024 {
            write!(f, "{}B", self.0)
        } else if self.0 < 1024 * 1024 {
            write!(f, "{:.2}KiB", self.0 as f64 / 1024.0)
        } else if self.0 < 1024 * 1024 * 1024 {
            write!(f, "{:.2}MiB", self.0 as f64 / 1024.0 / 1024.0)
        } else {
            write!(f, "{:.2}GiB", self.0 as f64 / 1024.0 / 1024.0 / 1024.0)
        }
    }
}
impl fmt::Debug for AllocMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "alloc={} dealloc={} peak={}",
            self.alloc, self.dealloc, self.peak
        )
    }
}
impl fmt::Display for AllocMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "alloc={} total={} peak={}",
            Iec(self.alloc.saturating_sub(self.dealloc)),
            Iec(self.alloc),
            Iec(self.peak)
        )
    }
}
