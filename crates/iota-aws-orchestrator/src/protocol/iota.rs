// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    path::PathBuf,
    str::FromStr,
};

use iota_swarm_config::genesis_config::GenesisConfig;
use iota_types::{base_types::IotaAddress, multiaddr::Multiaddr};
use serde::{Deserialize, Serialize};

use super::{ProtocolCommands, ProtocolMetrics};
use crate::{
    ConsensusProtocol,
    benchmark::{BenchmarkParameters, BenchmarkType},
    client::Instance,
    display,
    settings::{BinaryBuildConfig, Settings, build_cargo_command, join_non_empty_strings},
};

#[derive(Serialize, Deserialize, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IotaBenchmarkType {
    /// Percentage of shared vs owned objects; 0 means only owned objects and
    /// 100 means only shared objects.
    shared_objects_ratio: u16,
}

impl Debug for IotaBenchmarkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.shared_objects_ratio)
    }
}

impl Display for IotaBenchmarkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}% shared objects", self.shared_objects_ratio)
    }
}

impl FromStr for IotaBenchmarkType {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            shared_objects_ratio: s.parse::<u16>()?.min(100),
        })
    }
}

impl BenchmarkType for IotaBenchmarkType {}

/// All configurations information to run an IOTA client or validator.
pub struct IotaProtocol {
    working_dir: PathBuf,
    use_fullnode_for_execution: bool,
    use_precompiled_binaries: bool,
    build_configs: HashMap<String, BinaryBuildConfig>,
    enable_flamegraph: bool,
}

impl IotaProtocol {
    /// Make a new instance of the IOTA protocol commands generator.
    pub fn new(settings: &Settings) -> Self {
        Self {
            working_dir: [&settings.working_dir, &iota_config::IOTA_CONFIG_DIR.into()]
                .iter()
                .collect(),
            use_fullnode_for_execution: settings.use_fullnode_for_execution,
            use_precompiled_binaries: settings.build_cache_enabled(),
            build_configs: settings.build_configs.clone(),
            enable_flamegraph: settings.enable_flamegraph,
        }
    }

    /// Build the command to run a binary, either using precompiled binary or
    /// cargo run. Returns the command string with proper toolchain and
    /// features configured.
    fn run_binary_command<S1: AsRef<str>, S2: AsRef<str>>(
        &self,
        binary_name: &str,
        setup_commands: &[S1],
        additional_args: &[S2],
    ) -> String {
        if self.use_precompiled_binaries {
            // The precompiled binary is located in the working directory
            let binary_path = format!("./target/release/{binary_name}");
            let binary_command = join_non_empty_strings(
                &std::iter::once(binary_path.as_str())
                    .chain(additional_args.iter().map(|s| s.as_ref()))
                    .collect::<Vec<_>>(),
                " ",
            );

            let all_commands: Vec<String> = setup_commands
                .iter()
                .map(|s| s.as_ref().to_string())
                .chain(std::iter::once(binary_command))
                .collect();

            join_non_empty_strings(&all_commands, " && ")
        } else {
            let build_config = self
                .build_configs
                .get(binary_name)
                .expect("No build config found for binary");
            build_cargo_command(
                "run",
                build_config.toolchain.clone(),
                build_config.features.clone(),
                &[binary_name],
                setup_commands,
                additional_args,
            )
        }
    }
}

impl ProtocolCommands<IotaBenchmarkType> for IotaProtocol {
    fn protocol_dependencies(&self) -> Vec<&'static str> {
        if !self.use_precompiled_binaries {
            return vec!["sudo apt-get -y install libudev-dev libpq5 libpq-dev"];
        }

        vec![]
    }

    fn db_directories(&self) -> Vec<PathBuf> {
        let authorities_db = [&self.working_dir, &iota_config::AUTHORITIES_DB_NAME.into()]
            .iter()
            .collect();
        let consensus_db = [&self.working_dir, &iota_config::CONSENSUS_DB_NAME.into()]
            .iter()
            .collect();
        vec![authorities_db, consensus_db]
    }

    fn genesis_command<'a, I>(
        &self,
        instances: I,
        parameters: &BenchmarkParameters<IotaBenchmarkType>,
    ) -> String
    where
        I: Iterator<Item = &'a Instance>,
    {
        let working_dir = self.working_dir.display();
        let ips = instances
            .map(|x| {
                match parameters.use_internal_ip_address {
                    true => x.private_ip,
                    false => x.main_ip,
                }
                .to_string()
            })
            .collect::<Vec<_>>()
            .join(" ");

        let epoch_duration_flag = parameters
            .epoch_duration_ms
            .map(|epoch_duration_ms| format!("--epoch-duration-ms {epoch_duration_ms}"))
            .unwrap_or_default();
        let chain_start_timestamp_flag = parameters
            .chain_start_timestamp_ms
            .map(|timestamp_ms| format!("--chain-start-timestamp-ms {timestamp_ms}"))
            .unwrap_or_default();
        let additional_gas_accounts_flag = format!(
            "--num-additional-gas-accounts {}",
            parameters.additional_gas_accounts
        );

        let iota_command = self.run_binary_command(
            "iota",
            &[&format!("mkdir -p {working_dir}")],
            &[
                "genesis",
                &format!("-f --working-dir {working_dir} --benchmark-ips {ips} --admin-interface-address=localhost:1337"),
                &epoch_duration_flag,
                &chain_start_timestamp_flag,
                &additional_gas_accounts_flag,
            ],
        );

        display::action(format!("\n Genesis Command: {iota_command}"));

        iota_command
    }

    fn monitor_command<I>(&self, _instances: I) -> Vec<(Instance, String)>
    where
        I: IntoIterator<Item = Instance>,
    {
        // instances
        //     .into_iter()
        //     .map(|i| {
        //         (
        //             i,
        //             "tail -f --pid=$(pidof iota) -f /dev/null; tail -100
        // node.log".to_string(),         )
        //     })
        //     .collect()
        vec![]
    }

    fn node_command<I>(
        &self,
        instances: I,
        parameters: &BenchmarkParameters<IotaBenchmarkType>,
    ) -> Vec<(Instance, String)>
    where
        I: IntoIterator<Item = Instance>,
    {
        let working_dir = self.working_dir.clone();
        let network_addresses = Self::resolve_network_addresses(instances, parameters);

        network_addresses
            .into_iter()
            .enumerate()
            .map(|(i, (instance, network_address))| {
                let validator_config =
                    iota_config::validator_config_file(network_address.clone(), i);
                let config_path: PathBuf = working_dir.join(validator_config);
                let max_pipeline_delay = parameters.max_pipeline_delay;
                let iota_node_command = self.run_binary_command(
                    "iota-node",
                    &[
                        match parameters.consensus_protocol {
                            ConsensusProtocol::Starfish => "export CONSENSUS_PROTOCOL=starfish",
                            ConsensusProtocol::Mysticeti => "export CONSENSUS_PROTOCOL=mysticeti",
                            ConsensusProtocol::SwapEachEpoch => {
                                "export CONSENSUS_PROTOCOL=swap_each_epoch"
                            }
                        },
                        format!("export MAX_PIPELINE_DELAY={max_pipeline_delay}").as_str(),
                        if self.enable_flamegraph {
                            "export TRACE_FLAMEGRAPH=1"
                        } else {
                            ""
                        },
                    ],
                    &[&format!(
                        "--config-path {} --listen-address {}",
                        config_path.display(),
                        network_address.with_zero_ip()
                    )],
                );

                display::action(format!(
                    "\n Validator-node Command ({i}): {iota_node_command}"
                ));

                (instance, iota_node_command)
            })
            .collect()
    }

    fn fullnode_command<I>(
        &self,
        instances: I,
        parameters: &BenchmarkParameters<IotaBenchmarkType>,
    ) -> Vec<(Instance, String)>
    where
        I: IntoIterator<Item = Instance>,
    {
        let working_dir = self.working_dir.clone();

        instances
            .into_iter()
            .enumerate()
            .map(|(i, instance)| {
                let config_path: PathBuf = working_dir.join(iota_config::IOTA_FULLNODE_CONFIG);
                let fullnode_ip = match parameters.use_internal_ip_address {
                    true => &instance.private_ip,
                    false => &instance.main_ip,
                };

                let iota_node_command = self.run_binary_command(
                    "iota-node",
                    &[
                        // Overwrite listen address and external address with 0.0.0.0 and actual fullnode IP.
                        // Escape quotes for proper handling inside tmux wrapper
                        format!(
                            "sed -i 's|listen-address: \\\"127.0.0.1:|listen-address: \\\"0.0.0.0:|' {0} && sed -i 's|external-address: /ip4/127.0.0.1/|external-address: /ip4/{1}/|' {0}",
                            config_path.display(),
                            fullnode_ip
                        ),
                        if self.enable_flamegraph {
                            "export TRACE_FLAMEGRAPH=1".to_string()
                        } else {
                            "".to_string()
                        },
                    ],
                    &[&format!("--config-path {}", config_path.display())],
                );

                display::action(format!("\n Full-node Command ({i}): {iota_node_command}"));

                (instance, iota_node_command)
            })
            .collect()
    }

    fn client_command<I>(
        &self,
        instances: I,
        parameters: &BenchmarkParameters<IotaBenchmarkType>,
    ) -> Vec<(Instance, String)>
    where
        I: IntoIterator<Item = Instance>,
    {
        let genesis_path: PathBuf = [
            &self.working_dir,
            &iota_config::IOTA_GENESIS_FILENAME.into(),
        ]
        .iter()
        .collect();
        let keystore_path: PathBuf = [
            &self.working_dir,
            &iota_config::IOTA_BENCHMARK_GENESIS_GAS_KEYSTORE_FILENAME.into(),
        ]
        .iter()
        .collect();

        let committee_size = parameters.nodes;
        let clients: Vec<_> = instances.into_iter().collect();
        let load_share = parameters.load / clients.len();
        let shared_counter = parameters.benchmark_type.shared_objects_ratio;
        let transfer_objects = 100 - shared_counter;
        let metrics_port = Self::CLIENT_METRICS_PORT;
        // Get gas keys for all validators and clients
        let gas_keys =
            GenesisConfig::benchmark_gas_keys(committee_size + parameters.additional_gas_accounts);
        // Validators use the first `nodes` keys, so clients should start after that
        let client_key_offset = committee_size;

        clients
            .into_iter()
            .enumerate()
            .map(|(i, instance)| {
                let genesis = genesis_path.display().to_string();
                let keystore = keystore_path.display().to_string();
                // Offset client key index to avoid colliding with validator keys
                let gas_key = &gas_keys[client_key_offset + i];
                let gas_address = IotaAddress::from(&gas_key.public());

                let mut stress_args: Vec<String> = vec![
                    "--num-client-threads 24 --num-server-threads 1".to_string(),
                    "--local false --num-transfer-accounts 2".to_string(),
                    format!("--genesis-blob-path {genesis} --keystore-path {keystore}"),
                    format!("--primary-gas-owner-id {gas_address}"),
                    "bench".to_string(),
                    format!("--in-flight-ratio 30 --num-workers 24 --target-qps {load_share}"),
                    format!(
                        "--shared-counter {shared_counter} --transfer-object {transfer_objects}"
                    ),
                    "--shared-counter-hotness-factor 50".to_string(),
                    format!("--client-metric-host 0.0.0.0 --client-metric-port {metrics_port}"),
                ];

                if self.use_fullnode_for_execution {
                    stress_args.push("--use-fullnode-for-execution true".to_string());
                    stress_args.push("--fullnode-rpc-addresses http://127.0.0.1:9000".to_string());
                }

                let stress_command = self.run_binary_command(
                    "stress",
                    // required for stress binary, otherwise it will use the CARGO_MANIFEST_DIR,
                    // which is set during compilation time
                    &["export MOVE_EXAMPLES_DIR=$(pwd)/examples/move"],
                    &stress_args,
                );

                display::action(format!("\n Stress Command ({i}): {stress_command}"));

                (instance, stress_command)
            })
            .collect()
    }
}

impl IotaProtocol {
    const CLIENT_METRICS_PORT: u16 = GenesisConfig::BENCHMARKS_PORT_OFFSET + 2000;

    /// Creates the network addresses in multi address format for the instances.
    /// It returns the Instance and the corresponding address.
    pub fn resolve_network_addresses(
        instances: impl IntoIterator<Item = Instance>,
        parameters: &BenchmarkParameters<IotaBenchmarkType>,
    ) -> Vec<(Instance, Multiaddr)> {
        let instances: Vec<Instance> = instances.into_iter().collect();
        let ips: Vec<_> = instances
            .iter()
            .map(|x| match parameters.use_internal_ip_address {
                true => x.private_ip.to_string(),
                false => x.main_ip.to_string(),
            })
            .collect();
        let genesis_config = GenesisConfig::new_for_benchmarks(
            &ips,
            parameters.epoch_duration_ms,
            parameters.chain_start_timestamp_ms,
            Some(parameters.additional_gas_accounts),
        );
        let mut addresses = Vec::new();
        if let Some(validator_configs) = genesis_config.validator_config_info.as_ref() {
            for (i, validator_info) in validator_configs.iter().enumerate() {
                let address = &validator_info.network_address;
                addresses.push((instances[i].clone(), address.clone()));
            }
        }
        addresses
    }
}

impl ProtocolMetrics for IotaProtocol {
    const BENCHMARK_DURATION: &'static str = "benchmark_duration";
    const TOTAL_TRANSACTIONS: &'static str = "latency_s_count";
    const LATENCY_BUCKETS: &'static str = "latency_s";
    const LATENCY_SUM: &'static str = "latency_s_sum";
    const LATENCY_SQUARED_SUM: &'static str = "latency_squared_s";

    fn nodes_metrics_path<I, T>(
        &self,
        instances: I,
        parameters: &BenchmarkParameters<T>,
    ) -> Vec<(Instance, String)>
    where
        I: IntoIterator<Item = Instance>,
        T: BenchmarkType,
    {
        let (ips, instances): (Vec<_>, Vec<_>) = instances
            .into_iter()
            .map(|x| {
                (
                    match parameters.use_internal_ip_address {
                        true => x.private_ip,
                        false => x.main_ip,
                    }
                    .to_string(),
                    x,
                )
            })
            .unzip();
        GenesisConfig::new_for_benchmarks(
            &ips,
            parameters.epoch_duration_ms,
            parameters.chain_start_timestamp_ms,
            Some(parameters.additional_gas_accounts),
        )
        .validator_config_info
        .expect("No validator in genesis")
        .iter()
        .zip(instances)
        .map(|(config, instance)| {
            let path = format!(
                "{}:{}{}",
                match parameters.use_internal_ip_address {
                    true => instance.private_ip,
                    false => instance.main_ip,
                },
                config.metrics_address.port(),
                iota_metrics::METRICS_ROUTE
            );
            (instance, path)
        })
        .collect()
    }

    fn clients_metrics_path<I, T>(
        &self,
        instances: I,
        parameters: &BenchmarkParameters<T>,
    ) -> Vec<(Instance, String)>
    where
        I: IntoIterator<Item = Instance>,
        T: BenchmarkType,
    {
        instances
            .into_iter()
            .map(|instance| {
                let path = format!(
                    "{}:{}{}",
                    match parameters.use_internal_ip_address {
                        true => instance.private_ip,
                        false => instance.main_ip,
                    },
                    Self::CLIENT_METRICS_PORT,
                    iota_metrics::METRICS_ROUTE
                );
                (instance, path)
            })
            .collect()
    }
}
