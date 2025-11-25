// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::Mutex,
};
use tracing::{error, info};

use crate::types::{BuildCacheResponse, BuildJob, BuildRequest, BuildStatus};

/// The build cache that handles git operations and cargo builds
pub struct BuildCache {
    builds: Arc<Mutex<HashMap<String, BuildJob>>>,
    build_mutex: Arc<Mutex<()>>, // Ensures only one build at a time
    // Path of the build results
    cache_dir: PathBuf,
    // Path of the git workspace
    workspace_dir: PathBuf,
    // Git repository URL
    repository_url: String,
}

impl BuildCache {
    /// Create a new build cache
    pub fn new(cache_dir: String, workspace_dir: String, repository_url: String) -> Result<Self> {
        let cache_path = PathBuf::from(cache_dir);
        let workspace_path = PathBuf::from(workspace_dir);

        // Create directories
        fs::create_dir_all(&cache_path)?;
        fs::create_dir_all(&workspace_path)?;

        Ok(Self {
            builds: Arc::new(Mutex::new(HashMap::new())),
            build_mutex: Arc::new(Mutex::new(())),
            cache_dir: cache_path,
            workspace_dir: workspace_path,
            repository_url,
        })
    }

    /// Generate cache key from commit and CPU target only
    fn cache_key(&self, commit: &str, cpu_target: &str) -> String {
        format!("{}:{}", commit, cpu_target)
    }

    /// Get the cache directory for a specific build
    fn get_cache_path(&self, commit: &str, cpu_target: &str) -> PathBuf {
        self.cache_dir.join(format!("{}_{}", commit, cpu_target))
    }

    /// Helper function to check which binaries exist in cache
    fn check_existing_binaries(
        &self,
        commit: &str,
        cpu_target: &str,
        binaries: &[String],
    ) -> (Vec<String>, Vec<String>) {
        let cache_path = self.get_cache_path(commit, cpu_target);
        let mut available = Vec::new();
        let mut missing = Vec::new();

        for binary in binaries {
            let binary_path = cache_path.join(binary);
            if binary_path.exists() {
                available.push(binary.clone());
            } else {
                missing.push(binary.clone());
            }
        }

        (available, missing)
    }

    /// Check if binaries for a commit and CPU target are available
    pub async fn check_binaries(
        &self,
        commit: &str,
        cpu_target: &str,
        binaries: &[String],
    ) -> BuildCacheResponse {
        let (available_binaries, missing_binaries) =
            self.check_existing_binaries(commit, cpu_target, binaries);
        let all_available = missing_binaries.is_empty();

        BuildCacheResponse {
            commit: commit.to_string(),
            cpu_target: cpu_target.to_string(),
            available: all_available,
            binaries: available_binaries,
        }
    }

    /// Get binary data for download
    pub async fn get_binary_data(
        &self,
        commit: &str,
        cpu_target: &str,
        binary_name: &str,
    ) -> Result<Vec<u8>> {
        let cache_path = self.get_cache_path(commit, cpu_target);
        let binary_path = cache_path.join(binary_name);

        // Security: Ensure the resolved path stays within the cache directory
        let canonical_cache_path = cache_path.canonicalize().map_err(|_| {
            anyhow::anyhow!(
                "Invalid cache path for commit {} and CPU target {}",
                commit,
                cpu_target
            )
        })?;

        let canonical_binary_path = binary_path.canonicalize().map_err(|_| {
            anyhow::anyhow!(
                "Binary {} not found for commit {} and CPU target {}",
                binary_name,
                commit,
                cpu_target
            )
        })?;

        if !canonical_binary_path.starts_with(&canonical_cache_path) {
            return Err(anyhow::anyhow!(
                "Invalid binary path for {}. Path traversal not allowed.",
                binary_name
            ));
        }

        if !binary_path.exists() {
            return Err(anyhow::anyhow!(
                "Binary {} not found for commit {} and CPU target {}",
                binary_name,
                commit,
                cpu_target
            ));
        }

        Ok(fs::read(binary_path)?)
    }

    /// Start building binaries for a commit
    pub async fn start_build(&self, request: BuildRequest) -> Result<()> {
        let key = self.cache_key(&request.commit, &request.cpu_target);

        // Check which binaries already exist and which need to be built
        let (available_binaries, missing_binaries) =
            self.check_existing_binaries(&request.commit, &request.cpu_target, &request.binaries);

        if missing_binaries.is_empty() {
            info!("All requested binaries already available for {}", key);
            return Ok(());
        }

        info!(
            "Available binaries: {:?}, Missing binaries: {:?}",
            available_binaries, missing_binaries
        );

        // Try to acquire build lock without blocking - if any build is running, return
        // error
        let build_guard = self.build_mutex.try_lock();
        if build_guard.is_err() {
            return Err(anyhow::anyhow!(
                "Build server is busy - another build is currently in progress"
            ));
        }

        let mut builds = self.builds.lock().await;

        // Check if this specific build already exists and is in progress
        if let Some(existing) = builds.get(&key) {
            match existing.status {
                BuildStatus::Building | BuildStatus::Queued => {
                    return Err(anyhow::anyhow!("Build already in progress for {}", key));
                }
                BuildStatus::Failed(_) => {
                    info!("Previous build failed for {}, starting new build", key);
                }
                BuildStatus::Success => {
                    // This shouldn't happen since we checked binaries above, but handle it
                    // gracefully
                    info!("Build marked as completed for {}", key);
                }
            }
        }

        // Create build job with only the missing binaries
        let job = BuildJob {
            commit: request.commit.clone(),
            cpu_target: request.cpu_target.clone(),
            binaries: missing_binaries.clone(),
            status: BuildStatus::Queued,
            started_at: None,
            completed_at: None,
        };

        builds.insert(key.clone(), job);
        drop(builds);

        // Start build in background while holding the build guard
        let cache = self.clone_for_async();
        let mut build_request = request.clone();
        build_request.binaries = missing_binaries; // Only build what's missing
        tokio::spawn(async move {
            // Acquire the build mutex for the duration of the build
            let _build_guard = cache.build_mutex.lock().await;
            if let Err(e) = cache.perform_build(build_request).await {
                error!("Build failed: {}", e);
            }
            // _build_guard is dropped here, releasing the mutex
        });

        Ok(())
    }

    /// Clone self for async operations (we need to implement Clone)
    fn clone_for_async(&self) -> Self {
        Self {
            builds: Arc::clone(&self.builds),
            build_mutex: Arc::clone(&self.build_mutex),
            cache_dir: self.cache_dir.clone(),
            repository_url: self.repository_url.clone(),
            workspace_dir: self.workspace_dir.clone(),
        }
    }

    /// Perform the actual build
    async fn perform_build(&self, request: BuildRequest) -> Result<()> {
        let key = self.cache_key(&request.commit, &request.cpu_target);
        let cache_path = self.get_cache_path(&request.commit, &request.cpu_target);
        let repo_path = self.workspace_dir.join("repo");

        // Update job status
        {
            let mut builds = self.builds.lock().await;
            if let Some(job) = builds.get_mut(&key) {
                job.status = BuildStatus::Building;
                job.started_at = Some(chrono::Utc::now().to_rfc3339());
            }
        }

        // Clone or update repository
        if let Err(e) = self.setup_repository(&repo_path, &request.commit).await {
            self.mark_build_failed(&key, &format!("Repository setup failed: {}", e))
                .await;
            return Err(e);
        }

        // Build binaries
        if let Err(e) = self
            .build_binaries(
                &repo_path,
                &request.cpu_target,
                &request.binaries,
                &cache_path,
            )
            .await
        {
            self.mark_build_failed(&key, &format!("Build failed: {}", e))
                .await;
            return Err(e);
        }

        // Mark as completed
        {
            let mut builds = self.builds.lock().await;
            if let Some(job) = builds.get_mut(&key) {
                job.status = BuildStatus::Success;
                job.completed_at = Some(chrono::Utc::now().to_rfc3339());
            }
        }

        info!(
            "Build completed successfully for commit {} with CPU target {}",
            request.commit, request.cpu_target
        );
        Ok(())
    }

    /// Setup repository (clone or update to specific commit)
    async fn setup_repository(&self, repo_path: &Path, commit: &str) -> Result<()> {
        if !repo_path.exists() {
            info!("Cloning repository to {:?}", repo_path);
            let output = Command::new("git")
                .args(["clone", &self.repository_url, repo_path.to_str().unwrap()])
                .current_dir(&self.workspace_dir)
                .output()
                .await?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Git clone failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        // Fetch latest changes with all references
        info!("Fetching latest changes");
        let output = Command::new("git")
            .args(["fetch", "origin", "--force"])
            .current_dir(repo_path)
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Git fetch failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Delete any existing build-temp branch
        let _ = Command::new("git")
            .args(["branch", "-D", "build-temp"])
            .current_dir(repo_path)
            .output()
            .await; // Ignore errors if branch doesn't exist

        // Create clean build-temp branch from origin reference
        info!("Creating clean build-temp branch from origin/{}", commit);
        let output = Command::new("git")
            .args([
                "checkout",
                "-b",
                "build-temp",
                &format!("origin/{}", commit),
            ])
            .current_dir(repo_path)
            .output()
            .await?;

        if !output.status.success() {
            // If origin/commit doesn't exist, try direct commit hash
            info!(
                "origin/{} not found, trying direct commit {}",
                commit, commit
            );
            let output = Command::new("git")
                .args(["checkout", "-b", "build-temp", commit])
                .current_dir(repo_path)
                .output()
                .await?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to checkout {} - not found as origin/{} or commit hash: {}",
                    commit,
                    commit,
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        Ok(())
    }

    /// Build the specified binaries
    async fn build_binaries(
        &self,
        repo_path: &Path,
        cpu_target: &str,
        binaries: &[String],
        output_path: &Path,
    ) -> Result<()> {
        // Create output directory
        fs::create_dir_all(output_path)?;

        // Set RUSTFLAGS for CPU target optimization
        let rustflags = format!("-C target-cpu={}", cpu_target);

        info!("Building binaries with RUSTFLAGS: {}", rustflags);

        // Build each binary
        for binary in binaries {
            info!("Building binary: {}", binary);

            let mut child = Command::new("cargo")
                .args(["build", "--release", "--bin", binary])
                .current_dir(repo_path)
                .env("RUSTFLAGS", &rustflags)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            // Stream output for monitoring
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    info!("Build output: {}", line);
                }
            }

            let output = child.wait_with_output().await?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Build failed for {}: {}",
                    binary,
                    String::from_utf8_lossy(&output.stderr)
                ));
            }

            // Copy binary to cache
            let source = repo_path.join("target/release").join(binary);
            let dest = output_path.join(binary);

            if source.exists() {
                fs::copy(&source, &dest)?;
                info!("Cached binary {} to {:?}", binary, dest);
            } else {
                return Err(anyhow::anyhow!(
                    "Built binary {} not found at {:?}",
                    binary,
                    source
                ));
            }
        }

        Ok(())
    }

    /// Mark a build as failed
    async fn mark_build_failed(&self, key: &str, error_msg: &str) {
        let mut builds = self.builds.lock().await;
        if let Some(job) = builds.get_mut(key) {
            job.status = BuildStatus::Failed(error_msg.to_string());
            job.completed_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    /// Get build status
    pub async fn get_build_status(
        &self,
        commit: &str,
        cpu_target: &str,
        requested_binaries: &[String],
    ) -> Option<BuildJob> {
        let key = self.cache_key(commit, cpu_target);
        let builds = self.builds.lock().await;

        if let Some(mut job) = builds.get(&key).cloned() {
            // Dynamically check which binaries are actually available on disk
            let (available_binaries, _missing_binaries) =
                self.check_existing_binaries(commit, cpu_target, requested_binaries);

            // Update the job with the actual available binaries
            job.binaries = available_binaries;

            Some(job)
        } else {
            None
        }
    }
}
