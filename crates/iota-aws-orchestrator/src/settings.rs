// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    env,
    fmt::Display,
    fs::{self},
    hash::Hash,
    path::{Path, PathBuf},
};

use reqwest::Url;
use serde::{Deserialize, Deserializer, de::Error};

use crate::error::{SettingsError, SettingsResult};

/// Helper function to filter out empty strings and join them with a separator.
pub(crate) fn join_non_empty_strings<S: AsRef<str>>(items: &[S], separator: &str) -> String {
    items
        .iter()
        .map(|s| s.as_ref())
        .filter(|f| !f.is_empty())
        .collect::<Vec<_>>()
        .join(separator)
}

pub fn build_cargo_command<S1: AsRef<str>, S2: AsRef<str>, S3: AsRef<str>>(
    subcommand: &str,
    toolchain: Option<String>,
    features: Vec<String>,
    binaries: &[S1],
    setup_commands: &[S2],
    additional_args: &[S3],
) -> String {
    let toolchain_arg = toolchain
        .as_ref()
        .filter(|t| t.as_str() != "stable")
        .map(|t| format!("+{t}"))
        .unwrap_or_default();

    let target_dir_arg = toolchain
        .filter(|t| t != "stable")
        .map(|t| format!("--target-dir target_{t}"))
        .unwrap_or_default();

    let features_arg = if features.is_empty() {
        "".to_string()
    } else {
        format!("--features \"{}\"", features.join(" "))
    };

    let binaries_args: Vec<String> = binaries
        .iter()
        .map(|name| format!("--bin {}", name.as_ref()))
        .collect();
    let binaries_args = join_non_empty_strings(&binaries_args, " ");

    let additional_args_str = join_non_empty_strings(additional_args, " ");

    let mut cargo_args = vec![
        "cargo",
        &toolchain_arg,
        subcommand,
        &target_dir_arg,
        "--release",
        &binaries_args,
        &features_arg,
    ];

    if !additional_args_str.is_empty() {
        cargo_args.extend(&["--", &additional_args_str]);
    }

    let cargo_command = join_non_empty_strings(&cargo_args, " ");

    let default_setup = [
        "source \"$HOME/.cargo/env\"",
        "export RUSTFLAGS='-C target-cpu=native'",
    ];

    let all_commands: Vec<String> = default_setup
        .iter()
        .map(|s| s.to_string())
        .chain(setup_commands.iter().map(|s| s.as_ref().to_string()))
        .chain(std::iter::once(cargo_command))
        .collect();
    join_non_empty_strings(&all_commands, " && ")
}

/// The git repository holding the codebase.
#[derive(Deserialize, Clone)]
pub struct Repository {
    /// The url of the repository.
    #[serde(deserialize_with = "parse_url")]
    pub url: Url,
    /// The commit (or branch name) to deploy.
    pub commit: String,
}

fn parse_url<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    let url = Url::parse(s).map_err(D::Error::custom)?;

    match url.path_segments().map(|x| x.count() >= 2) {
        None | Some(false) => Err(D::Error::custom(SettingsError::MalformedRepositoryUrl(url))),
        _ => Ok(url),
    }
}

/// The list of supported cloud providers.
#[derive(Deserialize, Clone)]
pub enum CloudProvider {
    #[serde(alias = "aws")]
    Aws,
}

/// Configuration for a build cache server that supports multiple CPU targets.
#[derive(Deserialize, Clone)]
pub struct BuildCacheServer {
    /// List of CPU targets this server supports (e.g., ["x86-64", "x86-64-v2",
    /// "x86-64-v3"]).
    pub targets: Vec<String>,
    /// The base URL of the build cache server (e.g., "http://192.168.1.100:8080").
    pub url: String,
    /// Optional username for basic authentication.
    #[serde(default)]
    pub username: Option<String>,
    /// Optional password for basic authentication.
    #[serde(default)]
    pub password: Option<String>,
}

/// Configuration for the optional build cache.
#[derive(Deserialize, Clone)]
pub struct BuildCache {
    /// Whether to enable the build cache.
    pub enabled: bool,
    /// Named build cache server configurations.
    pub servers: HashMap<String, BuildCacheServer>,
}

#[derive(Deserialize, Clone)]
pub struct BinaryBuildConfig {
    /// Select rust toolchain to build and run the binary.
    #[serde(default)]
    pub toolchain: Option<String>,
    /// Additional features to enable when building the binary.
    #[serde(default)]
    pub features: Vec<String>,
}

/// The testbed settings. Those are topically specified in a file.
#[derive(Deserialize, Clone)]
pub struct Settings {
    /// The testbed unique id. This allows multiple users to run concurrent
    /// testbeds on the same cloud provider's account without interference
    /// with each others.
    pub testbed_id: String,
    /// The cloud provider hosting the testbed.
    pub cloud_provider: CloudProvider,
    /// The path to the secret token for authentication with the cloud provider.
    pub token_file: PathBuf,
    /// The ssh private key to access the instances.
    pub ssh_private_key_file: PathBuf,
    /// The corresponding ssh public key registered on the instances. If not
    /// specified. the public key defaults the same path as the private key
    /// with an added extension 'pub'.
    pub ssh_public_key_file: Option<PathBuf>,
    /// The list of cloud provider regions to deploy on the testbed. If the
    /// metrics server is used, it will be located in the first region in
    /// the list.
    pub regions: Vec<String>,
    /// The specs of the instances to deploy for nodes. Those are dependent
    /// on the cloud provider, e.g., specifying 't3.medium' creates
    /// instances with 2 vCPU and 4GB of ram on AWS.
    pub node_specs: String,
    /// The list of cloud provider regions to deploy for clients on the
    /// testbed.
    pub client_specs: String,
    /// Region to deploy the metrics instance.
    pub metrics_specs: String,
    /// Optional build cache configuration.
    pub build_cache: Option<BuildCache>,
    /// The details of the git repository to deploy.
    pub repository: Repository,
    /// The working directory on the remote instance (containing all
    /// configuration files).
    #[serde(default = "default_working_dir")]
    pub working_dir: PathBuf,
    /// Pass '--use-fullnode-for-execution' and '--fullnode-rpc-addresses' to
    /// stress binary.
    #[serde(default)]
    pub use_fullnode_for_execution: bool,
    /// The directory (on the local machine) where to save benchmarks
    /// results.
    #[serde(default = "default_results_dir")]
    pub results_dir: PathBuf,
    /// Binary build configuration.
    #[serde(default)]
    pub build_configs: HashMap<String, BinaryBuildConfig>,
    /// Enable flamegraphs when running nodes.
    #[serde(default)]
    pub enable_flamegraph: bool,
    /// Enable prometheus snapshots.
    #[serde(default)]
    pub enable_prometheus_snapshots: bool,
}

fn default_working_dir() -> PathBuf {
    ["~/", "working_dir"].iter().collect()
}

fn default_results_dir() -> PathBuf {
    ["./", "results"].iter().collect()
}

impl Settings {
    /// Load the settings from a json file.
    pub fn load<P>(path: P) -> SettingsResult<Self>
    where
        P: AsRef<Path> + Display + Clone,
    {
        let reader = || -> Result<Self, std::io::Error> {
            let data = fs::read(path.clone())?;
            let data = resolve_env(std::str::from_utf8(&data).unwrap());
            let settings: Settings = serde_json::from_slice(data.as_bytes())?;

            fs::create_dir_all(&settings.results_dir)?;

            Ok(settings)
        };

        reader().map_err(|e| SettingsError::InvalidSettings {
            file: path.to_string(),
            message: e.to_string(),
        })
    }

    /// Get the name of the repository (from its url).
    pub fn repository_name(&self) -> String {
        self.repository
            .url
            .path_segments()
            .expect("Url should already be checked when loading settings")
            .collect::<Vec<_>>()[1]
            .split('.')
            .next()
            .unwrap()
            .to_string()
    }

    /// Load the secret token to authenticate with the cloud provider.
    pub fn load_token(&self) -> SettingsResult<String> {
        match fs::read_to_string(&self.token_file) {
            Ok(token) => Ok(token.trim_end_matches('\n').to_string()),
            Err(e) => Err(SettingsError::InvalidTokenFile {
                file: self.token_file.display().to_string(),
                message: e.to_string(),
            }),
        }
    }

    /// Load the ssh public key from file.
    pub fn load_ssh_public_key(&self) -> SettingsResult<String> {
        let ssh_public_key_file = self.ssh_public_key_file.clone().unwrap_or_else(|| {
            let mut private = self.ssh_private_key_file.clone();
            private.set_extension("pub");
            private
        });
        match fs::read_to_string(&ssh_public_key_file) {
            Ok(token) => Ok(token.trim_end_matches('\n').to_string()),
            Err(e) => Err(SettingsError::InvalidSshPublicKeyFile {
                file: ssh_public_key_file.display().to_string(),
                message: e.to_string(),
            }),
        }
    }

    /// Check if the build cache is enabled.
    pub fn build_cache_enabled(&self) -> bool {
        self.build_cache
            .as_ref()
            .map(|b| b.enabled)
            .unwrap_or(false)
    }

    /// Get build groups for the default binaries (iota, iota-node, stress).
    /// Groups binaries by toolchain and features to minimize build steps.
    pub fn build_groups(&self) -> BuildGroups {
        let mut groups: BuildGroups = HashMap::new();

        for name in ["iota", "iota-node", "stress"] {
            let config = self.build_configs.get(name);

            let mut features = config.map(|c| c.features.clone()).unwrap_or_default();
            features.sort(); // Sort for consistent grouping

            let group = BuildGroup {
                toolchain: config.and_then(|c| c.toolchain.clone()),
                features,
            };

            groups.entry(group).or_default().push(name.to_string());
        }

        groups
    }

    /// Get the build cache server configuration for a specific CPU target.
    pub fn build_cache_server_for_target(&self, cpu_target: &str) -> Option<&BuildCacheServer> {
        self.build_cache.as_ref().and_then(|build_cache| {
            if build_cache.enabled {
                // Find a server that supports this CPU target
                build_cache
                    .servers
                    .values()
                    .find(|server| server.targets.contains(&cpu_target.to_string()))
            } else {
                None
            }
        })
    }

    /// The number of regions specified in the settings.
    #[cfg(test)]
    pub fn number_of_regions(&self) -> usize {
        self.regions.len()
    }

    /// Test settings for unit tests.
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        // Create a temporary public key file.
        let mut path = tempfile::tempdir().unwrap().keep();
        path.push("test_public_key.pub");
        let public_key = "This is a fake public key for tests";
        fs::write(&path, public_key).unwrap();

        // Return set settings.
        Self {
            testbed_id: "testbed".into(),
            cloud_provider: CloudProvider::Aws,
            token_file: "/path/to/token/file".into(),
            ssh_private_key_file: "/path/to/private/key/file".into(),
            ssh_public_key_file: Some(path),
            regions: vec!["London".into(), "New York".into()],
            node_specs: "small".into(),
            client_specs: "small".into(),
            metrics_specs: "small".into(),
            build_cache: Some(BuildCache {
                enabled: false,
                servers: HashMap::from([(
                    "x86_server".to_string(),
                    BuildCacheServer {
                        url: "http://127.0.0.1:8080".into(),
                        username: None,
                        password: None,
                        targets: vec![
                            "x86-64".to_string(),
                            "x86-64-v2".to_string(),
                            "x86-64-v3".to_string(),
                        ],
                    },
                )]),
            }),
            repository: Repository {
                url: Url::parse("https://example.net/author/repo").unwrap(),
                commit: "main".into(),
            },
            working_dir: "/path/to/working_dir".into(),
            use_fullnode_for_execution: false,
            results_dir: "results".into(),
            build_configs: HashMap::new(),
            enable_flamegraph: false,
            enable_prometheus_snapshots: false,
        }
    }
}

/// Represents a group of binaries that can be built together with the same
/// toolchain and features.
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct BuildGroup {
    pub toolchain: Option<String>,
    pub features: Vec<String>,
}

/// Maps build groups to the list of binary names in each group.
pub type BuildGroups = HashMap<BuildGroup, Vec<String>>;

// Resolves ${ENV} into it's value for each env variable.
fn resolve_env(s: &str) -> String {
    let mut s = s.to_string();
    for (name, value) in env::vars() {
        s = s.replace(&format!("${{{name}}}"), &value);
    }
    if s.contains("${") {
        eprintln!("settings.json:\n{s}\n");
        panic!("Unresolved env variables in the settings.json");
    }
    s
}

#[cfg(test)]
mod test {
    use reqwest::Url;

    use crate::settings::Settings;

    #[test]
    fn repository_name() {
        let mut settings = Settings::new_for_test();
        settings.repository.url = Url::parse("https://example.com/author/name").unwrap();
        assert_eq!(settings.repository_name(), "name");
    }
}
