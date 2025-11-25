// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, net::SocketAddr, path::PathBuf, time::Duration};

use reqwest::{Client, StatusCode};
use tokio::{fs, io::AsyncWriteExt};

use crate::types::{BuildCacheResponse, BuildRequest};

/// Error type for build cache client operations
#[derive(Debug, thiserror::Error)]
pub enum BuildCacheError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Cache operation failed: {0}")]
    Cache(String),
    #[error("Timeout waiting for binaries to be available")]
    Timeout,
}

pub type BuildCacheResult<T> = Result<T, BuildCacheError>;

/// The build cache client that communicates with the build cache server.
pub struct BuildCacheClient {
    client: Client,
    build_instance_ip: String,
    port: u16,
}

impl BuildCacheClient {
    /// Create a new build cache client.
    pub fn new(server_address: &str) -> Result<Self, std::net::AddrParseError> {
        let socket_addr: SocketAddr = server_address.parse()?;

        let build_instance_ip = socket_addr.ip().to_string();
        let port = socket_addr.port();

        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");

        Ok(Self {
            client,
            build_instance_ip,
            port,
        })
    }

    /// Check if binaries for a specific commit are available in the cache.
    pub async fn check_binaries_available(
        &self,
        commit: &str,
        cpu_target: &str,
        binaries: &[String],
    ) -> BuildCacheResult<BuildCacheResponse> {
        let url = format!(
            "http://{}:{}/check/{}/{}",
            self.build_instance_ip, self.port, commit, cpu_target
        );

        let mut params = HashMap::new();
        params.insert("binaries", binaries.join(","));

        let response = self.client.get(&url).query(&params).send().await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(BuildCacheResponse {
                commit: commit.to_string(),
                cpu_target: cpu_target.to_string(),
                available: false,
                binaries: vec![],
            });
        }

        response.json().await.map_err(BuildCacheError::Http)
    }

    /// Download a binary from the build cache to a local path.
    pub async fn download_binary(
        &self,
        commit: &str,
        cpu_target: &str,
        binary_name: &str,
        local_path: &PathBuf,
    ) -> BuildCacheResult<()> {
        let url = format!(
            "http://{}:{}/download/{}/{}/{}",
            self.build_instance_ip, self.port, commit, cpu_target, binary_name
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(BuildCacheError::Cache(format!(
                "Failed to download binary: HTTP {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::File::create(local_path).await?;
        file.write_all(&bytes).await?;

        // Make binary executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = file.metadata().await?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(local_path, permissions).await?;
        }

        Ok(())
    }

    /// Request the build instance to build binaries for a specific commit.
    pub async fn request_build(
        &self,
        commit: &str,
        cpu_target: &str,
        binaries: &[String],
    ) -> BuildCacheResult<()> {
        let url = format!("http://{}:{}/build", self.build_instance_ip, self.port);

        let build_request = BuildRequest {
            commit: commit.to_string(),
            cpu_target: cpu_target.to_string(),
            binaries: binaries.to_vec(),
        };

        let response = self.client.post(&url).json(&build_request).send().await?;

        if !response.status().is_success() {
            // Capture status before consuming response
            let status = response.status();

            // Try to get the error message from the response body
            let error_message = match response.text().await {
                Ok(body) => body,
                Err(_) => format!("HTTP {}", status),
            };

            return Err(BuildCacheError::Cache(format!(
                "Build request failed for commit {} CPU target {}: {}",
                commit, cpu_target, error_message
            )));
        }

        Ok(())
    }

    /// Wait for binaries to be available in the cache, checking periodically.
    pub async fn wait_for_binaries(
        &self,
        commit: &str,
        cpu_target: &str,
        binaries: &[String],
        timeout: Duration,
        check_interval: Duration,
    ) -> BuildCacheResult<BuildCacheResponse> {
        let start = tokio::time::Instant::now();

        loop {
            let request_start = tokio::time::Instant::now();

            let response = self
                .check_binaries_available(commit, cpu_target, binaries)
                .await?;

            if response.available {
                return Ok(response);
            }

            if start.elapsed() >= timeout {
                return Err(BuildCacheError::Timeout);
            }

            // Calculate remaining sleep time after accounting for request duration
            if let Some(remaining_sleep) = check_interval.checked_sub(request_start.elapsed()) {
                tokio::time::sleep(remaining_sleep).await;
            }
        }
    }
}
