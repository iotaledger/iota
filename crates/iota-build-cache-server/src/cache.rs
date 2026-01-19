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
    // Allowed CPU targets for builds
    allowed_cpu_targets: Vec<String>,
    // Maximum number of commits to keep in cache (for disk space management)
    max_cached_commits: usize,
    // Maximum workspace size in bytes before running cargo clean
    max_workspace_size_bytes: u64,
}

impl BuildCache {
    /// Create a new build cache
    pub fn new(
        cache_dir: String,
        workspace_dir: String,
        repository_url: String,
        allowed_cpu_targets: Vec<String>,
        max_cached_commits: usize,
        max_workspace_size_gb: u64,
    ) -> Result<Self> {
        let cache_path = PathBuf::from(cache_dir);
        let workspace_path = PathBuf::from(workspace_dir);

        // Create directories
        fs::create_dir_all(&cache_path)?;
        fs::create_dir_all(&workspace_path)?;

        // Convert GB to bytes
        let max_workspace_size_bytes = max_workspace_size_gb * 1024 * 1024 * 1024;

        Ok(Self {
            builds: Arc::new(Mutex::new(HashMap::new())),
            build_mutex: Arc::new(Mutex::new(())),
            cache_dir: cache_path,
            workspace_dir: workspace_path,
            repository_url,
            allowed_cpu_targets,
            max_cached_commits,
            max_workspace_size_bytes,
        })
    }

    /// Generate cache key from commit, CPU target, toolchain, and features
    fn cache_key(
        &self,
        commit: &str,
        cpu_target: &str,
        toolchain: Option<&str>,
        features: &[String],
    ) -> String {
        let mut key = format!("{commit}:{cpu_target}");

        // Add toolchain if present and not "stable" (stable is the default)
        if let Some(tc) = toolchain {
            if tc != "stable" {
                key.push_str(&format!(":toolchain={tc}"));
            }
        }

        // Add sorted features if present
        if !features.is_empty() {
            let mut sorted_features = features.to_vec();
            sorted_features.sort();
            key.push_str(&format!(":features={}", sorted_features.join(",")));
        }

        key
    }

    /// Get the cache directory for a specific build
    fn get_cache_path(
        &self,
        commit: &str,
        cpu_target: &str,
        toolchain: Option<&str>,
        features: &[String],
    ) -> PathBuf {
        // Use cache_key to ensure consistency, but replace problematic characters for
        // filesystem
        let key = self
            .cache_key(commit, cpu_target, toolchain, features)
            .replace(':', "_")
            .replace('=', "-")
            .replace(',', "_");
        self.cache_dir.join(key)
    }

    /// Helper to get CPU-specific workspace path
    /// Each CPU target gets its own workspace to avoid target directory
    /// conflicts. Also includes toolchain to avoid conflicts between different
    /// rust versions.
    fn get_workspace_path(&self, cpu_target: &str, toolchain: Option<&str>) -> PathBuf {
        let mut path = cpu_target.to_string();

        // Add toolchain to path if it's not stable (stable is the default)
        if let Some(tc) = toolchain {
            if tc != "stable" {
                path.push_str(&format!("_{tc}"));
            }
        }

        self.workspace_dir.join(path)
    }

    /// Validate CPU target against allowed list
    fn validate_cpu_target(&self, cpu_target: &str) -> Result<()> {
        // Check against allowed list
        if !self.allowed_cpu_targets.contains(&cpu_target.to_string()) {
            return Err(anyhow::anyhow!(
                "CPU target '{}' not allowed. Allowed targets: {}",
                cpu_target,
                self.allowed_cpu_targets.join(", ")
            ));
        }

        Ok(())
    }

    /// Helper function to check which binaries exist in cache
    fn check_existing_binaries(
        &self,
        commit: &str,
        cpu_target: &str,
        toolchain: Option<&str>,
        features: &[String],
        binaries: &[String],
    ) -> (Vec<String>, Vec<String>) {
        let cache_path = self.get_cache_path(commit, cpu_target, toolchain, features);
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
        toolchain: Option<&str>,
        features: &[String],
        binaries: &[String],
    ) -> Result<BuildCacheResponse> {
        // Validate CPU target
        self.validate_cpu_target(cpu_target)?;

        let (available_binaries, missing_binaries) =
            self.check_existing_binaries(commit, cpu_target, toolchain, features, binaries);
        let all_available = missing_binaries.is_empty();

        Ok(BuildCacheResponse {
            commit: commit.to_string(),
            cpu_target: cpu_target.to_string(),
            available: all_available,
            toolchain: toolchain.map(|s| s.to_string()),
            features: features.to_vec(),
            binaries: available_binaries,
        })
    }

    /// Get binary file metadata (path, size, sha256) for streaming downloads
    pub async fn get_binary_info(
        &self,
        commit: &str,
        cpu_target: &str,
        toolchain: Option<&str>,
        features: &[String],
        binary_name: &str,
    ) -> Result<(std::path::PathBuf, u64, String)> {
        // Validate CPU target
        self.validate_cpu_target(cpu_target)?;

        let binary_path =
            self.get_binary_path(commit, cpu_target, toolchain, features, binary_name)?;
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
        toolchain: Option<&str>,
        features: &[String],
        binary_name: &str,
    ) -> Result<std::path::PathBuf> {
        let cache_path = self.get_cache_path(commit, cpu_target, toolchain, features);
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
        // Use a dedicated workspace for commit resolution to avoid conflicts with
        // builds
        let resolve_workspace = self.get_workspace_path("resolve", None);
        let resolved_commit = self
            .setup_repository(&resolve_workspace, commit_ref)
            .await?;
        Ok(resolved_commit)
    }

    /// Start building binaries for a commit (resolves branches/tags to commit
    /// hash)
    pub async fn start_build(
        &self,
        commit: &str,
        cpu_target: &str,
        toolchain: Option<&str>,
        features: &[String],
        binaries: &[String],
    ) -> Result<BuildResponse> {
        // Validate CPU target
        self.validate_cpu_target(cpu_target)?;

        let key = self.cache_key(commit, cpu_target, toolchain, features);

        // Check which binaries already exist and which need to be built
        let (available_binaries, missing_binaries) =
            self.check_existing_binaries(commit, cpu_target, toolchain, features, binaries);

        if missing_binaries.is_empty() {
            info!("All requested binaries already available for {key}");
            return Ok(BuildResponse {
                resolved_commit: commit.to_string(),
                cpu_target: cpu_target.to_string(),
                toolchain: toolchain.map(|s| s.to_string()),
                features: features.to_vec(),
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
            toolchain: toolchain.map(|s| s.to_string()),
            features: features.to_vec(),
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
        let toolchain_clone = toolchain.map(|s| s.to_string());
        let features_clone = features.to_vec();
        let missing_binaries_clone = missing_binaries.clone();

        tokio::spawn(async move {
            // Acquire the build mutex for the duration of the build
            let _build_guard = cache.build_mutex.lock().await;

            // Only build what's missing
            if let Err(e) = cache
                .perform_build(
                    &commit_clone,
                    &cpu_target_clone,
                    toolchain_clone.as_deref(),
                    &features_clone,
                    &missing_binaries_clone,
                )
                .await
            {
                error!("Build failed: {e}");
            }
            // _build_guard is dropped here, releasing the mutex
        });

        Ok(BuildResponse {
            resolved_commit: commit.to_string(),
            cpu_target: cpu_target.to_string(),
            toolchain: toolchain.map(|s| s.to_string()),
            features: features.to_vec(),
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
            allowed_cpu_targets: self.allowed_cpu_targets.clone(),
            max_cached_commits: self.max_cached_commits,
            max_workspace_size_bytes: self.max_workspace_size_bytes,
        }
    }

    /// Perform the actual build
    async fn perform_build(
        &self,
        commit: &str,
        cpu_target: &str,
        toolchain: Option<&str>,
        features: &[String],
        binaries: &[String],
    ) -> Result<()> {
        // Use CPU-specific workspace to avoid target directory conflicts
        // Also use toolchain-specific workspace to avoid artifact conflicts
        let repo_path = self.get_workspace_path(cpu_target, toolchain);

        // First setup repository and resolve commit to actual SHA
        let resolved_commit = self
            .setup_repository(&repo_path, commit)
            .await
            .map_err(|e| anyhow::anyhow!("Repository setup failed: {e}"))?;

        let key = self.cache_key(&resolved_commit, cpu_target, toolchain, features);
        let cache_path = self.get_cache_path(&resolved_commit, cpu_target, toolchain, features);

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
            .build_binaries(
                &repo_path,
                cpu_target,
                toolchain,
                features,
                binaries,
                &cache_path,
            )
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

        // Perform cache cleanup after successful build
        if let Err(e) = self.cleanup_old_cache_entries().await {
            error!("Cache cleanup failed: {e}");
            // Don't fail the build if cleanup fails
        }

        // Perform workspace target cleanup if needed
        match self.cleanup_workspace_targets().await {
            Ok(cleaned) if !cleaned.is_empty() => {
                info!("Cleaned workspace targets: {}", cleaned.join(", "));
            }
            Ok(_) => {
                info!("No workspace targets needed cleaning");
            }
            Err(e) => {
                error!("Workspace cleanup failed: {e}");
                // Don't fail the build if cleanup fails
            }
        }

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
        toolchain: Option<&str>,
        features: &[String],
        binaries: &[String],
        output_path: &Path,
    ) -> Result<()> {
        // Create output directory
        fs::create_dir_all(output_path)?;

        // Set RUSTFLAGS for CPU target optimization
        let rustflags = format!("-C target-cpu={cpu_target}");

        info!("Building binaries with RUSTFLAGS: {rustflags}");

        let toolchain_arg = if let Some(tc) = toolchain {
            if tc != "stable" {
                info!("Using toolchain: {tc}");
                Some(format!("+{tc}"))
            } else {
                None
            }
        } else {
            None
        };

        let features_arg = if !features.is_empty() {
            let features_str = features.join(",");
            info!("Building with features: \"{features_str}\"");
            Some(format!("--features={features_str}"))
        } else {
            None
        };

        // Build each binary
        for binary in binaries {
            info!("Building binary: {binary}");

            // Build arguments
            let mut args = vec!["build", "--release", "--bin", binary];

            // Add toolchain if specified
            if let Some(ref t) = toolchain_arg {
                args.insert(0, t);
            }

            // Add features if specified
            if let Some(ref f) = features_arg {
                args.push(f);
            }

            // print the full command for logging
            info!(
                "Running build command: \"cargo {}\" in \"{}\" with flags: \"{rustflags}\"",
                args.join(" "),
                repo_path.display()
            );

            let mut child = Command::new("cargo")
                .args(&args)
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

    /// Calculate the size of a directory in bytes
    fn calculate_directory_size(path: &Path) -> Result<u64> {
        let mut total_size = 0;

        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let metadata = entry.metadata()?;

                if metadata.is_file() {
                    total_size += metadata.len();
                } else if metadata.is_dir() {
                    total_size += Self::calculate_directory_size(&entry.path())?;
                }
            }
        }

        Ok(total_size)
    }

    /// Run cargo clean in the specified workspace (all subdirectories)
    async fn run_cargo_clean(&self, workspace_path: &Path) -> Result<()> {
        info!(
            "Running cargo clean in workspace: {}",
            workspace_path.display()
        );
        let output = Command::new("cargo")
            .args(["clean"])
            .current_dir(workspace_path)
            .output()
            .await?;

        if output.status.success() {
            info!(
                "Successfully ran cargo clean in workspace: {}",
                workspace_path.display()
            );
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to run cargo clean in {}: {}",
                workspace_path.display(),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Clean up workspace target directories if they exceed the size limit
    pub async fn cleanup_workspace_targets(&self) -> Result<Vec<String>> {
        let mut cleaned_workspaces = Vec::new();

        if !self.workspace_dir.exists() {
            return Ok(cleaned_workspaces);
        }

        match Self::calculate_directory_size(&self.workspace_dir) {
            Ok(size) if size > self.max_workspace_size_bytes => {
                info!(
                    "Workspace {} target directory is {} GB, running cargo clean",
                    self.workspace_dir.display(),
                    size / (1024 * 1024 * 1024)
                );

                // Clean each CPU-specific workspace
                for entry in fs::read_dir(&self.workspace_dir)? {
                    let entry = entry?;
                    if !entry.file_type()?.is_dir() {
                        continue;
                    }

                    let workspace_path = entry.path();

                    match self.run_cargo_clean(&workspace_path).await {
                        Ok(()) => {
                            let workspace_name = workspace_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            cleaned_workspaces.push(workspace_name);
                        }
                        Err(e) => {
                            error!(
                                "Failed to clean workspace {}: {}",
                                workspace_path.display(),
                                e
                            );
                        }
                    }
                }
            }
            Ok(_size) => {}
            Err(e) => {
                error!(
                    "Failed to calculate size for {}: {}",
                    self.workspace_dir.display(),
                    e
                );
            }
        }

        Ok(cleaned_workspaces)
    }

    /// Clean up old cache entries, keeping only the most recent commits
    pub async fn cleanup_old_cache_entries(&self) -> Result<()> {
        let cache_dir = &self.cache_dir;

        // Read all directories in cache_dir
        let mut commit_dirs = Vec::new();
        let entries = fs::read_dir(cache_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        commit_dirs.push((path, modified));
                    }
                }
            }
        }

        // Sort by modification time (newest first)
        commit_dirs.sort_by(|a, b| b.1.cmp(&a.1));

        // Remove directories beyond the limit
        if commit_dirs.len() > self.max_cached_commits {
            let dirs_to_remove = &commit_dirs[self.max_cached_commits..];
            let count = dirs_to_remove.len();

            for (dir_path, _) in dirs_to_remove {
                info!("Removing old cache directory: {dir_path:?}");
                if let Err(e) = fs::remove_dir_all(dir_path) {
                    error!("Failed to remove cache directory {dir_path:?}: {e}");
                }
            }

            info!(
                "Cache cleanup completed. Kept {} directories, removed {} directories.",
                self.max_cached_commits, count
            );
        };

        Ok(())
    }

    /// Get build status
    pub async fn get_build_status(
        &self,
        commit: &str,
        cpu_target: &str,
        toolchain: Option<&str>,
        features: &[String],
        requested_binaries: &[String],
    ) -> Result<Option<BuildJob>> {
        // Validate CPU target
        self.validate_cpu_target(cpu_target)?;

        let key = self.cache_key(commit, cpu_target, toolchain, features);
        let builds = self.builds.lock().await;

        if let Some(mut job) = builds.get(&key).cloned() {
            // Dynamically check which binaries are actually available on disk
            let (available_binaries, _missing_binaries) = self.check_existing_binaries(
                commit,
                cpu_target,
                toolchain,
                features,
                requested_binaries,
            );

            // Update the job with the actual available binaries
            job.binaries = available_binaries;

            Ok(Some(job))
        } else {
            Ok(None)
        }
    }
}
