// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt::Display,
    io::{self, Write, stdout},
};

use crossterm::{
    cursor::{RestorePosition, SavePosition},
    style::{Print, PrintStyledContent, Stylize},
    terminal::{Clear, ClearType},
};
use prettytable::format::{self};

use crate::logger;

pub fn header<S: Display>(message: S) {
    let msg = format!("\n{message}\n");
    logger::log(&msg);
    crossterm::execute!(stdout(), PrintStyledContent(msg.green().bold()),).unwrap();
}

pub fn error<S: Display>(message: S) {
    let msg = format!("\n{message}\n");
    logger::log(&msg);
    crossterm::execute!(stdout(), PrintStyledContent(msg.red().bold()),).unwrap();
}

pub fn warn<S: Display>(message: S) {
    let msg = format!("\n{message}\n");
    logger::log(&msg);
    crossterm::execute!(stdout(), PrintStyledContent(msg.bold()),).unwrap();
}

pub fn config<N: Display, V: Display>(name: N, value: V) {
    logger::log(&format!("{name}: {value}\n"));
    crossterm::execute!(
        stdout(),
        PrintStyledContent(format!("{name}: ").bold()),
        Print(format!("{value}\n"))
    )
    .unwrap();
}

pub fn confirm<S: Display>(message: S) -> bool {
    // Print the prompt
    crossterm::execute!(
        stdout(),
        PrintStyledContent(format!("{message} ").bold()),
        Print("[y/N]: ")
    )
    .unwrap();

    // Make sure prompt is visible before waiting for input
    stdout().flush().unwrap();

    // Read input
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) => {
            let normalized = input.trim().to_lowercase();
            matches!(normalized.as_str(), "y" | "yes")
        }
        Err(_) => false,
    }
}

pub fn action<S: Display>(message: S) {
    let msg: String = format!("{message} ... ");
    logger::log(&msg);
    crossterm::execute!(stdout(), Print(&msg), SavePosition).unwrap();
}

pub fn status<S: Display>(status: S) {
    let msg = format!("[{status}]");
    logger::log(&msg);
    crossterm::execute!(
        stdout(),
        RestorePosition,
        SavePosition,
        Clear(ClearType::UntilNewLine),
        Print(&msg)
    )
    .unwrap();
}

pub fn done() {
    logger::log("[Ok]\n");
    crossterm::execute!(
        stdout(),
        RestorePosition,
        Clear(ClearType::UntilNewLine),
        Print(format!("[{}]\n", "Ok".green()))
    )
    .unwrap();
}

pub fn newline() {
    logger::log("\n");
    crossterm::execute!(stdout(), Print("\n")).unwrap();
}

/// Default style for tables printed to stdout.
pub fn default_table_format() -> format::TableFormat {
    format::FormatBuilder::new()
        .separators(
            &[
                format::LinePosition::Top,
                format::LinePosition::Bottom,
                format::LinePosition::Title,
            ],
            format::LineSeparator::new('-', '-', '-', '-'),
        )
        .padding(1, 1)
        .build()
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use tokio::time::sleep;

    use super::{action, config, done, error, header, newline, warn};
    use crate::display::status;

    #[tokio::test]
    #[ignore = "only used to manually check if prints work correctly"]
    async fn display() {
        header("This is a header");
        config("This is a config", 2);
        action("Running a long function");
        for i in 0..5 {
            sleep(Duration::from_secs(1)).await;
            if i == 2 {
                warn("This is a warning!");
            }
            status(format!("{}/5", i + 1));
        }
        done();
        error("This is an error!");
        warn("This is a warning!");
        newline();
    }
}
