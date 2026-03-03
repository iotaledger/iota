// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt::{Debug, Display, Formatter},
    hash::Hash,
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use duration_str::parse;
use iota_benchmark::workloads::abstract_account::{AuthenticatorKind, TxPayloadObjType};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
pub enum RunInterval {
    Count(u64),
    Time(tokio::time::Duration),
}

impl RunInterval {
    pub fn time_limit_secs(&self) -> Option<u64> {
        match self {
            RunInterval::Time(d) => Some(d.as_secs()),
            RunInterval::Count(_) => None,
        }
    }
    pub fn as_stress_flag(&self) -> String {
        match self {
            RunInterval::Time(d) => format!("--run-duration {}s", d.as_secs()),
            RunInterval::Count(n) => format!("--run-duration {n}"),
        }
    }
}

impl FromStr for RunInterval {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(i) = s.parse() {
            Ok(RunInterval::Count(i))
        } else if let Ok(d) = parse(s) {
            Ok(RunInterval::Time(d))
        } else {
            Err("Required integer number of cycles or time duration".to_string())
        }
    }
}

impl std::fmt::Display for RunInterval {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RunInterval::Count(count) => f.write_str(format!("{count}").as_str()),
            RunInterval::Time(d) => f.write_str(format!("{}sec", d.as_secs()).as_str()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OtelConfig {
    pub otlp_endpoint: String, // example "http://10.0.0.12:4317"
    pub protocol: String,      // "grpc" (or "http/protobuf")
    pub sampler: String,       // "parentbased_traceidratio"
    pub sampler_arg: String,   // "0.1"
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            otlp_endpoint: String::new(),
            protocol: "grpc".to_string(),
            sampler: "parentbased_traceidratio".to_string(),
            sampler_arg: "0.1".to_string(),
        }
    }
}

use crate::{
    ConsensusProtocol,
    faults::FaultsType,
    measurement::MeasurementsCollection,
    net_latency::{PerturbationSpec, TopologyLayout},
};

pub trait BenchmarkType:
    Serialize
    + DeserializeOwned
    + Default
    + Clone
    + FromStr
    + Display
    + Debug
    + PartialEq
    + Eq
    + Hash
    + PartialOrd
    + Ord
    + FromStr
{
}

/// The benchmark parameters for a run.
#[derive(Serialize, Deserialize, Clone)]
pub struct BenchmarkParameters<T> {
    /// The type of benchmark to run.
    pub benchmark_type: T,
    /// Optional OpenTelemetry configuration.
    pub otel: Option<OtelConfig>,
    /// The committee size.
    pub nodes: usize,
    /// The number of additional gas accounts to create.
    pub additional_gas_accounts: usize,
    /// The number of (crash-)faults.
    pub faults: FaultsType,
    /// The total load (tx/s) to submit to the system.
    pub load: usize,
    /// The run interval of the benchmark. This can be either a duration (e.g.,
    /// 60s) or a transaction count (e.g., 100_000 txs).
    pub run_interval: RunInterval,
    /// AA workload: which authenticator kind to use.
    pub aa_authenticator: AuthenticatorKind,
    /// AA workload: whether the transactions should fail.
    pub should_fail: bool,
    /// AA workload: which authenticator kind to use.
    pub tx_payload_obj_type: TxPayloadObjType,
    /// Number of worker tasks inside stress.
    pub stress_num_workers: u64,
    /// In-flight ratio inside stress.
    pub stress_in_flight_ratio: u64,
    /// AA workload: split amount inside stress.
    pub aa_split_amount: u64,
    /// Stress client threads used for AA workload (bench keeps the old
    /// hardcoded behavior).
    pub stress_num_client_threads: u64,
    /// Stress server threads used for AA workload (bench keeps the old
    /// hardcoded behavior).
    pub stress_num_server_threads: u64,
    /// Flag indicating whether nodes should advertise their internal or public
    /// IP address for inter-node communication. When running the simulation
    /// in multiple regions, nodes need to use their public IPs to correctly
    /// communicate, however when a simulation is running in a single VPC,
    /// they should use their internal IPs to avoid paying for data sent between
    /// the nodes.
    pub use_internal_ip_address: bool,
    /// The topology of private network latencies, RandomGeographical,
    /// RandomClustered, HardCodedClustered, or Mainnet
    pub latency_topology: Option<TopologyLayout>,
    /// Maximum latency between two nodes in the private network.
    pub maximum_latency: u16,
    /// Specification of Perturbation imposed on the private network latencies.
    pub perturbation_spec: PerturbationSpec,
    /// Consensus Protocol used.
    pub consensus_protocol: ConsensusProtocol,
    /// Optional: Epoch duration in milliseconds, default is 1h
    pub epoch_duration_ms: Option<u64>,
    /// Max pipeline delay used only by starfish
    pub max_pipeline_delay: u32,
    /// Computed chain start timestamp (computed once in next() if
    /// use_current_timestamp_for_genesis is true)
    pub chain_start_timestamp_ms: Option<u64>,
    /// Shared counter hotness factor (0-100)
    pub shared_counter_hotness_factor: Option<u8>,
    /// Number of shared counters to use
    pub num_shared_counters: Option<usize>,
    /// Directory to store benchmark results
    pub benchmark_dir: PathBuf,
    /// Optional path to benchmark stats metadata for downloading stats after
    /// the run.
    pub benchmark_stats_path: Option<String>,
}

impl<T: BenchmarkType> Default for BenchmarkParameters<T> {
    fn default() -> Self {
        Self {
            benchmark_type: T::default(),
            otel: None,
            nodes: 4,
            additional_gas_accounts: 0,
            faults: FaultsType::default(),
            load: 500,
            run_interval: RunInterval::Time(Duration::from_secs(60)),
            aa_authenticator: AuthenticatorKind::default(),
            should_fail: false,
            tx_payload_obj_type: TxPayloadObjType::default(),
            aa_split_amount: 1_000,
            stress_num_workers: 2,
            stress_in_flight_ratio: 10,
            stress_num_client_threads: 8,
            stress_num_server_threads: 8,
            use_internal_ip_address: true,
            latency_topology: Some(TopologyLayout::Mainnet),
            perturbation_spec: PerturbationSpec::None,
            consensus_protocol: ConsensusProtocol::Starfish,
            maximum_latency: 400,
            epoch_duration_ms: None,
            max_pipeline_delay: 400,
            chain_start_timestamp_ms: None,
            shared_counter_hotness_factor: None,
            num_shared_counters: None,
            benchmark_dir: PathBuf::default(),
            benchmark_stats_path: None,
        }
    }
}

impl<T: BenchmarkType> Debug for BenchmarkParameters<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}-{:?}-{}-{}-{}-{:?}",
            self.benchmark_type,
            self.faults,
            self.nodes,
            self.load,
            self.use_internal_ip_address,
            self.chain_start_timestamp_ms,
        )
    }
}

impl<T> Display for BenchmarkParameters<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} nodes ({}) - {} tx/s (use internal IPs: {}; use current timestamp: {:?})",
            self.nodes,
            self.faults,
            self.load,
            self.use_internal_ip_address,
            self.chain_start_timestamp_ms,
        )
    }
}

impl<T> BenchmarkParameters<T> {
    /// Make a new benchmark parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        benchmark_type: T,
        otel: Option<OtelConfig>,
        nodes: usize,
        additional_gas_accounts: usize,
        faults: FaultsType,
        load: usize,
        run_interval: RunInterval,
        aa_authenticator: AuthenticatorKind,
        should_fail: bool,
        tx_payload_obj_type: TxPayloadObjType,
        stress_num_workers: u64,
        aa_split_amount: u64,
        stress_in_flight_ratio: u64,
        stress_num_client_threads: u64,
        stress_num_server_threads: u64,
        use_internal_ip_address: bool,
        latency_topology: Option<TopologyLayout>,
        perturbation_spec: PerturbationSpec,
        maximum_latency: u16,
        epoch_duration_ms: Option<u64>,
        consensus_protocol: ConsensusProtocol,
        max_pipeline_delay: u32,
        chain_start_timestamp_ms: Option<u64>,
        shared_counter_hotness_factor: Option<u8>,
        num_shared_counters: Option<usize>,
        benchmark_dir: PathBuf,
        benchmark_stats_path: Option<String>,
    ) -> Self {
        Self {
            benchmark_type,
            otel,
            nodes,
            additional_gas_accounts,
            faults,
            load,
            run_interval,
            aa_authenticator,
            should_fail,
            tx_payload_obj_type,
            aa_split_amount,
            stress_num_workers,
            stress_in_flight_ratio,
            stress_num_client_threads,
            stress_num_server_threads,
            use_internal_ip_address,

            latency_topology,
            perturbation_spec,
            consensus_protocol,
            maximum_latency,
            epoch_duration_ms,
            max_pipeline_delay,
            chain_start_timestamp_ms,
            shared_counter_hotness_factor,
            num_shared_counters,
            benchmark_dir,
            benchmark_stats_path,
        }
    }
}

/// The load type to submit to the nodes.
pub enum LoadType {
    /// Submit a fixed set of loads (one per benchmark run).
    Fixed(Vec<usize>),

    /// Search for the breaking point of the L-graph.
    // TODO: Doesn't work very well, use tps regression as additional signal.
    Search {
        /// The initial load to test (and use a baseline).
        starting_load: usize,
        /// The maximum number of iterations before converging on a breaking
        /// point.
        max_iterations: usize,
    },
}

/// Generate benchmark parameters (one set of parameters per run).
// TODO: The rusty thing to do would be to implement Iter.
pub struct BenchmarkParametersGenerator<T> {
    /// The type of benchmark to run.
    benchmark_type: T,
    /// Optional OpenTelemetry configuration.
    otel: Option<OtelConfig>,
    /// The committee size.
    pub nodes: usize,
    /// The number of additional clients.
    pub additional_gas_accounts: usize,
    /// The load type.
    load_type: LoadType,
    /// The number of faulty nodes.
    pub faults: FaultsType,
    /// The duration of the benchmark.
    run_interval: RunInterval,
    /// The load of the next benchmark run.
    next_load: Option<usize>,
    /// Temporary hold a lower bound of the breaking point.
    lower_bound_result: Option<MeasurementsCollection<T>>,
    /// Temporary hold an upper bound of the breaking point.
    upper_bound_result: Option<MeasurementsCollection<T>>,
    /// The current number of iterations.
    iterations: usize,
    /// Flag indicating whether nodes should advertise their internal or public
    /// IP address for inter-node communication.
    pub use_internal_ip_address: bool,

    /// AA workload authenticator.
    aa_authenticator: AuthenticatorKind,

    /// AA workload: whether the transactions should fail.
    should_fail: bool,

    /// Type of object transaction uses - owned or shared.
    tx_payload_obj_type: TxPayloadObjType,

    /// Number of worker tasks inside stress.
    stress_num_workers: u64,

    /// AA workload: split amount inside stress.
    aa_split_amount: u64,

    /// In-flight ratio inside stress.
    stress_in_flight_ratio: u64,

    /// Stress threads used for AA.
    stress_num_client_threads: u64,
    stress_num_server_threads: u64,

    /// The topology of private network latencies, RandomGeographical,
    /// RandomClustered, HardCodedClustered, or Mainnet
    pub latency_topology: Option<TopologyLayout>,
    /// Maximum latency between two nodes in the private network.
    pub maximum_latency: u16,
    /// Specification of Perturbation imposed on the private network latencies.
    pub perturbation_spec: PerturbationSpec,
    /// Consensus Protocol used.
    pub consensus_protocol: ConsensusProtocol,
    /// Optional: Epoch duration in milliseconds, default is 1h
    epoch_duration_ms: Option<u64>,
    /// Maximum pipeline delay.
    pub max_pipeline_delay: u32,
    /// Use current system time as genesis chain start timestamp instead of 0
    use_current_timestamp_for_genesis: bool,
    /// Shared counter hotness factor (0-100)
    shared_counter_hotness_factor: Option<u8>,
    /// Number of shared counters to use
    num_shared_counters: Option<usize>,
    /// Path for the benchmark stats metadata to be downloaded after the run
    benchmark_stats_path: Option<String>,
}

impl<T: BenchmarkType> Iterator for BenchmarkParametersGenerator<T> {
    type Item = BenchmarkParameters<T>;

    /// Return the next set of benchmark parameters to run.
    fn next(&mut self) -> Option<Self::Item> {
        // Compute timestamp once if needed
        let chain_start_timestamp_ms = if self.use_current_timestamp_for_genesis {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            )
        } else {
            None
        };

        self.next_load.map(|load| {
            BenchmarkParameters::new(
                self.benchmark_type.clone(),
                self.otel.clone(),
                self.nodes,
                self.additional_gas_accounts,
                self.faults.clone(),
                load,
                self.run_interval,
                self.aa_authenticator,
                self.should_fail,
                self.tx_payload_obj_type,
                self.stress_num_workers,
                self.aa_split_amount,
                self.stress_in_flight_ratio,
                self.stress_num_client_threads,
                self.stress_num_server_threads,
                self.use_internal_ip_address,
                self.latency_topology.clone(),
                self.perturbation_spec.clone(),
                self.maximum_latency,
                self.epoch_duration_ms,
                self.consensus_protocol.clone(),
                self.max_pipeline_delay,
                chain_start_timestamp_ms,
                self.shared_counter_hotness_factor,
                self.num_shared_counters,
                PathBuf::default(),
                self.benchmark_stats_path.clone(),
            )
        })
    }
}

impl<T: BenchmarkType> BenchmarkParametersGenerator<T> {
    /// The default benchmark run interval.
    const DEFAULT_RUN_INTERVAL: RunInterval = RunInterval::Time(Duration::from_secs(180));

    /// make a new generator.
    pub fn new(
        nodes: usize,
        additional_gas_accounts: usize,
        mut load_type: LoadType,
        use_internal_ip_address: bool,
    ) -> Self {
        let next_load = match &mut load_type {
            LoadType::Fixed(loads) => {
                if loads.is_empty() {
                    None
                } else {
                    Some(loads.remove(0))
                }
            }
            LoadType::Search { starting_load, .. } => Some(*starting_load),
        };
        Self {
            benchmark_type: T::default(),
            otel: None,
            nodes,
            additional_gas_accounts,
            load_type,
            faults: FaultsType::default(),
            run_interval: Self::DEFAULT_RUN_INTERVAL,
            next_load,
            lower_bound_result: None,
            upper_bound_result: None,
            iterations: 0,
            use_internal_ip_address,
            perturbation_spec: PerturbationSpec::None,
            latency_topology: Some(TopologyLayout::Mainnet),
            consensus_protocol: ConsensusProtocol::Starfish,
            maximum_latency: 400,
            epoch_duration_ms: None,
            use_current_timestamp_for_genesis: false,
            max_pipeline_delay: 400,
            shared_counter_hotness_factor: None,
            num_shared_counters: None,
            aa_authenticator: AuthenticatorKind::default(),
            should_fail: false,
            tx_payload_obj_type: TxPayloadObjType::default(),
            stress_num_workers: 2,
            stress_in_flight_ratio: 5,
            aa_split_amount: 1_000,
            stress_num_client_threads: 8,
            stress_num_server_threads: 8,
            benchmark_stats_path: None,
        }
    }

    pub fn with_aa_authenticator(mut self, aa_authenticator: AuthenticatorKind) -> Self {
        self.aa_authenticator = aa_authenticator;
        self
    }

    pub fn with_should_fail(mut self, should_fail: bool) -> Self {
        self.should_fail = should_fail;
        self
    }

    pub fn with_tx_payload_obj_type(mut self, tx_payload_obj_type: TxPayloadObjType) -> Self {
        self.tx_payload_obj_type = tx_payload_obj_type;
        self
    }

    pub fn with_stress_num_workers(mut self, stress_num_workers: u64) -> Self {
        self.stress_num_workers = stress_num_workers;
        self
    }

    pub fn with_aa_split_amount(mut self, aa_split_amount: u64) -> Self {
        self.aa_split_amount = aa_split_amount;
        self
    }

    pub fn with_stress_in_flight_ratio(mut self, stress_in_flight_ratio: u64) -> Self {
        self.stress_in_flight_ratio = stress_in_flight_ratio;
        self
    }

    pub fn with_stress_client_threads(mut self, stress_num_client_threads: u64) -> Self {
        self.stress_num_client_threads = stress_num_client_threads;
        self
    }

    pub fn with_stress_server_threads(mut self, stress_num_server_threads: u64) -> Self {
        self.stress_num_server_threads = stress_num_server_threads;
        self
    }

    /// Set the benchmark type.
    pub fn with_benchmark_type(mut self, benchmark_type: T) -> Self {
        self.benchmark_type = benchmark_type;
        self
    }

    /// Set crash-recovery pattern and the number of faulty nodes.
    pub fn with_faults(mut self, faults: FaultsType) -> Self {
        self.faults = faults;
        self
    }

    /// Set a custom benchmark run interval.
    pub fn with_custom_run_interval(mut self, run_interval: RunInterval) -> Self {
        self.run_interval = run_interval;
        self
    }

    pub fn with_perturbation_spec(mut self, perturbation_spec: PerturbationSpec) -> Self {
        self.perturbation_spec = perturbation_spec;
        self
    }

    pub fn with_latency_topology(mut self, latency_topology: Option<TopologyLayout>) -> Self {
        self.latency_topology = latency_topology;
        self
    }

    pub fn with_consensus_protocol(mut self, consensus_protocol: ConsensusProtocol) -> Self {
        self.consensus_protocol = consensus_protocol;
        self
    }

    pub fn with_max_latency(mut self, max_latency: u16) -> Self {
        self.maximum_latency = max_latency;
        self
    }

    pub fn with_epoch_duration(mut self, epoch_duration_ms: Option<u64>) -> Self {
        self.epoch_duration_ms = epoch_duration_ms;
        self
    }

    pub fn with_max_pipeline_delay(mut self, max_pipeline_delay: u32) -> Self {
        self.max_pipeline_delay = max_pipeline_delay;
        self
    }

    pub fn with_current_timestamp_for_genesis(mut self, use_current_timestamp: bool) -> Self {
        self.use_current_timestamp_for_genesis = use_current_timestamp;
        self
    }

    pub fn with_shared_counter_hotness_factor(mut self, factor: u8) -> Self {
        self.shared_counter_hotness_factor = Some(factor);
        self
    }

    pub fn with_num_shared_counters(mut self, counters: usize) -> Self {
        self.num_shared_counters = Some(counters);
        self
    }

    pub fn with_benchmark_stats_path(mut self, path: Option<String>) -> Self {
        self.benchmark_stats_path = path;
        self
    }

    /// Detects whether the latest benchmark parameters run the system out of
    /// capacity.
    fn out_of_capacity(
        last_result: &MeasurementsCollection<T>,
        new_result: &MeasurementsCollection<T>,
    ) -> bool {
        // We consider the system is out of capacity if the latency increased by over 5x
        // with respect to the latest run.
        let threshold = last_result.aggregate_average_latency() * 5;
        let high_latency = new_result.aggregate_average_latency() > threshold;

        // Or if the throughput is less than 2/3 of the input rate.
        let last_load = new_result.transaction_load() as u64;
        let no_throughput_increase = new_result.aggregate_tps() < (2 * last_load / 3);

        high_latency || no_throughput_increase
    }

    /// Register a new benchmark measurements collection. These results are used
    /// to determine whether the system reached its breaking point.
    pub fn register_result(&mut self, result: MeasurementsCollection<T>) {
        self.next_load = match &mut self.load_type {
            LoadType::Fixed(loads) => {
                if loads.is_empty() {
                    None
                } else {
                    Some(loads.remove(0))
                }
            }
            LoadType::Search { max_iterations, .. } => {
                // Terminate the search.
                if self.iterations >= *max_iterations {
                    None

                // Search for the breaking point.
                } else {
                    self.iterations += 1;
                    match (&mut self.lower_bound_result, &mut self.upper_bound_result) {
                        (None, None) => {
                            let next = result.transaction_load() * 2;
                            self.lower_bound_result = Some(result);
                            Some(next)
                        }
                        (Some(lower), None) => {
                            if Self::out_of_capacity(lower, &result) {
                                let next =
                                    (lower.transaction_load() + result.transaction_load()) / 2;
                                self.upper_bound_result = Some(result);
                                Some(next)
                            } else {
                                let next = result.transaction_load() * 2;
                                *lower = result;
                                Some(next)
                            }
                        }
                        (Some(lower), Some(upper)) => {
                            if Self::out_of_capacity(lower, &result) {
                                *upper = result;
                            } else {
                                *lower = result;
                            }
                            Some((lower.transaction_load() + upper.transaction_load()) / 2)
                        }
                        _ => panic!("Benchmark parameters generator is in an incoherent state"),
                    }
                }
            }
        };
    }
}

#[cfg(test)]
pub mod test {
    use std::{collections::HashMap, fmt::Display, str::FromStr};

    use serde::{Deserialize, Serialize};

    use super::{BenchmarkParametersGenerator, BenchmarkType, LoadType};
    use crate::{
        measurement::{Measurement, MeasurementsCollection},
        settings::Settings,
    };

    /// Mock benchmark type for unit tests.
    #[derive(
        Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Default,
    )]
    pub struct TestBenchmarkType;

    impl Display for TestBenchmarkType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TestBenchmarkType")
        }
    }

    impl FromStr for TestBenchmarkType {
        type Err = ();

        fn from_str(_s: &str) -> Result<Self, Self::Err> {
            Ok(Self {})
        }
    }

    impl BenchmarkType for TestBenchmarkType {}

    #[test]
    fn set_lower_bound() {
        let settings = Settings::new_for_test();
        let nodes = 4;
        let additional_gas_accounts = 0;
        let load = LoadType::Search {
            starting_load: 100,
            max_iterations: 10,
        };
        let mut generator = BenchmarkParametersGenerator::<TestBenchmarkType>::new(
            nodes,
            additional_gas_accounts,
            load,
            true,
        );
        let parameters = generator.next().unwrap();

        let collection = MeasurementsCollection::new(&settings, parameters);
        generator.register_result(collection);

        let next_parameters = generator.next();
        assert!(next_parameters.is_some());
        assert_eq!(next_parameters.unwrap().load, 200);

        assert!(generator.lower_bound_result.is_some());
        assert_eq!(
            generator.lower_bound_result.unwrap().transaction_load(),
            100
        );
        assert!(generator.upper_bound_result.is_none());
    }

    #[test]
    fn set_upper_bound() {
        let settings = Settings::new_for_test();
        let nodes = 4;
        let additional_gas_accounts = 0;
        let load = LoadType::Search {
            starting_load: 100,
            max_iterations: 10,
        };
        let mut generator = BenchmarkParametersGenerator::<TestBenchmarkType>::new(
            nodes,
            additional_gas_accounts,
            load,
            true,
        );
        let first_parameters = generator.next().unwrap();

        // Register a first result (zero latency). This sets the lower bound.
        let collection = MeasurementsCollection::new(&settings, first_parameters);
        generator.register_result(collection);
        let second_parameters = generator.next().unwrap();

        // Register a second result (with positive latency). This sets the upper bound.
        let mut collection = MeasurementsCollection::new(&settings, second_parameters);
        let measurement = Measurement::new_for_test("transfer_object".to_string());
        let workload_map = HashMap::from([("transfer_object".to_string(), vec![measurement])]);
        collection.scrapers.insert(1, workload_map);
        generator.register_result(collection);

        // Ensure the next load is between the upper and the lower bound.
        let third_parameters = generator.next();
        assert!(third_parameters.is_some());
        assert_eq!(third_parameters.unwrap().load, 150);

        assert!(generator.lower_bound_result.is_some());
        assert_eq!(
            generator.lower_bound_result.unwrap().transaction_load(),
            100
        );
        assert!(generator.upper_bound_result.is_some());
        assert_eq!(
            generator.upper_bound_result.unwrap().transaction_load(),
            200
        );
    }

    #[test]
    fn max_iterations() {
        let settings = Settings::new_for_test();
        let nodes = 4;
        let additional_gas_accounts = 0;
        let load = LoadType::Search {
            starting_load: 100,
            max_iterations: 0,
        };
        let mut generator = BenchmarkParametersGenerator::<TestBenchmarkType>::new(
            nodes,
            additional_gas_accounts,
            load,
            true,
        );
        let parameters = generator.next().unwrap();

        let collection = MeasurementsCollection::new(&settings, parameters);
        generator.register_result(collection);

        let next_parameters = generator.next();
        assert!(next_parameters.is_none());
    }
}
