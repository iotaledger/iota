// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::Duration};

use benchmark::{BenchmarkParametersGenerator, LoadType};
use clap::{Parser, ValueEnum};
use client::{ServerProviderClient, aws::AwsClient};
use eyre::{Context, Result};
use faults::FaultsType;
use measurement::MeasurementsCollection;
use orchestrator::Orchestrator;
use protocol::iota::{IotaBenchmarkType, IotaProtocol};
use settings::{CloudProvider, Settings};
use ssh::SshConnectionManager;
use testbed::Testbed;

use crate::net_latency::TopologyLayout;

pub mod benchmark;
pub mod client;
pub mod display;
pub mod error;
pub mod faults;
pub mod logs;
pub mod measurement;
mod monitor;
pub mod net_latency;
pub mod orchestrator;
pub mod protocol;
pub mod settings;
pub mod ssh;
pub mod testbed;

type Protocol = IotaProtocol;
type BenchmarkType = IotaBenchmarkType;

#[derive(Parser)]
#[command(author, version, about = "Testbed orchestrator", long_about = None)]
pub struct Opts {
    /// The path to the settings file. This file contains basic information to
    /// deploy testbeds and run benchmarks such as the url of the git repo,
    /// the commit to deploy, etc.
    #[arg(
        long,
        value_name = "FILE",
        default_value = "crates/iota-aws-orchestrator/assets/settings.json",
        global = true
    )]
    settings_path: String,

    /// The type of operation to run.
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Parser)]
pub enum Operation {
    /// Get or modify the status of the testbed.
    Testbed {
        #[command(subcommand)]
        action: TestbedAction,
    },

    /// Run a benchmark on the specified testbed.
    Benchmark {
        /// Percentage of shared vs owned objects; 0 means only owned objects
        /// and 100 means only shared objects.
        #[arg(long, default_value = "0", global = true)]
        benchmark_type: String,

        /// The committee size to deploy.
        #[arg(long, value_name = "INT")]
        committee: usize,

        /// Number of faulty nodes.
        #[arg(long, value_name = "INT", default_value = "0", global = true)]
        faults: usize,

        /// Whether the faulty nodes recover.
        #[arg(long, action, default_value = "false", global = true)]
        crash_recovery: bool,

        /// The interval to crash nodes in seconds.
        #[arg(long, value_parser = parse_duration, default_value = "60", global = true)]
        crash_interval: Duration,

        /// The minimum duration of the benchmark in seconds.
        #[arg(long, value_parser = parse_duration, default_value = "600", global = true)]
        duration: Duration,

        /// The interval between measurements collection in seconds.
        #[arg(long, value_parser = parse_duration, default_value = "15", global = true)]
        scrape_interval: Duration,

        /// Whether to skip testbed updates before running benchmarks.
        #[arg(long, action, default_value = "false", global = true)]
        skip_testbed_update: bool,

        /// Whether to skip testbed configuration before running benchmarks.
        #[arg(long, action, default_value = "false", global = true)]
        skip_testbed_configuration: bool,

        /// Whether to download and analyze the client and node log files.
        #[arg(long, action, default_value = "false", global = true)]
        log_processing: bool,

        /// The number of instances running exclusively load generators. If set
        /// to zero the orchestrator collocates one load generator with
        /// each node.
        #[arg(long, value_name = "INT", default_value = "0", global = true)]
        dedicated_clients: usize,

        /// Whether to forgo a grafana and prometheus instance and leave the
        /// testbed unmonitored.
        #[arg(long, action, default_value = "false", global = true)]
        skip_monitoring: bool,

        /// The timeout duration for ssh commands (in seconds).
        #[arg(long, action, value_parser = parse_duration, default_value = "30", global = true)]
        timeout: Duration,

        /// The number of times the orchestrator should retry an ssh command.
        #[arg(long, value_name = "INT", default_value = "5", global = true)]
        retries: usize,

        /// The load to submit to the system.
        #[command(subcommand)]
        load_type: Load,

        /// Flag indicating whether nodes should advertise their internal or
        /// public IP address for inter-node communication. When running
        /// the simulation in multiple regions, nodes need to use their public
        /// IPs to correctly communicate, however when a simulation is
        /// running in a single VPC, they should use their internal IPs to avoid
        /// paying for data sent between the nodes.
        #[clap(long, action, default_value_t = false, global = true)]
        use_internal_ip_addresses: bool,

        /// Optional Latency Topology. if omitted => None -> skips latency
        /// matrix generation
        #[arg(long, global = true)]
        latency_topology: Option<LatencyTopology>,
        /// Optional perturbation spec. If omitted => None
        #[arg(long = "latency-perturbation-spec", global = true)]
        latency_perturbation_spec: Option<PerturbationSpec>,

        /// How many clusters to use in the latency topology
        #[arg(long, value_name = "INT", default_value = "10", global = true)]
        number_of_clusters: usize,

        /// Number-of-triangles parameter for broken-topologies
        #[arg(long, value_name = "INT", default_value = "5", global = true)]
        number_of_triangles: u16,

        /// Extra artificial latency when perturbing topo
        #[arg(long, value_name = "INT", default_value = "20", global = true)]
        added_latency: u16,

        /// Maximum latency between two nodes/clusters in a private network
        #[arg(long, value_name = "INT", default_value = "400", global = true)]
        maximum_latency: u16,

        /// Switch protocols between mysticeti and starfish every epoch,
        /// default: false, aka use starfish in every epoch.
        #[clap(long, action, default_value_t = false, global = true)]
        protocol_switch_each_epoch: bool,

        /// Optional: Epoch duration in milliseconds, default is 1h
        #[arg(long, value_name = "INT", global = true)]
        epoch_duration_ms: Option<u64>,

        /// Number of blocking connections in the blocking
        /// latency_perturbation_spec
        #[arg(long, value_name = "INT", default_value = "1", global = true)]
        blocking_connections: usize,
    },

    /// Print a summary of the specified measurements collection.
    Summarize {
        /// The path to the settings file.
        #[arg(long, value_name = "FILE")]
        path: String,
    },
}

#[derive(Parser)]
pub enum TestbedAction {
    /// Display the testbed status.
    Status,

    /// Deploy the specified number of instances in all regions specified by in
    /// the setting file.
    Deploy {
        /// Number of instances to deploy.
        #[arg(long)]
        instances: usize,

        /// Skips deployment of a Metrics instance
        #[arg(long, action, default_value = "false", global = true)]
        skip_monitoring: bool,

        /// The number of instances running exclusively load generators.
        #[arg(long, value_name = "INT", default_value = "0", global = true)]
        dedicated_clients: usize,

        /// Attempts to prioritise cheaper spot instances
        /// Note: stop and start commands are not available for spot instances
        #[arg(long, action, default_value = "false", global = true)]
        use_spot_instances: bool,
    },

    /// Start at most the specified number of instances per region on an
    /// existing testbed.
    Start {
        /// Number of instances to deploy.
        #[arg(long, default_value = "200")]
        instances: usize,

        // Skips deployment of a Metrics instance
        #[arg(long, action, default_value = "false", global = true)]
        skip_monitoring: bool,

        /// The number of instances running exclusively load generators.
        #[arg(long, value_name = "INT", default_value = "0", global = true)]
        dedicated_clients: usize,
    },

    /// Stop an existing testbed (without destroying the instances).
    Stop {
        /// Keeps the monitoring instance running
        #[arg(long, action, default_value = "false", global = true)]
        keep_monitoring: bool,
    },

    /// Destroy the testbed and terminate all instances.
    Destroy {
        /// Keeps the monitoring instance running
        #[arg(long, action, default_value = "false", global = true)]
        keep_monitoring: bool,
    },
}

#[derive(Parser)]
pub enum Load {
    /// The fixed loads (in tx/s) to submit to the nodes.
    FixedLoad {
        /// A list of fixed load (tx/s).
        #[arg(
            long,
            value_name = "INT",
            num_args(1..),
            value_delimiter = ','
        )]
        loads: Vec<usize>,
    },

    /// Search for the maximum load that the system can sustainably handle.
    Search {
        /// The initial load (in tx/s) to test and use a baseline.
        #[arg(long, value_name = "INT", default_value = "250")]
        starting_load: usize,
        /// The maximum number of iterations before converging on a breaking
        /// point.
        #[arg(long, value_name = "INT", default_value = "5")]
        max_iterations: usize,
    },
}
#[derive(ValueEnum, Clone, Debug)]
pub enum PerturbationSpec {
    BrokenTriangle,
    Blocking,
    // potentially other options later
}

#[derive(ValueEnum, Clone, Debug)]
pub enum LatencyTopology {
    /// Generates a latency matrix for each node, randomly positioned on a
    /// cylinder.
    RandomGeographical,
    /// Generates a latency matrix by randomly clustering nodes into clusters
    /// and randomly positioning clusters on a cylinder.
    RandomClustered,
    /// Uses a hardcoded 10x10 matrix with 10 equal-sized regions.
    HardCodedClustered,
    /// Uses mainnet validator region distribution for latencies.
    Mainnet,
}

fn parse_duration(arg: &str) -> Result<Duration, std::num::ParseIntError> {
    let seconds = arg.parse()?;
    Ok(Duration::from_secs(seconds))
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let opts: Opts = Opts::parse();

    // Load the settings files.
    let settings = Settings::load(&opts.settings_path).wrap_err("Failed to load settings")?;

    match &settings.cloud_provider {
        CloudProvider::Aws => {
            // Create the client for the cloud provider.
            let client = AwsClient::new(settings.clone()).await;

            // Execute the command.
            run(settings, client, opts).await
        }
    }
}

async fn run<C: ServerProviderClient>(settings: Settings, client: C, opts: Opts) -> Result<()> {
    // Create a new testbed.
    let mut testbed = Testbed::new(settings.clone(), client)
        .await
        .wrap_err("Failed to create testbed")?;

    match opts.operation {
        Operation::Testbed { action } => match action {
            // Display the current status of the testbed.
            TestbedAction::Status => testbed.status(),

            // Deploy the specified number of instances on the testbed.
            TestbedAction::Deploy {
                instances,
                dedicated_clients,
                skip_monitoring,
                use_spot_instances,
            } => testbed
                .deploy(
                    instances,
                    skip_monitoring,
                    dedicated_clients,
                    use_spot_instances,
                )
                .await
                .wrap_err("Failed to deploy testbed")?,

            // Start the specified number of instances on an existing testbed.
            TestbedAction::Start {
                instances,
                skip_monitoring,
                dedicated_clients,
            } => testbed
                .start(instances, dedicated_clients, skip_monitoring)
                .await
                .wrap_err("Failed to start testbed")?,

            // Stop an existing testbed.
            TestbedAction::Stop { keep_monitoring } => testbed
                .stop(keep_monitoring)
                .await
                .wrap_err("Failed to stop testbed")?,

            // Destroy the testbed and terminal all instances.
            TestbedAction::Destroy { keep_monitoring } => testbed
                .destroy(keep_monitoring)
                .await
                .wrap_err("Failed to destroy testbed")?,
        },

        // Run benchmarks.
        Operation::Benchmark {
            benchmark_type,
            committee,
            faults,
            crash_recovery,
            crash_interval,
            duration,
            scrape_interval,
            skip_testbed_update,
            skip_testbed_configuration,
            log_processing,
            dedicated_clients,
            skip_monitoring,
            timeout,
            retries,
            load_type,
            use_internal_ip_addresses,
            latency_perturbation_spec,
            latency_topology,
            added_latency,
            number_of_triangles,
            number_of_clusters,
            protocol_switch_each_epoch,
            maximum_latency,
            epoch_duration_ms,
            blocking_connections,
        } => {
            // Create a new orchestrator to instruct the testbed.
            let username = testbed.username();
            let private_key_file = settings.ssh_private_key_file.clone();
            let ssh_manager = SshConnectionManager::new(username.into(), private_key_file)
                .with_timeout(timeout)
                .with_retries(retries);

            let node_instances = testbed.node_instances();
            let client_instances = testbed.client_instances();
            let metrics_instance = testbed.metrics_instance();

            let setup_commands = testbed
                .setup_commands()
                .await
                .wrap_err("Failed to load testbed setup commands")?;

            let protocol_commands = Protocol::new(&settings);
            let benchmark_type = BenchmarkType::from_str(&benchmark_type)?;

            let load = match load_type {
                Load::FixedLoad { loads } => {
                    let loads = if loads.is_empty() { vec![200] } else { loads };
                    LoadType::Fixed(loads)
                }
                Load::Search {
                    starting_load,
                    max_iterations,
                } => LoadType::Search {
                    starting_load,
                    max_iterations,
                },
            };

            let fault_type = if !crash_recovery || faults == 0 {
                FaultsType::Permanent { faults }
            } else {
                FaultsType::CrashRecovery {
                    max_faults: faults,
                    interval: crash_interval,
                }
            };

            let perturbation_spec = match latency_perturbation_spec {
                Some(PerturbationSpec::BrokenTriangle) => {
                    net_latency::PerturbationSpec::BrokenTriangle {
                        added_latency,
                        number_of_triangles,
                    }
                }
                Some(PerturbationSpec::Blocking) => net_latency::PerturbationSpec::Blocking {
                    number_of_blocked_connections: blocking_connections,
                },
                None => net_latency::PerturbationSpec::None,
            };

            let latency_topology = match latency_topology {
                Some(LatencyTopology::RandomGeographical) => {
                    Some(TopologyLayout::RandomGeographical)
                }
                Some(LatencyTopology::RandomClustered) => {
                    Some(TopologyLayout::RandomClustered { number_of_clusters })
                }
                Some(LatencyTopology::HardCodedClustered) => {
                    Some(TopologyLayout::HardCodedClustered)
                }
                Some(LatencyTopology::Mainnet) => Some(TopologyLayout::Mainnet),
                None => None,
            };

            let generator =
                BenchmarkParametersGenerator::new(committee, load, use_internal_ip_addresses)
                    .with_benchmark_type(benchmark_type)
                    .with_custom_duration(duration)
                    .with_perturbation_spec(perturbation_spec)
                    .with_latency_topology(latency_topology)
                    .with_protocol_switch_each_epoch(protocol_switch_each_epoch)
                    .with_max_latency(maximum_latency)
                    .with_epoch_duration(epoch_duration_ms)
                    .with_faults(fault_type);

            Orchestrator::new(
                settings,
                node_instances,
                client_instances,
                metrics_instance,
                setup_commands,
                protocol_commands,
                ssh_manager,
            )
            .with_scrape_interval(scrape_interval)
            .with_crash_interval(crash_interval)
            .skip_testbed_updates(skip_testbed_update)
            .skip_testbed_configuration(skip_testbed_configuration)
            .with_log_processing(log_processing)
            .with_dedicated_clients(dedicated_clients)
            .skip_monitoring(skip_monitoring)
            .run_benchmarks(generator)
            .await
            .wrap_err("Failed to run benchmarks")?;
        }

        // Print a summary of the specified measurements collection.
        Operation::Summarize { path } => {
            MeasurementsCollection::<BenchmarkType>::load(path)?.display_summary()
        }
    }
    Ok(())
}
