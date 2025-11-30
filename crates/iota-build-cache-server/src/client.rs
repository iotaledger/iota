// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, path::PathBuf, time::Duration};

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
    base_url: String,
    credentials: Option<(String, String)>, // (username, password)
}

impl BuildCacheClient {
    /// Create a new build cache client with basic authentication.
    fn with_auth(
        base_url: &str,
        credentials: Option<(String, String)>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Validate the URL by parsing it
        let url = url::Url::parse(base_url)
            .map_err(|e| format!("Invalid base URL '{}': {}", base_url, e))?;

        // Ensure we have a valid scheme
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(format!(
                "Unsupported scheme '{}', only 'http' and 'https' are supported",
                url.scheme()
            )
            .into());
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            credentials,
        })
    }

    /// Create a new build cache client without authentication.
    pub fn new(base_url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::with_auth(base_url, None)
    }

    /// Create a new build cache client with basic authentication using
    /// username/password.
    pub fn with_credentials(
        base_url: &str,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let credentials = match (username, password) {
            (Some(u), Some(p)) => Some((u, p)),
            _ => None,
        };
        Self::with_auth(base_url, credentials)
    }

    /// Set or update the authentication credentials.
    pub fn set_credentials(&mut self, username: Option<String>, password: Option<String>) {
        self.credentials = match (username, password) {
            (Some(u), Some(p)) => Some((u, p)),
            _ => None,
        };
    }

    /// Add authentication to a request builder if credentials are set.
    fn add_auth(&self, request_builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.credentials {
            Some((username, password)) => request_builder.basic_auth(username, Some(password)),
            None => request_builder,
        }
    }

    pub async fn resolve_commit(&self, commit: &str) -> BuildCacheResult<String> {
        let url = format!("{}/resolve", self.base_url);

        let mut params = HashMap::new();
        params.insert("commit", commit);

        let response = self
            .add_auth(self.client.get(&url).query(&params))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(BuildCacheError::Cache(format!(
                "Failed to resolve commit {commit}: HTTP {}",
                response.status()
            )));
        }

        let resolved_commit: String = response.json().await?;

        Ok(resolved_commit)
    }

    /// Check if binaries for a specific commit are available in the cache.
    pub async fn check_binaries_available(
        &self,
        commit: &str,
        cpu_target: &str,
        toolchain: Option<&str>,
        features: Option<&[String]>,
        binaries: &[String],
    ) -> BuildCacheResult<BuildCacheResponse> {
        let url = format!("{}/check", self.base_url);

        let mut params = HashMap::new();
        params.insert("commit", commit);
        params.insert("cpu_target", cpu_target);

        if let Some(tc) = toolchain {
            params.insert("toolchain", tc);
        }

        let features_str = features.map(|f| f.join(","));
        if let Some(ref feats_str) = features_str {
            if !feats_str.is_empty() {
                params.insert("features", feats_str);
            }
        }

        let binaries_str = binaries.join(",");
        params.insert("binaries", &binaries_str);

        let response = self
            .add_auth(self.client.get(&url).query(&params))
            .send()
            .await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(BuildCacheResponse {
                commit: commit.to_string(),
                cpu_target: cpu_target.to_string(),
                available: false,
                toolchain: toolchain.map(|s| s.to_string()),
                features: features.map(|f| f.to_vec()).unwrap_or_default(),
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
        toolchain: Option<&str>,
        features: Option<&[String]>,
        binary_name: &str,
        local_path: &PathBuf,
    ) -> BuildCacheResult<()> {
        let url = format!("{}/download", self.base_url);

        let mut params = HashMap::new();
        params.insert("commit", commit);
        params.insert("cpu_target", cpu_target);

        if let Some(tc) = toolchain {
            params.insert("toolchain", tc);
        }

        let features_str = features.map(|f| f.join(","));
        if let Some(ref feats_str) = features_str {
            if !feats_str.is_empty() {
                params.insert("features", feats_str);
            }
        }

        params.insert("binary", binary_name);

        let response = self
            .add_auth(self.client.get(&url).query(&params))
            .send()
            .await?;

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
        toolchain: Option<&str>,
        features: Option<&[String]>,
        binaries: &[String],
    ) -> BuildCacheResult<()> {
        let url = format!("{}/build", self.base_url);

        let build_request = BuildRequest {
            commit: commit.to_string(),
            cpu_target: cpu_target.to_string(),
            toolchain: toolchain.map(|s| s.to_string()),
            features: features.map(|f| f.to_vec()).unwrap_or_default(),
            binaries: binaries.to_vec(),
        };

        let response = self
            .add_auth(self.client.post(&url).json(&build_request))
            .send()
            .await?;

        if !response.status().is_success() {
            // Capture status before consuming response
            let status = response.status();

            // Try to get the error message from the response body
            let error_message = match response.text().await {
                Ok(body) => body,
                Err(_) => format!("HTTP {status}"),
            };

            return Err(BuildCacheError::Cache(format!(
                "Build request failed for commit {commit} CPU target {cpu_target}: {error_message}",
            )));
        }

        Ok(())
    }

    /// Wait for binaries to be available in the cache, checking periodically.
    pub async fn wait_for_binaries(
        &self,
        commit: &str,
        cpu_target: &str,
        toolchain: Option<&str>,
        features: Option<&[String]>,
        binaries: &[String],
        timeout: Duration,
        check_interval: Duration,
    ) -> BuildCacheResult<BuildCacheResponse> {
        let start = tokio::time::Instant::now();

        loop {
            let request_start = tokio::time::Instant::now();

            let response = self
                .check_binaries_available(commit, cpu_target, toolchain, features, binaries)
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
