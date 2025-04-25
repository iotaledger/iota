// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use csv::{Reader, Writer};
use iota_mainnet_unlocks::store::{INPUT_FILE as OUTPUT_FILE, StillLockedEntry};
use regex::Regex;
use tempfile::{TempDir, tempdir};

// Folders in the raw data repository that contain the CSV files.
const FOLDERS: &[&str] = &[
    "Assembly_IF_Members",
    "Assembly_Investors",
    "IOTA_Airdrop",
    "IOTA_Foundation",
    "New_Investors",
    "TEA",
    "Treasury_DAO",
    "UAE",
];

/// Clones the repository containing raw data into a temporary directory.
fn clone_repo(tmp_dir: &TempDir) -> Result<PathBuf> {
    let repo_path = tmp_dir.path().join("new_supply");

    let status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "https://github.com/iotaledger/new_supply.git",
        ])
        .arg(&repo_path)
        .status()
        .context("failed to execute `git clone`")?;

    if !status.success() {
        anyhow::bail!("`git clone` failed with exit status: {}", status);
    }

    Ok(repo_path)
}

/// Reads and aggregates the CSV unlock data from the cloned repository.
/// Returns a BTreeMap keyed by unlock date (as a String) with the aggregated
/// token amount (in nano-units).
fn aggregate_unlocks(repo_path: &Path) -> Result<BTreeMap<String, u64>> {
    let mut locked_by_date: BTreeMap<String, u64> = BTreeMap::new();

    for folder in FOLDERS {
        let csv_path = repo_path.join(folder).join("summary.csv");
        println!("Processing file: {:?}", csv_path);

        let mut rdr = Reader::from_path(&csv_path)
            .with_context(|| format!("failed to open CSV file: {csv_path:?}"))?;

        // Iterate over CSV records (header is skipped automatically).
        for result in rdr.records() {
            let record = result?;
            if record.len() < 2 {
                return Err(anyhow::anyhow!("invalid record: {record:?}"));
            }

            let tokens_str = record.get(0).unwrap().trim();
            let unlock_date = record.get(1).unwrap().trim().to_string();

            // Convert token amount to a u64 and then to nano-units.
            let tokens: u64 = tokens_str
                .parse()
                .with_context(|| format!("invalid token amount: {tokens_str}"))?;
            let nanos = tokens * 1000;

            *locked_by_date.entry(unlock_date).or_insert(0) += nanos;
        }
    }
    Ok(locked_by_date)
}

/// Converts a raw unlock date string into ISO 8601 format.
/// It removes a trailing " ([+0-9]+ UTC)" suffix, replaces the first space with
/// "T", and appends "Z".
fn format_date(ts: &str, re: &Regex) -> Result<DateTime<Utc>> {
    let cleaned = re.replace(ts, "");
    let iso = if let Some(space_index) = cleaned.find(' ') {
        let mut s = cleaned.to_string();
        s.replace_range(space_index..=space_index, "T");
        s.push('Z');
        s
    } else {
        format!("{cleaned}Z")
    };

    Ok(DateTime::parse_from_rfc3339(&iso)
        .context(format!("failed to parse timestamp: {iso}",))?
        .with_timezone(&Utc))
}

/// Writes the aggregated unlock data into a CSV file.
fn write_output_csv(output_file: &PathBuf, entries: &[StillLockedEntry]) -> Result<()> {
    let mut wtr = Writer::from_path(output_file).with_context(|| {
        format!(
            "failed to create output CSV file: {}",
            output_file.display()
        )
    })?;
    for entry in entries {
        wtr.serialize(entry)?;
    }
    wtr.flush()?;
    Ok(())
}

fn main() -> Result<()> {
    // Clone the repository containing raw data.
    let tmp_dir = tempdir()?;
    let repo_path = clone_repo(&tmp_dir)?;

    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let output_file = PathBuf::from(crate_dir).join("data").join(OUTPUT_FILE);

    // Aggregate unlock data from CSV files.
    let locked_by_date = aggregate_unlocks(&repo_path)?;

    if locked_by_date.is_empty() {
        println!("No data found – writing empty CSV.");
        write_output_csv(&output_file, &[])?;
        return Ok(());
    }

    // Compute the total locked tokens.
    let total_locked: u64 = locked_by_date.values().sum();

    // Prepare to transform each entry into an output record.
    let re = Regex::new(r" [\+0-9]+ UTC")?;
    let mut cumulative_unlocked = 0;
    let mut output_entries = Vec::new();

    // Process unlock dates in order.
    for (ts, &unlocked) in &locked_by_date {
        cumulative_unlocked += unlocked;
        let still_locked = total_locked - cumulative_unlocked;
        let iso_ts = format_date(ts, &re)?;
        output_entries.push(StillLockedEntry {
            timestamp: iso_ts,
            amount_still_locked: still_locked,
        });
    }

    // Write the aggregated data to a CSV file.
    write_output_csv(&output_file, &output_entries)?;
    println!("Done: {}", output_file.display());

    Ok(())
}
