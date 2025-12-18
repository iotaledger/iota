// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::{Mutex, OnceLock},
};

/// Global logger instance for writing to both stdout and file
static LOGGER: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// Initialize the logger with a file path
pub fn init_logger<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;

    LOGGER.get_or_init(|| Mutex::new(Some(file)));
    Ok(())
}

/// Log a message to the file (if initialized)
pub fn log(message: &str) {
    if let Some(logger) = LOGGER.get() {
        if let Ok(mut guard) = logger.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = write!(file, "{}", message);
                let _ = file.flush();
            }
        }
    }
}

/// Close the logger
pub fn close_logger() {
    if let Some(logger) = LOGGER.get() {
        if let Ok(mut guard) = logger.lock() {
            *guard = None;
        }
    }
}
