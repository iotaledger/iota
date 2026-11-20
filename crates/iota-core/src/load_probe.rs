// TEMP benchmark: per-thread heap and CPU counters around one execution. The
// test binary installs `CountingAlloc` as its global allocator. Not for commit.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

/// Whether the `LOAD_*` lines are printed: only when `STRESS_LOAD_PROBE` is
/// set, as the stress kit sets it for its test. Nodes of a network benchmark
/// built from the same commit run without them.
pub fn enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("STRESS_LOAD_PROBE").is_some());
    *ENABLED
}

thread_local! {
    static CURRENT: Cell<i64> = const { Cell::new(0) };
    static PEAK: Cell<i64> = const { Cell::new(0) };
    static ALLOCATED: Cell<i64> = const { Cell::new(0) };
}

pub struct CountingAlloc;

fn record(delta: i64) {
    let _ = CURRENT.try_with(|current| {
        let value = current.get() + delta;
        current.set(value);
        let _ = PEAK.try_with(|peak| {
            if value > peak.get() {
                peak.set(value)
            }
        });
    });
    if delta > 0 {
        let _ = ALLOCATED.try_with(|allocated| allocated.set(allocated.get() + delta));
    }
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record(layout.size() as i64);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record(layout.size() as i64);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        record(-(layout.size() as i64));
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            record(new_size as i64 - layout.size() as i64);
        }
        new_ptr
    }
}

unsafe extern "C" {
    fn clock_gettime(clock: i32, time: *mut [i64; 2]) -> i32;
}

#[cfg(target_os = "macos")]
const CLOCK_THREAD_CPUTIME_ID: i32 = 16;
#[cfg(not(target_os = "macos"))]
const CLOCK_THREAD_CPUTIME_ID: i32 = 3;

fn thread_cpu_us() -> i64 {
    let mut time = [0i64; 2];
    unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &mut time) };
    time[0] * 1_000_000 + time[1] / 1_000
}

pub struct Probe {
    base: i64,
    allocated: i64,
    cpu_us: i64,
}

impl Probe {
    pub fn start() -> Self {
        let base = CURRENT.with(|current| current.get());
        PEAK.with(|peak| peak.set(base));
        Self {
            base,
            allocated: ALLOCATED.with(|allocated| allocated.get()),
            cpu_us: thread_cpu_us(),
        }
    }

    /// `cpu_us=… heap_peak=… heap_retained=… heap_allocated=… end_ms=…`, bytes
    /// measured on this thread since `start`, `end_ms` in Unix milliseconds.
    pub fn report(&self) -> String {
        format!(
            "cpu_us={} heap_peak={} heap_retained={} heap_allocated={} end_ms={}",
            thread_cpu_us() - self.cpu_us,
            PEAK.with(|peak| peak.get()) - self.base,
            CURRENT.with(|current| current.get()) - self.base,
            ALLOCATED.with(|allocated| allocated.get()) - self.allocated,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis()),
        )
    }
}
