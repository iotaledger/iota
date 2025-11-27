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
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::Mutex,
};
use tracing::{error, info};

use crate::types::{BuildCacheResponse, BuildJob, BuildResponse, BuildStatus};

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
        format!("{commit}:{cpu_target}")
    }

    /// Get the cache directory for a specific build
    fn get_cache_path(&self, commit: &str, cpu_target: &str) -> PathBuf {
        self.cache_dir.join(format!("{commit}_{cpu_target}"))
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

    /// Get binary file metadata (path, size, sha256) for streaming downloads
    pub async fn get_binary_info(
        &self,
        commit: &str,
        cpu_target: &str,
        binary_name: &str,
    ) -> Result<(std::path::PathBuf, u64, String)> {
        let binary_path = self.get_binary_path(commit, cpu_target, binary_name)?;
        let metadata = fs::metadata(&binary_path)?;

        // Read SHA256 from checksum file
        let checksum_file = binary_path.with_extension("sha256");
        let sha256_hash = match fs::read_to_string(&checksum_file) {
            Ok(hash) => hash.trim().to_string(),
            Err(_) => {
                // If checksum file doesn't exist, calculate it on the fly
                let hash = Self::calculate_sha256(&binary_path)?;
                // Save it for future use
                let _ = fs::write(&checksum_file, &hash);
                hash
            }
        };

        Ok((binary_path, metadata.len(), sha256_hash))
    }

    /// Helper to get and validate binary path
    fn get_binary_path(
        &self,
        commit: &str,
        cpu_target: &str,
        binary_name: &str,
    ) -> Result<std::path::PathBuf> {
        let cache_path = self.get_cache_path(commit, cpu_target);
        let binary_path = cache_path.join(binary_name);

        // Security: Ensure the resolved path stays within the cache directory
        let canonical_cache_path = cache_path.canonicalize().map_err(|_| {
            anyhow::anyhow!("Invalid cache path for commit {commit} and CPU target {cpu_target}",)
        })?;

        let canonical_binary_path = binary_path.canonicalize().map_err(|_| {
            anyhow::anyhow!(
                "Binary {binary_name} not found for commit {commit} and CPU target {cpu_target}",
            )
        })?;

        if !canonical_binary_path.starts_with(&canonical_cache_path) {
            return Err(anyhow::anyhow!(
                "Invalid binary path for {binary_name}. Path traversal not allowed.",
            ));
        }

        if !canonical_binary_path.exists() {
            return Err(anyhow::anyhow!(
                "Binary {binary_name} not found for commit {commit} and CPU target {cpu_target}",
            ));
        }

        Ok(canonical_binary_path)
    }

    /// Resolve a branch/tag/commit to an actual commit hash
    pub async fn resolve_commit(&self, commit_ref: &str) -> Result<String> {
        // First setup the repository to ensure we have the latest refs
        let resolved_commit = self
            .setup_repository(&self.workspace_dir, commit_ref)
            .await?;
        Ok(resolved_commit)
    }

    /// Start building binaries for a commit (resolves branches/tags to commit
    /// hash)
    pub async fn start_build(
        &self,
        commit: &str,
        cpu_target: &str,
        binaries: &[String],
    ) -> Result<BuildResponse> {
        let key = self.cache_key(commit, cpu_target);

        // Check which binaries already exist and which need to be built
        let (available_binaries, missing_binaries) =
            self.check_existing_binaries(commit, cpu_target, binaries);

        if missing_binaries.is_empty() {
            info!("All requested binaries already available for {key}");
            return Ok(BuildResponse {
                resolved_commit: commit.to_string(),
                cpu_target: cpu_target.to_string(),
                binaries: available_binaries,
                message: "All binaries already available".to_string(),
            });
        }

        info!("Available binaries: {available_binaries:?}, Missing binaries: {missing_binaries:?}",);

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
                    return Err(anyhow::anyhow!("Build already in progress for {key}"));
                }
                BuildStatus::Failed(_) => {
                    info!("Previous build failed for {key}, starting new build");
                }
                BuildStatus::Success => {
                    // This shouldn't happen since we checked binaries above, but handle it
                    // gracefully
                    info!("Build marked as completed for {key}");
                }
            }
        }

        // Create build job with only the missing binaries
        let job = BuildJob {
            commit: commit.to_string(),
            cpu_target: cpu_target.to_string(),
            binaries: missing_binaries.clone(),
            status: BuildStatus::Queued,
            started_at: None,
            completed_at: None,
        };

        builds.insert(key.clone(), job);
        drop(builds);

        // Start build in background while holding the build guard
        let cache = self.clone_for_async();

        let commit_clone = commit.to_string();
        let cpu_target_clone = cpu_target.to_string();
        let missing_binaries_clone = missing_binaries.clone();

        tokio::spawn(async move {
            // Acquire the build mutex for the duration of the build
            let _build_guard = cache.build_mutex.lock().await;

            // Only build what's missing
            if let Err(e) = cache
                .perform_build(&commit_clone, &cpu_target_clone, &missing_binaries_clone)
                .await
            {
                error!("Build failed: {e}");
            }
            // _build_guard is dropped here, releasing the mutex
        });

        Ok(BuildResponse {
            resolved_commit: commit.to_string(),
            cpu_target: cpu_target.to_string(),
            binaries: missing_binaries,
            message: "Build started".to_string(),
        })
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
    async fn perform_build(
        &self,
        commit: &str,
        cpu_target: &str,
        binaries: &[String],
    ) -> Result<()> {
        let repo_path = &self.workspace_dir;

        // First setup repository and resolve commit to actual SHA
        let resolved_commit = self
            .setup_repository(repo_path, commit)
            .await
            .map_err(|e| anyhow::anyhow!("Repository setup failed: {e}"))?;

        let key = self.cache_key(&resolved_commit, cpu_target);
        let cache_path = self.get_cache_path(&resolved_commit, cpu_target);

        // Update job status
        {
            let mut builds = self.builds.lock().await;
            if let Some(job) = builds.get_mut(&key) {
                job.status = BuildStatus::Building;
                job.started_at = Some(chrono::Utc::now().to_rfc3339());
            }
        }

        // Repository was already set up above and commit SHA was resolved

        // Build binaries
        if let Err(e) = self
            .build_binaries(repo_path, cpu_target, binaries, &cache_path)
            .await
        {
            self.mark_build_failed(&key, &format!("Build failed: {e}"))
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
            "Build completed successfully for commit {resolved_commit} with CPU target {cpu_target}"
        );
        Ok(())
    }

    /// Resolve commit reference to SHA using local git repository
    async fn resolve_commit_locally(&self, repo_path: &Path, commit_ref: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", commit_ref])
            .current_dir(repo_path)
            .output()
            .await?;

        if output.status.success() {
            let resolved_commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if resolved_commit.len() >= 7 && resolved_commit.chars().all(|c| c.is_ascii_hexdigit())
            {
                if commit_ref != resolved_commit {
                    info!("Resolved '{}' to commit '{}'", commit_ref, resolved_commit);
                }
                return Ok(resolved_commit);
            }
        }

        Err(anyhow::anyhow!(
            "Could not resolve {commit_ref} to commit SHA: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }

    /// Setup repository (clone or update to specific commit)
    async fn setup_repository(&self, repo_path: &Path, commit: &str) -> Result<String> {
        if !repo_path.exists() || !repo_path.join(".git").exists() {
            info!("Cloning repository to {repo_path:?}");

            // Create parent directory if it doesn't exist
            if let Some(parent) = repo_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Remove existing directory if it exists but is not a git repo
            if repo_path.exists() {
                std::fs::remove_dir_all(repo_path)?;
            }

            let output = Command::new("git")
                .args(["clone", &self.repository_url, repo_path.to_str().unwrap()])
                .output()
                .await?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Git clone failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        } else {
            info!("Repository already exists at {repo_path:?}, using existing repo");

            // Verify it's actually a git repository
            let output = Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(repo_path)
                .output()
                .await?;

            if !output.status.success() {
                info!(
                    "Existing directory is not a valid git repository, removing and cloning fresh"
                );
                std::fs::remove_dir_all(repo_path)?;

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
        }

        // Clean any uncommitted changes first
        info!("Cleaning working directory");
        let _ = Command::new("git")
            .args(["reset", "--hard", "HEAD"])
            .current_dir(repo_path)
            .output()
            .await;
        let _ = Command::new("git")
            .args(["clean", "-fd"])
            .current_dir(repo_path)
            .output()
            .await;

        // If we are already at the desired commit, return early
        let current_commit = self.resolve_commit_locally(repo_path, "HEAD").await?;
        if current_commit == commit {
            return Ok(current_commit);
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
            .args(["checkout", "develop"])
            .current_dir(repo_path)
            .output()
            .await;
        let _ = Command::new("git")
            .args(["branch", "-D", "build-temp"])
            .current_dir(repo_path)
            .output()
            .await; // Ignore errors if branch doesn't exist

        // Create clean build-temp branch from origin reference
        info!("Creating clean build-temp branch from origin/{commit}");
        let output = Command::new("git")
            .args(["checkout", "-b", "build-temp", &format!("origin/{commit}")])
            .current_dir(repo_path)
            .output()
            .await?;

        if !output.status.success() {
            // If origin/commit doesn't exist, try direct commit hash
            info!("origin/{commit} not found, trying direct commit {commit}");
            let output = Command::new("git")
                .args(["checkout", "-b", "build-temp", commit])
                .current_dir(repo_path)
                .output()
                .await?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to checkout {commit} - not found as origin/{commit} or commit hash: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        // Resolve the final commit SHA after all git operations
        let resolved_commit = self.resolve_commit_locally(repo_path, "HEAD").await?;
        Ok(resolved_commit)
    }

    /// Calculate SHA256 hash of a file
    fn calculate_sha256(file_path: &Path) -> Result<String> {
        let mut file = fs::File::open(file_path)?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
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
        let rustflags = format!("-C target-cpu={cpu_target}");

        info!("Building binaries with RUSTFLAGS: {rustflags}");

        // Build each binary
        for binary in binaries {
            info!("Building binary: {binary}");

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
                    info!("Build output: {line}");
                }
            }

            let output = child.wait_with_output().await?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Build failed for {binary}: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }

            // Copy binary to cache
            let source = repo_path.join("target/release").join(binary);
            let dest = output_path.join(binary);

            if source.exists() {
                // Calculate and save SHA256 checksum
                let sha256_hash = Self::calculate_sha256(&source)?;
                let checksum_file = dest.with_extension("sha256");
                fs::write(&checksum_file, sha256_hash)?;
                info!("Saved SHA256 checksum for {binary} to {checksum_file:?}");

                fs::copy(&source, &dest)?;
                info!("Cached binary {binary} to {dest:?}");
            } else {
                return Err(anyhow::anyhow!(
                    "Built binary {binary} not found at {source:?}",
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
