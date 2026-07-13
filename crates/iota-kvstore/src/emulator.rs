// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Provides a `BigTableEmulator` that manages the emulator lifecycle (spawn,
//! table creation, teardown) for use in integration tests across crates.
//!
//! Requires `gcloud`, `cbt`, and the BigTable emulator on PATH.

use std::{
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::OnceLock,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use futures::future::try_join_all;
use iota_bigtable::BigTableClient;
use strum::IntoEnumIterator;
use tokio::{
    net::TcpStream,
    process::Command as TokioCommand,
    time::{interval, timeout},
};

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
            .context("Failed to spawn BigTable emulator")?;

        let host = format!("localhost:{port}");
        // Construct first so Drop kills the child if the readiness wait fails.
        let mut emulator = Self { child, host };
        emulator.wait_until_ready(port).await?;
        create_tables(emulator.host(), INSTANCE_ID).await?;
        Ok(emulator)
    }

    /// Waits until the emulator accepts TCP connections on `port`.
    ///
    /// Fails if the process exits during startup or the timeout elapses.
    async fn wait_until_ready(&mut self, port: u16) -> Result<()> {
        const TIMEOUT: Duration = Duration::from_secs(10);
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let mut poll = interval(Duration::from_millis(50));
        timeout(TIMEOUT, async {
            loop {
                poll.tick().await;
                if let Some(status) = self.child.try_wait()? {
                    bail!("BigTable emulator exited during startup: {status}");
                }
                if TcpStream::connect(addr).await.is_ok() {
                    return Ok(());
                }
            }
        })
        .await
        .map_err(|_| anyhow!("BigTable emulator not ready on port {port} after {TIMEOUT:?}"))?
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
        .context("Failed to bind to ephemeral port")?;
    let addr = listener
        .local_addr()
        .context("Failed to get local address")?;
    _ = std::net::TcpStream::connect(addr).context("Failed to connect to ephemeral port")?;
    _ = listener.accept().context("Failed to accept connection")?;
    Ok(addr.port())
}

/// Resolve the path to `cbtemulator` relative to the gcloud SDK root.
///
/// Works regardless of whether gcloud was installed via apt, brew, or the
/// standalone installer.
///
/// The lookup shells out to `gcloud`, so a successful result is cached for
/// the lifetime of the process.
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

/// Checks whether the BigTable emulator is available on the local machine.
///
/// The availability probe spawns a `cbt` subprocess, so its outcome is cached
/// for the lifetime of the process.
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
            .with_context(|| format!("Failed to run cbt createtable {table}"))?;
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
            .with_context(|| format!("Failed to run cbt createfamily {table}"))?;
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
