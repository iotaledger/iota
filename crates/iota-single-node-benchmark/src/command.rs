// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use strum::EnumIter;

#[derive(Parser)]
#[command(
    name = "iota-single-node-benchmark",
    about = "Benchmark a single validator node",
    author,
    version
)]
pub struct Command {
    #[arg(
        long,
        default_value_t = 500000,
        help = "Number of transactions to submit"
    )]
    pub tx_count: u64,
    #[arg(
        long,
        default_value_t = 100,
        help = "Number of transactions in a consensus commit/checkpoint"
    )]
    pub checkpoint_size: usize,
    #[arg(
        long,
        help = "Whether to print out a sample transaction and effects that is going to be benchmarked on"
    )]
    pub print_sample_tx: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "If true, skip signing on the validators, instead, creating certificates directly using validator secrets"
    )]
    pub skip_signing: bool,
    #[arg(
        long,
        help = "If set, write one JSON line per executed transaction (transaction digest, \
        measured wall-clock nanoseconds, and the full resource profile) to this file. \
        The first line is a metadata record describing the run."
    )]
    pub profile_output: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = false,
        help = "Execute transactions one at a time instead of concurrently, so \
        per-transaction wall-clock measurements are not contaminated by contention"
    )]
    pub sequential: bool,
    #[arg(
        long,
        default_value_t = 0,
        help = "Cap the number of transactions executing at once (0 = unbounded, \
        the default all-at-once behavior). Set to N to measure per-transaction \
        wall-clock under exactly N-way contention; --concurrency 1 is equivalent \
        to --sequential."
    )]
    pub concurrency: usize,
    #[arg(
        long,
        default_value = "baseline",
        ignore_case = true,
        help = "Which component to benchmark"
    )]
    pub component: Component,
    #[arg(
        long,
        default_value_t = 0,
        help = "If nonzero, sustained mode: run rounds of the workload for this many seconds\
        against the real store, reusing accounts across rounds. Requires --db-path;\
        baseline component only."
    )]
    pub duration_secs: u64,
    #[arg(
        long,
        help = "Persistent store directory (default: a fresh temporary directory)"
    )]
    pub db_path: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = false,
        help = "Enable RocksDB write stalls, disabled by default in the test store.\
        The stall onset is the signal the write budget is calibrated against."
    )]
    pub enable_write_stall: bool,
    #[arg(
        long,
        help = "Write one JSON line per sustained-mode round to this file"
    )]
    pub stats_output: Option<PathBuf>,
    #[arg(
        long,
        help = "Write resident-memory readings for the measured phase (baseline,\
        peak, delta in bytes) to this file as JSON"
    )]
    pub rss_output: Option<PathBuf>,
    #[command(subcommand)]
    pub workload: WorkloadKind,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct MixtureEntry {
    #[serde(default = "default_weight")]
    pub weight: u64,
    pub params: PtbParams,
}

fn default_weight() -> u64 {
    1
}

pub fn load_mixture(spec_file: &PathBuf) -> Vec<MixtureEntry> {
    let text = std::fs::read_to_string(spec_file)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", spec_file.display()));
    let entries: Vec<MixtureEntry> = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("invalid mixture spec {}: {e}", spec_file.display()));
    assert!(!entries.is_empty(), "mixture spec is empty");
    for entry in &entries {
        assert!(entry.weight > 0, "mixture weights must be positive");
        assert_eq!(
            entry.params.num_shared_objects, 0,
            "shared objects are not supported in mixtures"
        );
    }
    entries
}

/// Everything `run_benchmark` needs besides the workload and component.
#[derive(Clone, Debug)]
pub struct BenchmarkConfig {
    pub checkpoint_size: usize,
    pub print_sample_tx: bool,
    pub skip_signing: bool,
    pub sequential: bool,
    pub concurrency: usize,
    pub duration_secs: u64,
    pub db_path: Option<PathBuf>,
    pub enable_write_stall: bool,
    pub stats_output: Option<PathBuf>,
    pub rss_output: Option<PathBuf>,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            checkpoint_size: 100,
            print_sample_tx: false,
            skip_signing: false,
            sequential: false,
            concurrency: 0,
            duration_secs: 0,
            db_path: None,
            enable_write_stall: false,
            stats_output: None,
            rss_output: None,
        }
    }
}

#[derive(Copy, Clone, EnumIter, ValueEnum)]
pub enum Component {
    ExecutionOnly,
    /// Baseline includes the execution and storage layer only.
    Baseline,
    /// On top of Baseline, this schedules transactions through the transaction
    /// manager.
    WithTxManager,
    /// This goes through the `handle_certificate` entry point on
    /// authority_server, which includes certificate verification,
    /// transaction manager, as well as a noop consensus layer. The noop
    /// consensus layer does absolutely nothing when receiving a transaction in
    /// consensus.
    ValidatorWithoutConsensus,
    /// Similar to ValidatorWithNoopConsensus, but the consensus layer contains
    /// a fake consensus protocol that basically sequences transactions in
    /// order. It then verify the transaction and store the sequenced
    /// transactions into the store. It covers the consensus-independent
    /// portion of the code in consensus handler.
    ValidatorWithFakeConsensus,
    /// Benchmark only validator signing component: `handle_transaction`.
    TxnSigning,
    /// Benchmark the checkpoint executor by constructing a full epoch of
    /// checkpoints, execute all transactions in them and measure time.
    CheckpointExecutor,
}

#[derive(Subcommand, Clone)]
pub enum WorkloadKind {
    PTB(PtbParams),
    /// A weighted mixture of PTB shapes interleaved within one run, so shape
    /// effects are not confounded with run-level conditions.
    Mixed {
        #[arg(
            long,
            help = "JSON file: a list of {\"weight\": w, \"params\": {PtbParams fields}}.\
            Omitted params fields take their defaults. Shared objects are not\
            supported in mixtures."
        )]
        spec_file: PathBuf,
    },
    Publish {
        #[arg(
            long,
            help = "Path to the manifest file that describe the package dependencies.\
            Follow examples in the tests directory to see how to set up the manifest file.\
            The manifest file is a json file that contains a list of dependent packages that need to\
            be published first, as well as the root package that will be benchmarked on. Each package\
            can be either in source code or bytecode form. If it is in source code form, the benchmark\
            will compile the package first before publishing it."
        )]
        manifest_file: PathBuf,
    },
}

#[derive(Clone, Debug, clap::Args, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PtbParams {
    #[arg(
        long,
        default_value_t = 0,
        help = "Number of address owned input objects per transaction.\
            This represents the amount of DB reads per transaction prior to execution."
    )]
    pub num_transfers: u64,
    #[arg(
        long,
        default_value_t = false,
        help = "When transferring an object, whether to use native TransferObjecet command, or to use Move code for the transfer"
    )]
    pub use_native_transfer: bool,
    #[arg(
        long,
        default_value_t = 0,
        help = "Number of dynamic fields read per transaction.\
            This represents the amount of runtime DB reads per transaction during execution."
    )]
    pub num_dynamic_fields: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Payload bytes stored in each dynamic-field child object"
    )]
    pub dynamic_field_size: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Number of pre-existing dynamic-field children to delete per transaction.\
            Requires --num-dynamic-fields at least this large. Real deletions of\
            pre-existing objects (tombstones)."
    )]
    pub num_deletes: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Computation intensity per transaction.\
            The transaction computes the n-th Fibonacci number \
            specified by this parameter * 100."
    )]
    pub computation: u32,
    #[arg(
        long,
        default_value_t = 0,
        help = "Whether to use shared objects in the transaction.\
            If 0, no shared objects will be used.\
            Otherwise `v` shared objects will be created and each transaction will use these `v` shared objects."
    )]
    pub num_shared_objects: usize,
    #[arg(
        long,
        default_value_t = 0,
        help = "How many NFTs to mint/transfer during the transaction.\
            If 0, no NFTs will be minted.\
            Otherwise `v` NFTs with the specified size will be created and transferred to the sender"
    )]
    pub num_mints: u16,
    #[arg(
        long,
        default_value_t = 32,
        help = "Size of the Move contents of the NFT to be minted, in bytes.\
            Defaults to 32 bytes (i.e., NFT with ID only)."
    )]
    pub nft_size: u16,
    #[arg(
        long,
        help = "If true, call a single batch_mint Move function.\
            Otherwise, batch via a PTB with multiple commands"
    )]
    pub use_batch_mint: bool,
    #[arg(
        long,
        default_value_t = 0,
        help = "Iterations of tight scalar arithmetic (high instruction count, minimal stack bytes)"
    )]
    pub scalar_ops: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Iterations of pack/unpack (high operand-stack push count, small values)"
    )]
    pub push_pop_ops: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Iterations of moving a large vector (high operand-stack byte flow, few instructions)"
    )]
    pub vector_move_ops: u64,
    #[arg(
        long,
        default_value_t = 4096,
        help = "Element count of the vector moved by --vector-move-ops"
    )]
    pub vector_move_size: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Bytes grown into a vector held in a local (locals-memory peak)"
    )]
    pub locals_bytes: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Depth of a nested-struct tree held live during execution"
    )]
    pub tree_depth: u64,
    #[arg(
        long,
        default_value_t = 2,
        help = "Children per node of the struct tree"
    )]
    pub tree_width: u64,
    #[arg(long, default_value_t = 0, help = "Hash-native calls per transaction")]
    pub num_hashes: u64,
    #[arg(
        long,
        value_enum,
        default_value_t = HashFamily::Sha2_256,
        help = "Which hash native --num-hashes loops"
    )]
    pub hash_family: HashFamily,
    #[arg(long, default_value_t = 64, help = "Input bytes per hash call")]
    pub hash_input_size: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Signature-verification native calls per transaction (valid fixtures generated at setup)"
    )]
    pub num_sig_verifies: u64,
    #[arg(
        long,
        value_enum,
        default_value_t = SigScheme::Ed25519,
        help = "Which signature scheme --num-sig-verifies loops"
    )]
    pub sig_scheme: SigScheme,
    #[arg(
        long,
        default_value_t = 64,
        help = "Signed message bytes for --num-sig-verifies"
    )]
    pub sig_msg_size: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "ECVRF proof verifications per transaction (valid fixture generated at setup)"
    )]
    pub num_ecvrf_verifies: u64,
    #[arg(
        long,
        default_value_t = 64,
        help = "VRF input (alpha string) bytes for --num-ecvrf-verifies"
    )]
    pub ecvrf_alpha_size: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Groth16 proof verifications per transaction (fixed valid proof\
        taken from the framework's unit tests)"
    )]
    pub num_groth16_verifies: u64,
    #[arg(
        long,
        value_enum,
        default_value_t = Groth16Curve::Bls12381,
        help = "Curve for --num-groth16-verifies"
    )]
    pub groth16_curve: Groth16Curve,
    #[arg(
        long,
        default_value_t = 0,
        help = "Poseidon hash calls per transaction"
    )]
    pub num_poseidon_hashes: u64,
    #[arg(
        long,
        default_value_t = 4,
        help = "Field elements hashed per poseidon call"
    )]
    pub poseidon_input_count: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "BLS12-381 group operations per transaction"
    )]
    pub num_group_ops: u64,
    #[arg(
        long,
        value_enum,
        default_value_t = GroupOp::G1Add,
        help = "Which group operation --num-group-ops loops"
    )]
    pub group_op: GroupOp,
    #[arg(
        long,
        default_value_t = 64,
        help = "Message bytes hashed per hash-to-g1 group operation"
    )]
    pub group_op_input_size: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Owned objects mutated in place per transaction (written objects with\
        no creation and no per-object native call); fixtures minted at setup"
    )]
    pub num_mutations: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Owned objects deleted per transaction via object::delete (tombstones\
        with no dynamic-field access); fixtures minted at setup"
    )]
    pub num_burns: u64,
    #[arg(
        long,
        default_value_t = 64,
        help = "Size of the owned-object fixtures for --num-mutations/--num-burns"
    )]
    pub owned_object_size: u16,
    #[arg(
        long,
        default_value_t = 0,
        help = "Iterations of 32-field unpack/repack (highest pushes-per-instruction)"
    )]
    pub high_push_ops: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Iterations of move/store chains (lowest pushes-per-instruction)"
    )]
    pub low_push_ops: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Calls into distinct generated packages per transaction (package-load\
        count/bytes decoupling); packages published at setup"
    )]
    pub num_packages_called: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Generated packages published at setup (default: --num-packages-called)"
    )]
    pub generated_package_count: u64,
    #[arg(
        long,
        default_value_t = 4096,
        help = "Constant-pool padding bytes per generated package"
    )]
    pub generated_package_bytes: u64,
    #[arg(long, default_value_t = 0, help = "Events emitted per transaction")]
    pub num_events: u64,
    #[arg(long, default_value_t = 32, help = "Payload bytes per emitted event")]
    pub event_size: u64,
}

/// Defaults matching the zero-work PTB, for tests that set only a few knobs.
impl Default for PtbParams {
    fn default() -> Self {
        Self {
            num_transfers: 0,
            use_native_transfer: false,
            num_dynamic_fields: 0,
            dynamic_field_size: 0,
            num_deletes: 0,
            computation: 0,
            num_shared_objects: 0,
            num_mints: 0,
            nft_size: 32,
            use_batch_mint: false,
            scalar_ops: 0,
            push_pop_ops: 0,
            vector_move_ops: 0,
            vector_move_size: 4096,
            locals_bytes: 0,
            tree_depth: 0,
            tree_width: 2,
            num_hashes: 0,
            hash_family: HashFamily::Sha2_256,
            hash_input_size: 64,
            num_sig_verifies: 0,
            sig_scheme: SigScheme::Ed25519,
            sig_msg_size: 64,
            num_ecvrf_verifies: 0,
            ecvrf_alpha_size: 64,
            num_groth16_verifies: 0,
            groth16_curve: Groth16Curve::Bls12381,
            num_poseidon_hashes: 0,
            poseidon_input_count: 4,
            num_group_ops: 0,
            group_op: GroupOp::G1Add,
            group_op_input_size: 64,
            num_mutations: 0,
            num_burns: 0,
            owned_object_size: 64,
            high_push_ops: 0,
            low_push_ops: 0,
            num_packages_called: 0,
            generated_package_count: 0,
            generated_package_bytes: 4096,
            num_events: 0,
            event_size: 32,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HashFamily {
    Sha2_256,
    Sha3_256,
    Keccak256,
    Blake2b256,
    HmacSha3_256,
}

#[derive(Copy, Clone, Debug, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SigScheme {
    Ed25519,
    BlsMinSig,
    BlsMinPk,
    Secp256k1,
    Secp256r1,
}

#[derive(Copy, Clone, Debug, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Groth16Curve {
    Bls12381,
    Bn254,
}

#[derive(Copy, Clone, Debug, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupOp {
    G1Add,
    G1Mul,
    HashToG1,
    Pairing,
}

impl WorkloadKind {
    pub(crate) fn gas_object_num_per_account(&self) -> u64 {
        match self {
            // Each transaction will always have 1 gas object, plus the number of owned objects that
            // will be transferred.
            WorkloadKind::PTB(params) => params.num_transfers + 1,
            WorkloadKind::Mixed { spec_file } => {
                load_mixture(spec_file)
                    .iter()
                    .map(|e| e.params.num_transfers)
                    .max()
                    .unwrap_or(0)
                    + 1
            }
            WorkloadKind::Publish { .. } => 1,
        }
    }
}
