// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, path::PathBuf, str::FromStr};

use eyre::Result;
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IotaBenchmarkType {
    /// Percentage of shared vs owned objects; 0 means only owned objects and
    /// 100 means only shared objects.
    SharedObjectsRatio(u16),
    /// Benchmark for Abstract Account functionality.
    AbstractAccountBench,
}

impl Default for IotaBenchmarkType {
    fn default() -> Self {
        Self::SharedObjectsRatio(0)
    }
}

impl std::fmt::Debug for IotaBenchmarkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SharedObjectsRatio(shared_objects_ratio) => write!(f, "{shared_objects_ratio}"),
            Self::AbstractAccountBench => write!(f, "abstract_account_bench"),
        }
    }
}

impl std::fmt::Display for IotaBenchmarkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SharedObjectsRatio(shared_objects_ratio) => {
                write!(f, "bench ({}% shared objects)", shared_objects_ratio)
            }
            Self::AbstractAccountBench => write!(f, "abstract-account-bench"),
        }
    }
}

impl FromStr for IotaBenchmarkType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = s.trim().to_ascii_lowercase();

        // Backward compatible: numeric => bench(shared_ratio)
        if let Ok(n) = v.parse::<u16>() {
            return Ok(Self::SharedObjectsRatio(n.min(100)));
        }

        match v.as_str() {
            "abstract-account-bench" => Ok(Self::AbstractAccountBench),
            _ => Err(format!(
                "Unknown benchmark type '{s}'. Expected 0..=100 or abstract-account-bench"
            )),
        }
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

    fn otel_env(
        &self,
        parameters: &BenchmarkParameters<IotaBenchmarkType>,
        service_name: &str,
    ) -> Vec<String> {
        let Some(otel) = &parameters.otel else {
            return vec![];
        };
        // TRACE_FILTER values can be implemented as params as well. For now, we keep
        // them fixed to trace handle_transaction and process_certificate which are the
        // most relevant spans for benchmarks.
        vec![
            format!("export OTEL_EXPORTER_OTLP_ENDPOINT={}", otel.otlp_endpoint),
            "export TRACE_FILTER=[handle_transaction]=trace,[process_certificate]=trace"
                .to_string(),
            format!(
                "export OTEL_EXPORTER_OTLP_TRACES_ENDPOINT={}",
                otel.otlp_endpoint
            ),
            format!("export OTEL_EXPORTER_OTLP_PROTOCOL={}", otel.protocol),
            format!("export OTEL_TRACES_SAMPLER={}", otel.sampler),
            format!("export OTEL_TRACES_SAMPLER_ARG={}", otel.sampler_arg),
            format!("export OTEL_SERVICE_NAME={service_name}"),
            format!("export OTEL_RESOURCE_ATTRIBUTES=service.name={service_name}"),
        ]
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
        let authorities_db = self.working_dir.join(iota_config::AUTHORITIES_DB_NAME);
        let consensus_db = self.working_dir.join(iota_config::CONSENSUS_DB_NAME);
        let full_node_db: PathBuf = self.working_dir.join(iota_config::FULL_NODE_DB_PATH);
        vec![authorities_db, consensus_db, full_node_db]
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
            &[
                // Set protocol config override to disable validator subsidies for benchmarks
                "export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1",
                "export IOTA_PROTOCOL_CONFIG_OVERRIDE_validator_target_reward=0",
                &format!("mkdir -p {working_dir}")
            ],
            &[
                "genesis",
                &format!("-f --working-dir {working_dir} --benchmark-ips {ips} --admin-interface-address=localhost:1337"),
                &epoch_duration_flag,
                &chain_start_timestamp_flag,
                &additional_gas_accounts_flag,
            ],
        );

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

                let mut setup: Vec<String> = vec![
                    match parameters.consensus_protocol {
                        ConsensusProtocol::Starfish => {
                            "export CONSENSUS_PROTOCOL=starfish".to_string()
                        }
                        ConsensusProtocol::Mysticeti => {
                            "export CONSENSUS_PROTOCOL=mysticeti".to_string()
                        }
                        ConsensusProtocol::SwapEachEpoch => {
                            "export CONSENSUS_PROTOCOL=swap_each_epoch".to_string()
                        }
                    },
                    format!("export MAX_PIPELINE_DELAY={max_pipeline_delay}"),
                    // Set protocol config override to disable validator subsidies for
                    // benchmarks
                    format!("export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1"),
                    format!("export IOTA_PROTOCOL_CONFIG_OVERRIDE_validator_target_reward=0"),
                ];

                if self.enable_flamegraph {
                    setup.push("export TRACE_FLAMEGRAPH=1".to_string());
                }

                setup.extend(self.otel_env(parameters, &format!("iota-validator-{i}")));

                let iota_node_command = self.run_binary_command(
                    "iota-node",
                    &setup,
                    &[format!(
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

                let mut setup = vec![
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
                        // Set protocol config override to disable validator subsidies for benchmarks
                        "export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1".to_string(),
                        "export IOTA_PROTOCOL_CONFIG_OVERRIDE_validator_target_reward=0".to_string(),
                ];
                setup.extend(self.otel_env(parameters, &format!("iota-node-{i}")));

                let iota_node_command = self.run_binary_command(
                    "iota-node",
                    &setup,
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

                let client_threads = parameters.stress_num_client_threads;
                let server_threads = parameters.stress_num_server_threads;

                let mut stress_args: Vec<String> = vec![
                    format!("--num-client-threads {client_threads} --num-server-threads {server_threads}"),
                    "--local false --num-transfer-accounts 2".to_string(),
                    format!("--genesis-blob-path {genesis} --keystore-path {keystore}"),
                    format!("--primary-gas-owner-id {gas_address}"),
                    // Run interval param
                    parameters.run_interval.as_stress_flag(),
                    format!("--client-metric-host 0.0.0.0 --client-metric-port {metrics_port}"),
                    if let Some(stats_path) = &parameters.benchmark_stats_path {
                        format!("--benchmark-stats-path {stats_path}")
                    } else {
                        "".to_string()
                    },
                ];

                match parameters.benchmark_type {
                    IotaBenchmarkType::SharedObjectsRatio(shared_objects_ratio) => {
                        let transfer_objects = 100 - shared_objects_ratio;
                        let hotness_factor = parameters.shared_counter_hotness_factor.unwrap_or(50);
                        stress_args.push("bench".to_string());
                        stress_args.push(format!("--target-qps {load_share}"));
                        stress_args.push(format!("--num-workers {}", parameters.stress_num_workers));
                        stress_args.push(format!("--in-flight-ratio {}", parameters.stress_in_flight_ratio));
                        stress_args.push(format!("--shared-counter {shared_objects_ratio} --transfer-object {transfer_objects}"));
                        stress_args.push(format!("--shared-counter-hotness-factor {hotness_factor}"));
                        if let Some(num_counters) = parameters.num_shared_counters {
                            stress_args.push(format!("--num-shared-counters {num_counters}"));
                        }
                    }

                    IotaBenchmarkType::AbstractAccountBench => {
                        stress_args.push("abstract-account-bench".to_string());
                        stress_args.push(format!("--authenticator {}", parameters.aa_authenticator));
                        stress_args.push(format!("--tx-payload-obj-type {}", parameters.tx_payload_obj_type));
                        stress_args.push(format!("--target-qps {load_share}"));
                        stress_args.push(format!("--num-workers {}", parameters.stress_num_workers));
                        stress_args.push(format!("--in-flight-ratio {}", parameters.stress_in_flight_ratio));
                        stress_args.push(format!("--split-amount {}", parameters.aa_split_amount));
                        if parameters.should_fail {
                            stress_args.push("--should-fail".to_string());
                        }
                    }
                }

                if self.use_fullnode_for_execution {
                    stress_args.push("--use-fullnode-for-execution true".to_string());
                    stress_args.push("--fullnode-rpc-addresses http://127.0.0.1:9000".to_string());
                }

                let mut setup = vec![
                    "export MOVE_EXAMPLES_DIR=$(pwd)/examples/move".to_string(),
                    "export RUST_LOG=iota_benchmark=debug".to_string(),
                ];

                setup.extend(self.otel_env(parameters, &format!("iota-stress-{i}")));

                let stress_command = self.run_binary_command("stress", &setup, &stress_args);

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

        // `u64::MAX - 1` is the max total supply value acceptable by
        // `iota::balance::increase_supply`
        let genesis_config = GenesisConfig::new_for_benchmarks(
            &ips,
            parameters.epoch_duration_ms,
            parameters.chain_start_timestamp_ms,
            Some(parameters.additional_gas_accounts),
            u64::MAX - 1,
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

    fn nodes_metrics_path<I>(
        &self,
        instances: I,
        use_internal_ip_address: bool,
    ) -> Vec<(Instance, String)>
    where
        I: IntoIterator<Item = Instance>,
    {
        let instances = instances.into_iter().collect::<Vec<_>>();
        let ips = (0..instances.len())
            .map(|_| "0.0.0.0".to_string())
            .collect::<Vec<_>>();
        // From GenesisConfig we only need validators' `metrics_address` port which is
        // computed from validator's offset in `ips`. The values of (the rest
        // of) the arguments are irrelevant.
        GenesisConfig::new_for_benchmarks(&ips, None, None, None, u64::MAX)
            .validator_config_info
            .expect("No validator in genesis")
            .iter()
            .zip(instances)
            .map(|(config, instance)| {
                let path = format!(
                    "{}:{}{}",
                    match use_internal_ip_address {
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

    fn clients_metrics_path<I>(
        &self,
        instances: I,
        use_internal_ip_address: bool,
    ) -> Vec<(Instance, String)>
    where
        I: IntoIterator<Item = Instance>,
    {
        instances
            .into_iter()
            .map(|instance| {
                let path = format!(
                    "{}:{}{}",
                    match use_internal_ip_address {
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
