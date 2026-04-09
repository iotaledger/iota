// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};

use tracing::info;
use tracing_subscriber::{
    Registry,
    filter::{LevelFilter, filter_fn},
    fmt::{self, MakeWriter},
    prelude::*,
};

// These act as our "Context" to route logs to the right file.
tokio::task_local! {
    pub static IS_LOOP: bool;
}

// Helper to check flags
fn is_loop() -> bool {
    IS_LOOP.try_with(|v| *v).unwrap_or(false)
}

// Shared writer for the loop layer that can be swapped
// This is used in `run_benchmark` to redirect logs to different set of
// parameters benchmarks
pub struct SwappableWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl Default for SwappableWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl SwappableWriter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Box::new(std::io::sink()))),
        }
    }

    pub fn swap(&self, new_writer: Box<dyn Write + Send>) -> std::io::Result<()> {
        let mut writer = self.inner.lock().unwrap();
        // Flush the old writer before swapping to ensure buffered data is written
        writer.flush()?;
        *writer = new_writer;
        Ok(())
    }
}

impl Clone for SwappableWriter {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Write for SwappableWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.lock().unwrap().flush()
    }
}

impl<'a> MakeWriter<'a> for SwappableWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Initialize the logger with a file path
pub fn init_logger<P: AsRef<Path>>(benchmark_dir: P) -> std::io::Result<SwappableWriter> {
    // Main logs: Accept everything that is NOT in the Op function
    let main_filter = filter_fn(|_| !is_loop());

    // Loop logs: Accept ONLY things inside the Loop
    let loop_filter = filter_fn(|_| is_loop());

    // Layer 1: Main.log
    let main_path = benchmark_dir.as_ref().join("main.log");
    let main_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(main_path)?;
    let main_layer = fmt::Layer::default()
        .with_ansi(false)
        .with_writer(Arc::new(main_file))
        .with_filter(main_filter)
        .with_filter(LevelFilter::INFO);

    // Layer 2: Loop Layer - using swappable writer
    let loop_writer = SwappableWriter::new();
    let loop_layer = fmt::Layer::default()
        .with_ansi(false)
        .with_writer(loop_writer.clone())
        .with_filter(loop_filter)
        .with_filter(LevelFilter::INFO);

    Registry::default().with(main_layer).with(loop_layer).init();

    Ok(loop_writer)
}

/// Log a message to the file (if initialized)
pub fn log(message: &str) {
    info!("{}", message);
}
