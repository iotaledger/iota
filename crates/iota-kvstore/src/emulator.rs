// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Test-only lifecycle management for the Google Cloud Bigtable emulator.
//!
//! [`BigTableEmulator::start`] spawns a `cbtemulator` process on a random
//! free port, creates every [`Table`] (with the `iota` column family) in it,
//! and kills the process on drop. Integration tests in this and other crates
//! use it to run against a real Bigtable API without a cloud instance.
//!
//! # Dependencies
//!
//! Three tools from the Google Cloud SDK are involved:
//!
//! - `gcloud`: must be on `PATH`. Only used to locate the SDK root. Install <https://cloud.google.com/sdk/docs/install>
//! - `cbtemulator`: the emulator binary itself. It is shipped with the SDK but
//!   *not* installed on `PATH`, so it is resolved relative to the SDK root
//!   reported by `gcloud` (see [`cbtemulator_path`]). Install with `gcloud
//!   components install bigtable`.
//! - `cbt`: the Bigtable CLI, used to create tables and column families in the
//!   emulator. Must be on `PATH`. Install with `gcloud components install cbt`.

use std::{
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::OnceLock,
};

use anyhow::{Context, Result, bail};
use futures::future::try_join_all;
use iota_bigtable::BigTableClient;
use strum::IntoEnumIterator;
use tokio::process::Command as TokioCommand;

use crate::Table;

pub const INSTANCE_ID: &str = "bigtable_test_instance";
pub const COLUMN_FAMILY: &str = "iota";

/// A self-contained BigTable emulator process that is spawned on a random port.
///
/// # Note
/// The emulator process is killed when this struct is dropped.
pub struct BigTableEmulator {
    child: Child,
    host: String,
}

impl BigTableEmulator {
    /// Spawns a new BigTable emulator as a child process on a random port and
    /// creates the necessary tables.
    pub async fn start() -> Result<Self> {
        require_bigtable_emulator()?;
        let port = get_available_port()?;
        let child = Command::new(cbtemulator_path()?)
            .arg(format!("-port={port}"))
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .context("failed to spawn BigTable emulator")?;

        let host = format!("localhost:{port}");
        let emulator = Self { child, host };
        create_tables(emulator.host(), INSTANCE_ID).await?;
        Ok(emulator)
    }

    /// Returns the host string for the emulator, e.g. `localhost:12345`.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Creates a [`BigTableClient`] connected to this emulator.
    pub fn client(&self) -> Result<BigTableClient> {
        BigTableClient::new_local(&self.host, "testing", INSTANCE_ID, COLUMN_FAMILY)
            .map_err(Into::into)
    }
}

impl Drop for BigTableEmulator {
    fn drop(&mut self) {
        _ = self.child.kill();
        _ = self.child.wait();
    }
}

/// Binds to an ephemeral port and return it.
///
/// The port is moved into `TIME_WAIT` so the OS reserves it briefly, allowing
/// the caller to reuse it with `SO_REUSEADDR`.
fn get_available_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .context("failed to bind to ephemeral port")?;
    let addr = listener
        .local_addr()
        .context("failed to get local address")?;
    _ = std::net::TcpStream::connect(addr).context("failed to connect to ephemeral port")?;
    _ = listener.accept().context("failed to accept connection")?;
    Ok(addr.port())
}

/// Resolves the path to the `cbtemulator` binary.
///
/// `cbtemulator` is not on `PATH`, so it is located at a fixed path under
/// the gcloud SDK root, which is queried from `gcloud` itself. This makes
/// the lookup independent of how the SDK was installed (apt, brew,
/// standalone installer).
///
/// A successful lookup is cached for the lifetime of the process.
pub fn cbtemulator_path() -> Result<&'static Path> {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    if let Some(path) = PATH.get() {
        return Ok(path);
    }

    let output = Command::new("gcloud")
        .args(["info", "--format=value(installation.sdk_root)"])
        .output()
        .context("gcloud not found on PATH — install the Google Cloud SDK to run these tests")?;

    if !output.status.success() {
        bail!("failed to query gcloud sdk root");
    }

    let sdk_root = String::from_utf8(output.stdout)
        .context("non-utf8 gcloud sdk root")?
        .trim()
        .to_string();

    let path = PathBuf::from(sdk_root).join("platform/bigtable-emulator/cbtemulator");

    if !path.exists() {
        bail!("cbtemulator not found at {path:?} — run: gcloud components install bigtable");
    }

    Ok(PATH.get_or_init(|| path))
}

/// Checks that both `cbtemulator` and `cbt` are available on this machine,
/// returning an error naming the missing component otherwise.
///
/// `cbtemulator` is checked via [`cbtemulator_path`]. The `cbt` CLI is probed
/// by spawning `cbt version`, and that outcome is cached for the lifetime of
/// the process.
pub fn require_bigtable_emulator() -> Result<()> {
    static IS_CBT_AVAILABLE: OnceLock<bool> = OnceLock::new();
    cbtemulator_path()?;
    let available = *IS_CBT_AVAILABLE.get_or_init(|| {
        Command::new("cbt")
            .arg("version")
            .output()
            .is_ok_and(|output| output.status.success())
    });
    if !available {
        bail!("cbt not found on PATH — run: gcloud components install cbt");
    }
    Ok(())
}

/// Creates all required BigTable tables in parallel using async subprocesses.
pub async fn create_tables(host: &str, instance_id: &str) -> Result<()> {
    try_join_all(Table::iter().map(|table| async move {
        let output = TokioCommand::new("cbt")
            .args(["-instance", instance_id, "-project", "emulator"])
            .arg("createtable")
            .arg(table.as_ref())
            .env("BIGTABLE_EMULATOR_HOST", host)
            .output()
            .await
            .with_context(|| format!("failed to run cbt createtable {table}"))?;
        if !output.status.success() {
            bail!(
                "cbt createtable {table} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let output = TokioCommand::new("cbt")
            .args(["-instance", instance_id, "-project", "emulator"])
            .args(["createfamily", table.as_ref(), COLUMN_FAMILY])
            .env("BIGTABLE_EMULATOR_HOST", host)
            .output()
            .await
            .with_context(|| format!("failed to run cbt createfamily {table}"))?;
        if !output.status.success() {
            bail!(
                "cbt createfamily {table} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }))
    .await?;
    Ok(())
}
