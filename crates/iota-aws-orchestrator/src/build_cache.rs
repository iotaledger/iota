// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, time::Duration};

use iota_build_cache_server::{BuildCacheClient, client::BuildCacheError};

use crate::{
    client::Instance,
    display,
    error::TestbedResult,
    settings::{BuildGroups, Settings},
    ssh::{CommandContext, SshConnectionManager},
};

/// Handles build cache operations for the orchestrator
pub struct BuildCacheService<'a> {
    settings: &'a Settings,
    ssh_manager: &'a SshConnectionManager,
}

impl<'a> BuildCacheService<'a> {
    pub fn new(settings: &'a Settings, ssh_manager: &'a SshConnectionManager) -> Self {
        Self {
            settings,
            ssh_manager,
        }
    }

    /// Update instances using build cache
    pub async fn update_with_build_cache(
        &self,
        commit: &str,
        build_groups: &BuildGroups,
        instances_without_metrics: Vec<Instance>,
        repo_name: String,
    ) -> TestbedResult<()> {
        // Detect CPU targets for all instances
        let cpu_to_instances = self
            .detect_cpu_targets_for_instances(instances_without_metrics)
            .await?;
        if cpu_to_instances.is_empty() {
            display::action("No instances need binaries");
            return Ok(());
        }

        // Take the first available build cache server to resolve the commit
        let cache_server = self
            .settings
            .build_cache
            .as_ref()
            .and_then(|cache| cache.servers.values().next())
            .ok_or_else(|| {
                BuildCacheError::Cache("No build cache servers configured".to_string())
            })?;

        let cache_client = BuildCacheClient::with_credentials(
            cache_server.url.as_str(),
            cache_server.username.clone(),
            cache_server.password.clone(),
        )
        .map_err(|e| BuildCacheError::Cache(format!("Invalid server URL: {e}")))?;

        let resolved_commit = cache_client
            .resolve_commit(commit)
            .await
            .map_err(|e| BuildCacheError::Cache(format!("Failed to resolve commit: {e}")))?;
        if commit != resolved_commit {
            display::action(format!(
                "Requested commit {commit} resolved to actual commit {resolved_commit} by build cache server",
            ));
        }

        // Command needs to run from the repository working directory
        let release_folder = "./target/release";

        // Process each CPU target group
        for (cpu_target, instances) in &cpu_to_instances {
            // Get the build cache config for this CPU target
            let cache_server = self.settings.build_cache_server_for_target(cpu_target)
                .ok_or_else(|| BuildCacheError::Cache(format!(
                    "No build cache server configured for CPU target '{cpu_target}' (needed for {} instances). \
                     Please add a server configuration that includes '{cpu_target}' in its targets list.",
                    instances.len()
                )))?;

            display::action(format!(
                "Updating builds for commit {resolved_commit} (CPU target: {cpu_target}) using server {}",
                cache_server.url
            ));

            let cache_client = BuildCacheClient::with_credentials(
                cache_server.url.as_str(),
                cache_server.username.clone(),
                cache_server.password.clone(),
            )
            .map_err(|e| {
                BuildCacheError::Cache(format!("Invalid server URL for {cpu_target}: {e}"))
            })?;

            // Process each build group separately
            for (group, binary_names) in build_groups {
                let toolchain = group.toolchain.as_deref();
                let features = &group.features;
                let features_opt = if features.is_empty() {
                    None
                } else {
                    Some(features.as_slice())
                };

                display::action(format!(
                    "Processing build group for commit {resolved_commit} (CPU target: {cpu_target}, toolchain: {:?}, features: {:?})",
                    toolchain, features
                ));

                // Check if binaries are available for this CPU target and build group
                let cache_response = cache_client
                    .check_binaries_available(
                        resolved_commit.as_str(),
                        cpu_target,
                        toolchain,
                        features_opt,
                        binary_names,
                    )
                    .await?;

                if !cache_response.available {
                    display::action(format!(
                        "Binaries not in cache for commit {resolved_commit} (CPU target: {cpu_target}, toolchain: {:?}, features: {:?}), requesting build on build cache server",
                        toolchain, features
                    ));

                    // Request build for this CPU target and build group

                    cache_client
                        .request_build(
                            resolved_commit.as_str(),
                            cpu_target,
                            toolchain,
                            features_opt,
                            binary_names,
                        )
                        .await?;

                    // Wait for build to complete
                    display::action(format!(
                        "Waiting for build to complete for commit {resolved_commit} (CPU target: {cpu_target}, toolchain: {:?}, features: {:?}) (this may take up to 45 minutes)",
                        toolchain, features
                    ));

                    let _ = cache_client
                        .wait_for_binaries(
                            resolved_commit.as_str(),
                            cpu_target,
                            toolchain,
                            features_opt,
                            binary_names,
                            Duration::from_secs(45 * 60),
                            Duration::from_secs(5),
                        )
                        .await?;
                }

                // Download and distribute binaries to instances with this CPU target
                display::action(format!(
                    "Distributing cached binaries for commit {resolved_commit} (CPU target: {cpu_target}, toolchain: {:?}, features: {:?}) to {} instances (instances: {})",
                    toolchain,
                    features,
                    instances.len(),
                    instances
                        .iter()
                        .map(|i| i.ssh_address().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));

                for binary in binary_names {
                    // Create download command that fetches from build cache with ETag support
                    // to avoid re-downloading unchanged binaries. We need to hash the existing
                    // binary on the instance to provide the ETag header.
                    // The server will respond with HTTP 304 Not Modified if the binary is
                    // unchanged. Otherwise, it will download the new binary.
                    let binary_path = format!("{release_folder}/{binary}");
                    let auth_header = cache_server
                        .username
                        .as_ref()
                        .and_then(|username| {
                            cache_server
                                .password
                                .as_ref()
                                .map(|password| (username, password))
                        })
                        .map(|(username, password)| format!("-u \"{}:{}\"", username, password))
                        .unwrap_or_default();

                    // Build download URL with optional toolchain and features
                    let mut download_url = format!(
                        "{}/download?commit={resolved_commit}&cpu_target={cpu_target}&binary={binary}",
                        cache_server.url,
                    );
                    if let Some(tc) = toolchain {
                        download_url.push_str(&format!("&toolchain={}", tc));
                    }
                    if !features.is_empty() {
                        download_url.push_str(&format!("&features={}", features.join(",")));
                    }

                    let download_command = format!(
                        r#"set -e && \
mkdir -p {release_folder} && \
if [ -f "{binary_path}" ]; then \
  existing_sha=$(sha256sum "{binary_path}" | cut -d' ' -f1) && \
  etag_header="If-None-Match: \"sha256:$existing_sha\"" && \
  http_code=$(curl -s -w "%{{http_code}}" -H "$etag_header" {auth_header} -L -o "{binary_path}.tmp" '{download_url}') && \
  if [ "$http_code" = "304" ]; then \
    echo "Binary {binary} is up to date (SHA256: $existing_sha)" && \
    rm -f "{binary_path}.tmp"; \
  elif [ "$http_code" = "200" ]; then \
    mv "{binary_path}.tmp" "{binary_path}" && \
    chmod +x "{binary_path}" && \
    echo "Binary {binary} updated"; \
  else \
    echo "ERROR: Download failed for {binary} with HTTP $http_code" >&2 && \
    rm -f "{binary_path}.tmp" && \
    exit 1; \
  fi; \
else \
  echo "Downloading {binary}..." && \
  if curl -f -L {auth_header} -o "{binary_path}" '{download_url}'; then \
    chmod +x "{binary_path}" && \
    echo "Binary {binary} downloaded successfully"; \
  else \
    echo "ERROR: Failed to download {binary}" >&2 && \
    rm -f "{binary_path}" && \
    exit 1; \
  fi; \
fi"#,
                    );

                    display::action(format!(
                        "Downloading {binary} ({cpu_target}) to {} instances",
                        instances.len()
                    ));

                    // we don't need to run the command in the background
                    // because all instances of the same CPU will execute
                    // the command in parallel. That is efficient enough, and we
                    // can panic on errors this way.
                    let context =
                        CommandContext::new().with_execute_from_path(repo_name.clone().into());

                    self.ssh_manager
                        .execute(instances.clone(), download_command, context)
                        .await?;
                }
            }
        }

        display::action(format!(
            "Successfully distributed binaries for commit {resolved_commit} to {} different CPU targets",
            cpu_to_instances.len()
        ));
        Ok(())
    }

    /// Detect CPU target architecture for all instances that need binaries.
    /// Returns a HashMap mapping CPU target to list of instances with that
    /// target
    async fn detect_cpu_targets_for_instances(
        &self,
        instances_needing_binaries: Vec<Instance>,
    ) -> TestbedResult<HashMap<String, Vec<Instance>>> {
        if instances_needing_binaries.is_empty() {
            return Ok(HashMap::new());
        }

        display::action("Detecting architecture for all instances");

        let context = CommandContext::new();
        let command = "uname -m".to_string();

        let arch_results = self
            .ssh_manager
            .execute(instances_needing_binaries.clone(), command, context)
            .await?;

        // Group instances by architecture first
        let mut x86_instances = Vec::new();
        let mut aarch64_instances = Vec::new();

        for (i, (stdout, _stderr)) in arch_results.iter().enumerate() {
            let instance = &instances_needing_binaries[i];
            let arch = stdout.trim();

            match arch {
                "x86_64" => x86_instances.push(instance.clone()),
                "aarch64" => aarch64_instances.push(instance.clone()),
                _ => {
                    return Err(crate::error::TestbedError::BuildCacheError(
                        BuildCacheError::Cache(format!(
                            "Instance {} has unsupported architecture '{}'. Only x86_64 and aarch64 are supported.",
                            instance.ssh_address(),
                            arch
                        )),
                    ));
                }
            }
        }

        let mut cpu_to_instances: HashMap<String, Vec<Instance>> = HashMap::new();

        // Handle x86_64 instances - detect CPU tier from /proc/cpuinfo
        if !x86_instances.is_empty() {
            display::action("Detecting x86_64 CPU targets");

            let context = CommandContext::new();
            let command = "cat /proc/cpuinfo".to_string();

            let results = self
                .ssh_manager
                .execute(x86_instances.clone(), command, context)
                .await?;

            for (i, (stdout, _stderr)) in results.iter().enumerate() {
                let instance = &x86_instances[i];
                let cpu_target = detect_x86_64_tier_from_cpuinfo(stdout.trim());

                cpu_to_instances
                    .entry(cpu_target.to_string())
                    .or_default()
                    .push(instance.clone());
            }
        }

        // Handle aarch64 instances - use rustc to detect native CPU target
        if !aarch64_instances.is_empty() {
            display::action("Detecting aarch64 CPU targets using rustc");

            let context = CommandContext::new();
            let command = "source \"$HOME/.cargo/env\" && rustc --print target-cpus".to_string();

            let results = self
                .ssh_manager
                .execute(aarch64_instances.clone(), command, context)
                .await?;

            for (i, (stdout, _stderr)) in results.iter().enumerate() {
                let instance = &aarch64_instances[i];
                let cpu_target = parse_cpu_target_from_rustc_output(stdout.trim())
                    .unwrap_or_else(|| "generic".to_string()); // fallback to generic

                cpu_to_instances
                    .entry(cpu_target)
                    .or_default()
                    .push(instance.clone());
            }
        }

        // Display detected CPU targets
        for (cpu_target, instances) in &cpu_to_instances {
            display::config(
                format!("CPU target: {cpu_target}"),
                instances
                    .iter()
                    .map(|i| i.ssh_address().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }

        display::done();
        Ok(cpu_to_instances)
    }
}

/// Parse CPU flags from /proc/cpuinfo and determine the x86-64 target tier
fn detect_x86_64_tier_from_cpuinfo(cpuinfo_content: &str) -> &'static str {
    // Find the flags line in /proc/cpuinfo
    let flags_line = cpuinfo_content
        .lines()
        .find(|line| line.starts_with("flags"))
        .unwrap_or("");

    // Extract the flags after the colon
    let flags = if let Some(colon_pos) = flags_line.find(':') {
        flags_line[colon_pos + 1..].trim()
    } else {
        ""
    };

    // Convert to lowercase for case-insensitive matching
    let flags_lower = flags.to_lowercase();
    let flag_set: std::collections::HashSet<&str> = flags_lower.split_whitespace().collect();

    // Check for x86-64-v3 features: AVX, AVX2, BMI1, BMI2, FMA
    let has_v3_features = flag_set.contains("avx")
        && flag_set.contains("avx2")
        && flag_set.contains("bmi1")
        && flag_set.contains("bmi2")
        && flag_set.contains("fma");

    // Check for x86-64-v2 features: SSE3, SSE4.1, SSE4.2, SSSE3
    let has_v2_features = flag_set.contains("sse3")
        && flag_set.contains("sse4_1")
        && flag_set.contains("sse4_2")
        && flag_set.contains("ssse3");

    if has_v3_features {
        "x86-64-v3"
    } else if has_v2_features {
        "x86-64-v2"
    } else {
        // Baseline x86-64 (assume any x86_64 CPU supports this)
        "x86-64"
    }
}

/// Parse CPU target from rustc --print target-cpus output
fn parse_cpu_target_from_rustc_output(output: &str) -> Option<String> {
    // Parse the output to extract the native CPU info
    // Looking for a line like:
    // " native - Select the CPU of the current host (currently apple-m4)."
    for line in output.lines() {
        if line.trim().starts_with("native") && line.contains("(currently") {
            // Extract the CPU name from "(currently cpu-name)"
            if let Some(start) = line.find("(currently ") {
                let cpu_part = &line[start + 11..]; // Skip "(currently "
                if let Some(end) = cpu_part.find(')') {
                    let cpu_name = cpu_part[..end].trim();
                    return Some(cpu_name.to_string());
                }
            }
        }
    }
    None
}
