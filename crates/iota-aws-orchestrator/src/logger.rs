// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::Mutex,
};

/// Global logger instance for writing to both stdout and file
static LOGGER: Mutex<Option<File>> = Mutex::new(None);

/// Initialize the logger with a file path
pub fn init_logger<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;

    LOGGER.lock().unwrap().replace(file);
    Ok(())
}

/// Log a message to the file (if initialized)
pub fn log(message: &str) {
    if let Some(mut file) = LOGGER.lock().unwrap().as_ref() {
        let _ = write!(file, "{}", message);
        let _ = file.flush();
    }
}

/// Close the logger
pub fn close_logger() {
    LOGGER.lock().unwrap().take();
}
