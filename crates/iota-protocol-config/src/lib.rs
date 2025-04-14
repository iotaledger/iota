// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};

use clap::*;
use iota_protocol_config_macros::{ProtocolConfigAccessors, ProtocolConfigFeatureFlagsGetters};
use move_vm_config::verifier::VerifierConfig;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use tracing::{info, warn};

/// The minimum and maximum protocol versions supported by this build.
const MIN_PROTOCOL_VERSION: u64 = 1;
pub const MAX_PROTOCOL_VERSION: u64 = 6;

// Record history of protocol version allocations here:
//
// Version 1: Original version.
// Version 2: Don't redistribute slashed staking rewards, fix computation of
//            SystemEpochInfoEventV1.
// Version 3: Set the `relocate_event_module` to be true so that the module that
//            is associated as the "sending module" for an event is relocated by
//            linkage.
//            Add `Clock` based unlock to `Timelock` objects.
// Version 4: Introduce the `max_type_to_layout_nodes` config that sets the
//            maximal nodes which are allowed when converting to a type layout.
// Version 5: Introduce fixed protocol-defined base fee, IotaSystemStateV2 and
//            SystemEpochInfoEventV2.
//            Disallow adding new modules in `deps-only` packages.
//            Improve gas/wall time efficiency of some Move stdlib vector
//            functions.
//            Add new gas model version to update charging of functions.
//            Enable proper conversion of certain type argument errors in the
//            execution layer.
// Version 6: Bound size of values created in the adapter.

#[derive(Copy, Clone, Debug, Hash, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion(u64);

impl ProtocolVersion {
    // The minimum and maximum protocol version supported by this binary.
    // Counterintuitively, this constant may change over time as support for old
    // protocol versions is removed from the source. This ensures that when a
    // new network (such as a testnet) is created, its genesis committee will
    // use a protocol version that is actually supported by the binary.
    pub const MIN: Self = Self(MIN_PROTOCOL_VERSION);

    pub const MAX: Self = Self(MAX_PROTOCOL_VERSION);

    #[cfg(not(msim))]
    const MAX_ALLOWED: Self = Self::MAX;

    // We create one additional "fake" version in simulator builds so that we can
    // test upgrades.
    #[cfg(msim)]
    pub const MAX_ALLOWED: Self = Self(MAX_PROTOCOL_VERSION + 1);

    pub fn new(v: u64) -> Self {
        Self(v)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    // For serde deserialization - we don't define a Default impl because there
    // isn't a single universally appropriate default value.
    pub fn max() -> Self {
        Self::MAX
    }
}

impl From<u64> for ProtocolVersion {
    fn from(v: u64) -> Self {
        Self::new(v)
    }
}

impl std::ops::Sub<u64> for ProtocolVersion {
    type Output = Self;
    fn sub(self, rhs: u64) -> Self::Output {
        Self::new(self.0 - rhs)
    }
}

impl std::ops::Add<u64> for ProtocolVersion {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        Self::new(self.0 + rhs)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Copy, PartialOrd, Ord, Eq, ValueEnum)]
pub enum Chain {
    Mainnet,
    Testnet,
    Unknown,
}

impl Default for Chain {
    fn default() -> Self {
        Self::Unknown
    }
}

impl Chain {
    pub fn as_str(self) -> &'static str {
        match self {
            Chain::Mainnet => "mainnet",
            Chain::Testnet => "testnet",
            Chain::Unknown => "unknown",
        }
    }
}

pub struct Error(pub String);

// TODO: There are quite a few non boolean values in the feature flags. We
// should move them out.
/// Records on/off feature flags that may vary at each protocol version.
#[derive(Default, Clone, Serialize, Debug, ProtocolConfigFeatureFlagsGetters)]
struct FeatureFlags {
    // Add feature flags here, e.g.:
    // new_protocol_feature: bool,

    // Disables unnecessary invariant check in the Move VM when swapping the value out of a local
    // This flag is used to provide the correct MoveVM configuration for clients.
    #[serde(skip_serializing_if = "is_true")]
    disable_invariant_violation_check_in_swap_loc: bool,

    // If true, checks no extra bytes in a compiled module
    // This flag is used to provide the correct MoveVM configuration for clients.
    #[serde(skip_serializing_if = "is_true")]
    no_extraneous_module_bytes: bool,

    // Enable zklogin auth
    #[serde(skip_serializing_if = "is_false")]
    zklogin_auth: bool,

    // How we order transactions coming out of consensus before sending to execution.
    #[serde(skip_serializing_if = "ConsensusTransactionOrdering::is_none")]
    consensus_transaction_ordering: ConsensusTransactionOrdering,

    #[serde(skip_serializing_if = "is_false")]
    enable_jwk_consensus_updates: bool,

    // Enable bridge protocol
    #[serde(skip_serializing_if = "is_false")]
    bridge: bool,

    // If true, multisig containing zkLogin sig is accepted.
    #[serde(skip_serializing_if = "is_false")]
    accept_zklogin_in_multisig: bool,

    // If true, use the hardened OTW check
    // This flag is used to provide the correct MoveVM configuration for clients.
    #[serde(skip_serializing_if = "is_true")]
    hardened_otw_check: bool,

    // Enable the poseidon hash function
    #[serde(skip_serializing_if = "is_false")]
    enable_poseidon: bool,

    // Enable native function for msm.
    #[serde(skip_serializing_if = "is_false")]
    enable_group_ops_native_function_msm: bool,

    // Controls the behavior of per object congestion control in consensus handler.
    #[serde(skip_serializing_if = "PerObjectCongestionControlMode::is_none")]
    per_object_congestion_control_mode: PerObjectCongestionControlMode,

    // The consensus protocol to be used for the epoch.
    #[serde(skip_serializing_if = "ConsensusChoice::is_mysticeti")]
    consensus_choice: ConsensusChoice,

    // Consensus network to use.
    #[serde(skip_serializing_if = "ConsensusNetwork::is_tonic")]
    consensus_network: ConsensusNetwork,

    // Set the upper bound allowed for max_epoch in zklogin signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    zklogin_max_epoch_upper_bound_delta: Option<u64>,

    // Enable VDF
    #[serde(skip_serializing_if = "is_false")]
    enable_vdf: bool,

    // Enable passkey auth (SIP-9)
    #[serde(skip_serializing_if = "is_false")]
    passkey_auth: bool,

    // Rethrow type layout errors during serialization instead of trying to convert them.
    // This flag is used to provide the correct MoveVM configuration for clients.
    #[serde(skip_serializing_if = "is_true")]
    rethrow_serialization_type_layout_errors: bool,

    // Makes the event's sending module version-aware.
    #[serde(skip_serializing_if = "is_false")]
    relocate_event_module: bool,

    // Enable a protocol-defined base gas price for all transactions.
    #[serde(skip_serializing_if = "is_false")]
    protocol_defined_base_fee: bool,

    // Enable uncompressed group elements in BLS123-81 G1
    #[serde(skip_serializing_if = "is_false")]
    uncompressed_g1_group_elements: bool,

    // Disallow adding new modules in `deps-only` packages.
    #[serde(skip_serializing_if = "is_false")]
    disallow_new_modules_in_deps_only_packages: bool,

    // Enable v2 native charging for natives.
    #[serde(skip_serializing_if = "is_false")]
    native_charging_v2: bool,

    // Properly convert certain type argument errors in the execution layer.
    #[serde(skip_serializing_if = "is_false")]
    convert_type_argument_error: bool,
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_false(b: &bool) -> bool {
    !b
}

/// Ordering mechanism for transactions in one consensus output.
#[derive(Default, Copy, Clone, PartialEq, Eq, Serialize, Debug)]
pub enum ConsensusTransactionOrdering {
    /// No ordering. Transactions are processed in the order they appear in the
    /// consensus output.
    #[default]
    None,
    /// Order transactions by gas price, highest first.
    ByGasPrice,
}

impl ConsensusTransactionOrdering {
    pub fn is_none(&self) -> bool {
        matches!(self, ConsensusTransactionOrdering::None)
    }
}

// The config for per object congestion control in consensus handler.
#[derive(Default, Copy, Clone, PartialEq, Eq, Serialize, Debug)]
pub enum PerObjectCongestionControlMode {
    #[default]
    None, // No congestion control.
    TotalGasBudget, // Use txn gas budget as execution cost.
    TotalTxCount,   // Use total txn count as execution cost.
}

impl PerObjectCongestionControlMode {
    pub fn is_none(&self) -> bool {
        matches!(self, PerObjectCongestionControlMode::None)
    }
}

// Configuration options for consensus algorithm.
#[derive(Default, Copy, Clone, PartialEq, Eq, Serialize, Debug)]
pub enum ConsensusChoice {
    #[default]
    Mysticeti,
}

impl ConsensusChoice {
    pub fn is_mysticeti(&self) -> bool {
        matches!(self, ConsensusChoice::Mysticeti)
    }
}

// Configuration options for consensus network.
#[derive(Default, Copy, Clone, PartialEq, Eq, Serialize, Debug)]
pub enum ConsensusNetwork {
    #[default]
    Tonic,
}

impl ConsensusNetwork {
    pub fn is_tonic(&self) -> bool {
        matches!(self, ConsensusNetwork::Tonic)
    }
}

/// Constants that change the behavior of the protocol.
///
/// The value of each constant here must be fixed for a given protocol version.
/// To change the value of a constant, advance the protocol version, and add
/// support for it in `get_for_version` under the new version number.
/// (below).
///
/// To add a new field to this struct, use the following procedure:
/// - Advance the protocol version.
/// - Add the field as a private `Option<T>` to the struct.
/// - Initialize the field to `None` in prior protocol versions.
/// - Initialize the field to `Some(val)` for your new protocol version.
/// - Add a public getter that simply unwraps the field.
/// - Two public getters of the form `field(&self) -> field_type` and
///   `field_as_option(&self) -> Option<field_type>` will be automatically
///   generated for you.
/// Example for a field: `new_constant: Option<u64>`
/// ```rust,ignore
///      pub fn new_constant(&self) -> u64 {
///         self.new_constant.expect(Self::CONSTANT_ERR_MSG)
///     }
///      pub fn new_constant_as_option(&self) -> Option<u64> {
///         self.new_constant.expect(Self::CONSTANT_ERR_MSG)
///     }
/// ```
/// With `pub fn new_constant(&self) -> u64`, if the constant is accessed in a
/// protocol version in which it is not defined, the validator will crash.
/// (Crashing is necessary because this type of error would almost always result
/// in forking if not prevented here). If you don't want the validator to crash,
/// you can use the `pub fn new_constant_as_option(&self) -> Option<u64>`
/// getter, which will return `None` if the field is not defined at that
/// version.
/// - If you want a customized getter, you can add a method in the impl.
#[skip_serializing_none]
#[derive(Clone, Serialize, Debug, ProtocolConfigAccessors)]
pub struct ProtocolConfig {
    pub version: ProtocolVersion,

    feature_flags: FeatureFlags,

    // ==== Transaction input limits ====

    //
    /// Maximum serialized size of a transaction (in bytes).
    max_tx_size_bytes: Option<u64>,

    /// Maximum number of input objects to a transaction. Enforced by the
    /// transaction input checker
    max_input_objects: Option<u64>,

    /// Max size of objects a transaction can write to disk after completion.
    /// Enforce by the IOTA adapter. This is the sum of the serialized size
    /// of all objects written to disk. The max size of individual objects
    /// on the other hand is `max_move_object_size`.
    max_size_written_objects: Option<u64>,
    /// Max size of objects a system transaction can write to disk after
    /// completion. Enforce by the IOTA adapter. Similar to
    /// `max_size_written_objects` but for system transactions.
    max_size_written_objects_system_tx: Option<u64>,

    /// Maximum size of serialized transaction effects.
    max_serialized_tx_effects_size_bytes: Option<u64>,

    /// Maximum size of serialized transaction effects for system transactions.
    max_serialized_tx_effects_size_bytes_system_tx: Option<u64>,

    /// Maximum number of gas payment objects for a transaction.
    max_gas_payment_objects: Option<u32>,

    /// Maximum number of modules in a Publish transaction.
    max_modules_in_publish: Option<u32>,

    /// Maximum number of transitive dependencies in a package when publishing.
    max_package_dependencies: Option<u32>,

    /// Maximum number of arguments in a move call or a
    /// ProgrammableTransaction's TransferObjects command.
    max_arguments: Option<u32>,

    /// Maximum number of total type arguments, computed recursively.
    max_type_arguments: Option<u32>,

    /// Maximum depth of an individual type argument.
    max_type_argument_depth: Option<u32>,

    /// Maximum size of a Pure CallArg.
    max_pure_argument_size: Option<u32>,

    /// Maximum number of Commands in a ProgrammableTransaction.
    max_programmable_tx_commands: Option<u32>,

    // ==== Move VM, Move bytecode verifier, and execution limits ===

    //
    /// Maximum Move bytecode version the VM understands. All older versions are
    /// accepted.
    move_binary_format_version: Option<u32>,
    min_move_binary_format_version: Option<u32>,

    /// Configuration controlling binary tables size.
    binary_module_handles: Option<u16>,
    binary_struct_handles: Option<u16>,
    binary_function_handles: Option<u16>,
    binary_function_instantiations: Option<u16>,
    binary_signatures: Option<u16>,
    binary_constant_pool: Option<u16>,
    binary_identifiers: Option<u16>,
    binary_address_identifiers: Option<u16>,
    binary_struct_defs: Option<u16>,
    binary_struct_def_instantiations: Option<u16>,
    binary_function_defs: Option<u16>,
    binary_field_handles: Option<u16>,
    binary_field_instantiations: Option<u16>,
    binary_friend_decls: Option<u16>,
    binary_enum_defs: Option<u16>,
    binary_enum_def_instantiations: Option<u16>,
    binary_variant_handles: Option<u16>,
    binary_variant_instantiation_handles: Option<u16>,

    /// Maximum size of the `contents` part of an object, in bytes. Enforced by
    /// the IOTA adapter when effects are produced.
    max_move_object_size: Option<u64>,

    // TODO: Option<increase to 500 KB. currently, publishing a package > 500 KB exceeds the max
    // computation gas cost
    /// Maximum size of a Move package object, in bytes. Enforced by the IOTA
    /// adapter at the end of a publish transaction.
    max_move_package_size: Option<u64>,

    /// Max number of publish or upgrade commands allowed in a programmable
    /// transaction block.
    max_publish_or_upgrade_per_ptb: Option<u64>,

    /// Maximum gas budget in NANOS that a transaction can use.
    max_tx_gas: Option<u64>,

    /// Maximum amount of the proposed gas price in NANOS (defined in the
    /// transaction).
    max_gas_price: Option<u64>,

    /// The max computation bucket for gas. This is the max that can be charged
    /// for computation.
    max_gas_computation_bucket: Option<u64>,

    // Define the value used to round up computation gas charges
    gas_rounding_step: Option<u64>,

    /// Maximum number of nested loops. Enforced by the Move bytecode verifier.
    max_loop_depth: Option<u64>,

    /// Maximum number of type arguments that can be bound to generic type
    /// parameters. Enforced by the Move bytecode verifier.
    max_generic_instantiation_length: Option<u64>,

    /// Maximum number of parameters that a Move function can have. Enforced by
    /// the Move bytecode verifier.
    max_function_parameters: Option<u64>,

    /// Maximum number of basic blocks that a Move function can have. Enforced
    /// by the Move bytecode verifier.
    max_basic_blocks: Option<u64>,

    /// Maximum stack size value. Enforced by the Move bytecode verifier.
    max_value_stack_size: Option<u64>,

    /// Maximum number of "type nodes", a metric for how big a SignatureToken
    /// will be when expanded into a fully qualified type. Enforced by the Move
    /// bytecode verifier.
    max_type_nodes: Option<u64>,

    /// Maximum number of push instructions in one function. Enforced by the
    /// Move bytecode verifier.
    max_push_size: Option<u64>,

    /// Maximum number of struct definitions in a module. Enforced by the Move
    /// bytecode verifier.
    max_struct_definitions: Option<u64>,

    /// Maximum number of function definitions in a module. Enforced by the Move
    /// bytecode verifier.
    max_function_definitions: Option<u64>,

    /// Maximum number of fields allowed in a struct definition. Enforced by the
    /// Move bytecode verifier.
    max_fields_in_struct: Option<u64>,

    /// Maximum dependency depth. Enforced by the Move linker when loading
    /// dependent modules.
    max_dependency_depth: Option<u64>,

    /// Maximum number of Move events that a single transaction can emit.
    /// Enforced by the VM during execution.
    max_num_event_emit: Option<u64>,

    /// Maximum number of new IDs that a single transaction can create. Enforced
    /// by the VM during execution.
    max_num_new_move_object_ids: Option<u64>,

    /// Maximum number of new IDs that a single system transaction can create.
    /// Enforced by the VM during execution.
    max_num_new_move_object_ids_system_tx: Option<u64>,

    /// Maximum number of IDs that a single transaction can delete. Enforced by
    /// the VM during execution.
    max_num_deleted_move_object_ids: Option<u64>,

    /// Maximum number of IDs that a single system transaction can delete.
    /// Enforced by the VM during execution.
    max_num_deleted_move_object_ids_system_tx: Option<u64>,

    /// Maximum number of IDs that a single transaction can transfer. Enforced
    /// by the VM during execution.
    max_num_transferred_move_object_ids: Option<u64>,

    /// Maximum number of IDs that a single system transaction can transfer.
    /// Enforced by the VM during execution.
    max_num_transferred_move_object_ids_system_tx: Option<u64>,

    /// Maximum size of a Move user event. Enforced by the VM during execution.
    max_event_emit_size: Option<u64>,

    /// Maximum size of a Move user event. Enforced by the VM during execution.
    max_event_emit_size_total: Option<u64>,

    /// Maximum length of a vector in Move. Enforced by the VM during execution,
    /// and for constants, by the verifier.
    max_move_vector_len: Option<u64>,

    /// Maximum length of an `Identifier` in Move. Enforced by the bytecode
    /// verifier at signing.
    max_move_identifier_len: Option<u64>,

    /// Maximum depth of a Move value within the VM.
    max_move_value_depth: Option<u64>,

    /// Maximum number of variants in an enum. Enforced by the bytecode verifier
    /// at signing.
    max_move_enum_variants: Option<u64>,

    /// Maximum number of back edges in Move function. Enforced by the bytecode
    /// verifier at signing.
    max_back_edges_per_function: Option<u64>,

    /// Maximum number of back edges in Move module. Enforced by the bytecode
    /// verifier at signing.
    max_back_edges_per_module: Option<u64>,

    /// Maximum number of meter `ticks` spent verifying a Move function.
    /// Enforced by the bytecode verifier at signing.
    max_verifier_meter_ticks_per_function: Option<u64>,

    /// Maximum number of meter `ticks` spent verifying a Move function.
    /// Enforced by the bytecode verifier at signing.
    max_meter_ticks_per_module: Option<u64>,

    /// Maximum number of meter `ticks` spent verifying a Move package. Enforced
    /// by the bytecode verifier at signing.
    max_meter_ticks_per_package: Option<u64>,

    // === Object runtime internal operation limits ====
    // These affect dynamic fields

    //
    /// Maximum number of cached objects in the object runtime ObjectStore.
    /// Enforced by object runtime during execution
    object_runtime_max_num_cached_objects: Option<u64>,

    /// Maximum number of cached objects in the object runtime ObjectStore in
    /// system transaction. Enforced by object runtime during execution
    object_runtime_max_num_cached_objects_system_tx: Option<u64>,

    /// Maximum number of stored objects accessed by object runtime ObjectStore.
    /// Enforced by object runtime during execution
    object_runtime_max_num_store_entries: Option<u64>,

    /// Maximum number of stored objects accessed by object runtime ObjectStore
    /// in system transaction. Enforced by object runtime during execution
    object_runtime_max_num_store_entries_system_tx: Option<u64>,

    // === Execution gas costs ====

    //
    /// Base cost for any IOTA transaction
    base_tx_cost_fixed: Option<u64>,

    /// Additional cost for a transaction that publishes a package
    /// i.e., the base cost of such a transaction is base_tx_cost_fixed +
    /// package_publish_cost_fixed
    package_publish_cost_fixed: Option<u64>,

    /// Cost per byte of a Move call transaction
    /// i.e., the cost of such a transaction is base_cost +
    /// (base_tx_cost_per_byte * size)
    base_tx_cost_per_byte: Option<u64>,

    /// Cost per byte for a transaction that publishes a package
    package_publish_cost_per_byte: Option<u64>,

    // Per-byte cost of reading an object during transaction execution
    obj_access_cost_read_per_byte: Option<u64>,

    // Per-byte cost of writing an object during transaction execution
    obj_access_cost_mutate_per_byte: Option<u64>,

    // Per-byte cost of deleting an object during transaction execution
    obj_access_cost_delete_per_byte: Option<u64>,

    /// Per-byte cost charged for each input object to a transaction.
    /// Meant to approximate the cost of checking locks for each object
    // TODO: Option<I'm not sure that this cost makes sense. Checking locks is "free"
    // in the sense that an invalid tx that can never be committed/pay gas can
    // force validators to check an arbitrary number of locks. If those checks are
    // "free" for invalid transactions, why charge for them in valid transactions
    // TODO: Option<if we keep this, I think we probably want it to be a fixed cost rather
    // than a per-byte cost. checking an object lock should not require loading an
    // entire object, just consulting an ID -> tx digest map
    obj_access_cost_verify_per_byte: Option<u64>,

    // Maximal nodes which are allowed when converting to a type layout.
    max_type_to_layout_nodes: Option<u64>,

    // Maximal size in bytes that a PTB value can be
    max_ptb_value_size: Option<u64>,

    // === Gas version. gas model ===

    //
    /// Gas model version, what code we are using to charge gas
    gas_model_version: Option<u64>,

    // === Storage gas costs ===

    //
    /// Per-byte cost of storing an object in the IOTA global object store. Some
    /// of this cost may be refundable if the object is later freed
    obj_data_cost_refundable: Option<u64>,

    // Per-byte cost of storing an object in the IOTA transaction log (e.g., in
    // CertifiedTransactionEffects) This depends on the size of various fields including the
    // effects TODO: Option<I don't fully understand this^ and more details would be useful
    obj_metadata_cost_non_refundable: Option<u64>,

    // === Tokenomics ===

    // TODO: Option<this should be changed to u64.
    /// Sender of a txn that touches an object will get this percent of the
    /// storage rebate back. In basis point.
    storage_rebate_rate: Option<u64>,

    /// The share of rewards that will be slashed and redistributed is 50%.
    /// In basis point.
    reward_slashing_rate: Option<u64>,

    /// Unit storage gas price, Nanos per internal gas unit.
    storage_gas_price: Option<u64>,

    // Base gas price for computation gas, nanos per computation unit.
    base_gas_price: Option<u64>,

    /// The number of tokens minted as a validator subsidy per epoch.
    validator_target_reward: Option<u64>,

    // === Core Protocol ===

    //
    /// Max number of transactions per checkpoint.
    /// Note that this is a protocol constant and not a config as validators
    /// must have this set to the same value, otherwise they *will* fork.
    max_transactions_per_checkpoint: Option<u64>,

    /// Max size of a checkpoint in bytes.
    /// Note that this is a protocol constant and not a config as validators
    /// must have this set to the same value, otherwise they *will* fork.
    max_checkpoint_size_bytes: Option<u64>,

    /// A protocol upgrade always requires 2f+1 stake to agree. We support a
    /// buffer of additional stake (as a fraction of f, expressed in basis
    /// points) that is required before an upgrade can happen automatically.
    /// 10000bps would indicate that complete unanimity is required (all
    /// 3f+1 must vote), while 0bps would indicate that 2f+1 is sufficient.
    buffer_stake_for_protocol_upgrade_bps: Option<u64>,

    // === Native Function Costs ===

    // `address` module
    // Cost params for the Move native function `address::from_bytes(bytes: vector<u8>)`
    address_from_bytes_cost_base: Option<u64>,
    // Cost params for the Move native function `address::to_u256(address): u256`
    address_to_u256_cost_base: Option<u64>,
    // Cost params for the Move native function `address::from_u256(u256): address`
    address_from_u256_cost_base: Option<u64>,

    // `config` module
    // Cost params for the Move native function `read_setting_impl<Name: copy + drop + store,
    // SettingValue: key + store, SettingDataValue: store, Value: copy + drop + store,
    // >(config: address, name: address, current_epoch: u64): Option<Value>`
    config_read_setting_impl_cost_base: Option<u64>,
    config_read_setting_impl_cost_per_byte: Option<u64>,

    // `dynamic_field` module
    // Cost params for the Move native function `hash_type_and_key<K: copy + drop + store>(parent:
    // address, k: K): address`
    dynamic_field_hash_type_and_key_cost_base: Option<u64>,
    dynamic_field_hash_type_and_key_type_cost_per_byte: Option<u64>,
    dynamic_field_hash_type_and_key_value_cost_per_byte: Option<u64>,
    dynamic_field_hash_type_and_key_type_tag_cost_per_byte: Option<u64>,
    // Cost params for the Move native function `add_child_object<Child: key>(parent: address,
    // child: Child)`
    dynamic_field_add_child_object_cost_base: Option<u64>,
    dynamic_field_add_child_object_type_cost_per_byte: Option<u64>,
    dynamic_field_add_child_object_value_cost_per_byte: Option<u64>,
    dynamic_field_add_child_object_struct_tag_cost_per_byte: Option<u64>,
    // Cost params for the Move native function `borrow_child_object_mut<Child: key>(parent: &mut
    // UID, id: address): &mut Child`
    dynamic_field_borrow_child_object_cost_base: Option<u64>,
    dynamic_field_borrow_child_object_child_ref_cost_per_byte: Option<u64>,
    dynamic_field_borrow_child_object_type_cost_per_byte: Option<u64>,
    // Cost params for the Move native function `remove_child_object<Child: key>(parent: address,
    // id: address): Child`
    dynamic_field_remove_child_object_cost_base: Option<u64>,
    dynamic_field_remove_child_object_child_cost_per_byte: Option<u64>,
    dynamic_field_remove_child_object_type_cost_per_byte: Option<u64>,
    // Cost params for the Move native function `has_child_object(parent: address, id: address):
    // bool`
    dynamic_field_has_child_object_cost_base: Option<u64>,
    // Cost params for the Move native function `has_child_object_with_ty<Child: key>(parent:
    // address, id: address): bool`
    dynamic_field_has_child_object_with_ty_cost_base: Option<u64>,
    dynamic_field_has_child_object_with_ty_type_cost_per_byte: Option<u64>,
    dynamic_field_has_child_object_with_ty_type_tag_cost_per_byte: Option<u64>,

    // `event` module
    // Cost params for the Move native function `event::emit<T: copy + drop>(event: T)`
    event_emit_cost_base: Option<u64>,
    event_emit_value_size_derivation_cost_per_byte: Option<u64>,
    event_emit_tag_size_derivation_cost_per_byte: Option<u64>,
    event_emit_output_cost_per_byte: Option<u64>,

    //  `object` module
    // Cost params for the Move native function `borrow_uid<T: key>(obj: &T): &UID`
    object_borrow_uid_cost_base: Option<u64>,
    // Cost params for the Move native function `delete_impl(id: address)`
    object_delete_impl_cost_base: Option<u64>,
    // Cost params for the Move native function `record_new_uid(id: address)`
    object_record_new_uid_cost_base: Option<u64>,

    // Transfer
    // Cost params for the Move native function `transfer_impl<T: key>(obj: T, recipient: address)`
    transfer_transfer_internal_cost_base: Option<u64>,
    // Cost params for the Move native function `freeze_object<T: key>(obj: T)`
    transfer_freeze_object_cost_base: Option<u64>,
    // Cost params for the Move native function `share_object<T: key>(obj: T)`
    transfer_share_object_cost_base: Option<u64>,
    // Cost params for the Move native function
    // `receive_object<T: key>(p: &mut UID, recv: Receiving<T>T)`
    transfer_receive_object_cost_base: Option<u64>,

    // TxContext
    // Cost params for the Move native function `transfer_impl<T: key>(obj: T, recipient: address)`
    tx_context_derive_id_cost_base: Option<u64>,

    // Types
    // Cost params for the Move native function `is_one_time_witness<T: drop>(_: &T): bool`
    types_is_one_time_witness_cost_base: Option<u64>,
    types_is_one_time_witness_type_tag_cost_per_byte: Option<u64>,
    types_is_one_time_witness_type_cost_per_byte: Option<u64>,

    // Validator
    // Cost params for the Move native function `validate_metadata_bcs(metadata: vector<u8>)`
    validator_validate_metadata_cost_base: Option<u64>,
    validator_validate_metadata_data_cost_per_byte: Option<u64>,

    // Crypto natives
    crypto_invalid_arguments_cost: Option<u64>,
    // bls12381::bls12381_min_sig_verify
    bls12381_bls12381_min_sig_verify_cost_base: Option<u64>,
    bls12381_bls12381_min_sig_verify_msg_cost_per_byte: Option<u64>,
    bls12381_bls12381_min_sig_verify_msg_cost_per_block: Option<u64>,

    // bls12381::bls12381_min_pk_verify
    bls12381_bls12381_min_pk_verify_cost_base: Option<u64>,
    bls12381_bls12381_min_pk_verify_msg_cost_per_byte: Option<u64>,
    bls12381_bls12381_min_pk_verify_msg_cost_per_block: Option<u64>,

    // ecdsa_k1::ecrecover
    ecdsa_k1_ecrecover_keccak256_cost_base: Option<u64>,
    ecdsa_k1_ecrecover_keccak256_msg_cost_per_byte: Option<u64>,
    ecdsa_k1_ecrecover_keccak256_msg_cost_per_block: Option<u64>,
    ecdsa_k1_ecrecover_sha256_cost_base: Option<u64>,
    ecdsa_k1_ecrecover_sha256_msg_cost_per_byte: Option<u64>,
    ecdsa_k1_ecrecover_sha256_msg_cost_per_block: Option<u64>,

    // ecdsa_k1::decompress_pubkey
    ecdsa_k1_decompress_pubkey_cost_base: Option<u64>,

    // ecdsa_k1::secp256k1_verify
    ecdsa_k1_secp256k1_verify_keccak256_cost_base: Option<u64>,
    ecdsa_k1_secp256k1_verify_keccak256_msg_cost_per_byte: Option<u64>,
    ecdsa_k1_secp256k1_verify_keccak256_msg_cost_per_block: Option<u64>,
    ecdsa_k1_secp256k1_verify_sha256_cost_base: Option<u64>,
    ecdsa_k1_secp256k1_verify_sha256_msg_cost_per_byte: Option<u64>,
    ecdsa_k1_secp256k1_verify_sha256_msg_cost_per_block: Option<u64>,

    // ecdsa_r1::ecrecover
    ecdsa_r1_ecrecover_keccak256_cost_base: Option<u64>,
    ecdsa_r1_ecrecover_keccak256_msg_cost_per_byte: Option<u64>,
    ecdsa_r1_ecrecover_keccak256_msg_cost_per_block: Option<u64>,
    ecdsa_r1_ecrecover_sha256_cost_base: Option<u64>,
    ecdsa_r1_ecrecover_sha256_msg_cost_per_byte: Option<u64>,
    ecdsa_r1_ecrecover_sha256_msg_cost_per_block: Option<u64>,

    // ecdsa_r1::secp256k1_verify
    ecdsa_r1_secp256r1_verify_keccak256_cost_base: Option<u64>,
    ecdsa_r1_secp256r1_verify_keccak256_msg_cost_per_byte: Option<u64>,
    ecdsa_r1_secp256r1_verify_keccak256_msg_cost_per_block: Option<u64>,
    ecdsa_r1_secp256r1_verify_sha256_cost_base: Option<u64>,
    ecdsa_r1_secp256r1_verify_sha256_msg_cost_per_byte: Option<u64>,
    ecdsa_r1_secp256r1_verify_sha256_msg_cost_per_block: Option<u64>,

    // ecvrf::verify
    ecvrf_ecvrf_verify_cost_base: Option<u64>,
    ecvrf_ecvrf_verify_alpha_string_cost_per_byte: Option<u64>,
    ecvrf_ecvrf_verify_alpha_string_cost_per_block: Option<u64>,

    // ed25519
    ed25519_ed25519_verify_cost_base: Option<u64>,
    ed25519_ed25519_verify_msg_cost_per_byte: Option<u64>,
    ed25519_ed25519_verify_msg_cost_per_block: Option<u64>,

    // groth16::prepare_verifying_key
    groth16_prepare_verifying_key_bls12381_cost_base: Option<u64>,
    groth16_prepare_verifying_key_bn254_cost_base: Option<u64>,

    // groth16::verify_groth16_proof_internal
    groth16_verify_groth16_proof_internal_bls12381_cost_base: Option<u64>,
    groth16_verify_groth16_proof_internal_bls12381_cost_per_public_input: Option<u64>,
    groth16_verify_groth16_proof_internal_bn254_cost_base: Option<u64>,
    groth16_verify_groth16_proof_internal_bn254_cost_per_public_input: Option<u64>,
    groth16_verify_groth16_proof_internal_public_input_cost_per_byte: Option<u64>,

    // hash::blake2b256
    hash_blake2b256_cost_base: Option<u64>,
    hash_blake2b256_data_cost_per_byte: Option<u64>,
    hash_blake2b256_data_cost_per_block: Option<u64>,

    // hash::keccak256
    hash_keccak256_cost_base: Option<u64>,
    hash_keccak256_data_cost_per_byte: Option<u64>,
    hash_keccak256_data_cost_per_block: Option<u64>,

    // poseidon::poseidon_bn254
    poseidon_bn254_cost_base: Option<u64>,
    poseidon_bn254_cost_per_block: Option<u64>,

    // group_ops
    group_ops_bls12381_decode_scalar_cost: Option<u64>,
    group_ops_bls12381_decode_g1_cost: Option<u64>,
    group_ops_bls12381_decode_g2_cost: Option<u64>,
    group_ops_bls12381_decode_gt_cost: Option<u64>,
    group_ops_bls12381_scalar_add_cost: Option<u64>,
    group_ops_bls12381_g1_add_cost: Option<u64>,
    group_ops_bls12381_g2_add_cost: Option<u64>,
    group_ops_bls12381_gt_add_cost: Option<u64>,
    group_ops_bls12381_scalar_sub_cost: Option<u64>,
    group_ops_bls12381_g1_sub_cost: Option<u64>,
    group_ops_bls12381_g2_sub_cost: Option<u64>,
    group_ops_bls12381_gt_sub_cost: Option<u64>,
    group_ops_bls12381_scalar_mul_cost: Option<u64>,
    group_ops_bls12381_g1_mul_cost: Option<u64>,
    group_ops_bls12381_g2_mul_cost: Option<u64>,
    group_ops_bls12381_gt_mul_cost: Option<u64>,
    group_ops_bls12381_scalar_div_cost: Option<u64>,
    group_ops_bls12381_g1_div_cost: Option<u64>,
    group_ops_bls12381_g2_div_cost: Option<u64>,
    group_ops_bls12381_gt_div_cost: Option<u64>,
    group_ops_bls12381_g1_hash_to_base_cost: Option<u64>,
    group_ops_bls12381_g2_hash_to_base_cost: Option<u64>,
    group_ops_bls12381_g1_hash_to_cost_per_byte: Option<u64>,
    group_ops_bls12381_g2_hash_to_cost_per_byte: Option<u64>,
    group_ops_bls12381_g1_msm_base_cost: Option<u64>,
    group_ops_bls12381_g2_msm_base_cost: Option<u64>,
    group_ops_bls12381_g1_msm_base_cost_per_input: Option<u64>,
    group_ops_bls12381_g2_msm_base_cost_per_input: Option<u64>,
    group_ops_bls12381_msm_max_len: Option<u32>,
    group_ops_bls12381_pairing_cost: Option<u64>,
    group_ops_bls12381_g1_to_uncompressed_g1_cost: Option<u64>,
    group_ops_bls12381_uncompressed_g1_to_g1_cost: Option<u64>,
    group_ops_bls12381_uncompressed_g1_sum_base_cost: Option<u64>,
    group_ops_bls12381_uncompressed_g1_sum_cost_per_term: Option<u64>,
    group_ops_bls12381_uncompressed_g1_sum_max_terms: Option<u64>,

    // hmac::hmac_sha3_256
    hmac_hmac_sha3_256_cost_base: Option<u64>,
    hmac_hmac_sha3_256_input_cost_per_byte: Option<u64>,
    hmac_hmac_sha3_256_input_cost_per_block: Option<u64>,

    // zklogin::check_zklogin_id
    check_zklogin_id_cost_base: Option<u64>,
    // zklogin::check_zklogin_issuer
    check_zklogin_issuer_cost_base: Option<u64>,

    vdf_verify_vdf_cost: Option<u64>,
    vdf_hash_to_input_cost: Option<u64>,

    // Stdlib costs
    bcs_per_byte_serialized_cost: Option<u64>,
    bcs_legacy_min_output_size_cost: Option<u64>,
    bcs_failure_cost: Option<u64>,

    hash_sha2_256_base_cost: Option<u64>,
    hash_sha2_256_per_byte_cost: Option<u64>,
    hash_sha2_256_legacy_min_input_len_cost: Option<u64>,
    hash_sha3_256_base_cost: Option<u64>,
    hash_sha3_256_per_byte_cost: Option<u64>,
    hash_sha3_256_legacy_min_input_len_cost: Option<u64>,
    type_name_get_base_cost: Option<u64>,
    type_name_get_per_byte_cost: Option<u64>,

    string_check_utf8_base_cost: Option<u64>,
    string_check_utf8_per_byte_cost: Option<u64>,
    string_is_char_boundary_base_cost: Option<u64>,
    string_sub_string_base_cost: Option<u64>,
    string_sub_string_per_byte_cost: Option<u64>,
    string_index_of_base_cost: Option<u64>,
    string_index_of_per_byte_pattern_cost: Option<u64>,
    string_index_of_per_byte_searched_cost: Option<u64>,

    vector_empty_base_cost: Option<u64>,
    vector_length_base_cost: Option<u64>,
    vector_push_back_base_cost: Option<u64>,
    vector_push_back_legacy_per_abstract_memory_unit_cost: Option<u64>,
    vector_borrow_base_cost: Option<u64>,
    vector_pop_back_base_cost: Option<u64>,
    vector_destroy_empty_base_cost: Option<u64>,
    vector_swap_base_cost: Option<u64>,
    debug_print_base_cost: Option<u64>,
    debug_print_stack_trace_base_cost: Option<u64>,

    // === Execution Version ===
    execution_version: Option<u64>,

    // Dictates the threshold (percentage of stake) that is used to calculate the "bad" nodes to be
    // swapped when creating the consensus schedule. The values should be of the range [0 - 33].
    // Anything above 33 (f) will not be allowed.
    consensus_bad_nodes_stake_threshold: Option<u64>,

    max_jwk_votes_per_validator_per_epoch: Option<u64>,
    // The maximum age of a JWK in epochs before it is removed from the AuthenticatorState object.
    // Applied at the end of an epoch as a delta from the new epoch value, so setting this to 1
    // will cause the new epoch to start with JWKs from the previous epoch still valid.
    max_age_of_jwk_in_epochs: Option<u64>,

    // === random beacon ===
    /// Maximum allowed precision loss when reducing voting weights for the
    /// random beacon protocol.
    random_beacon_reduction_allowed_delta: Option<u16>,

    /// Minimum number of shares below which voting weights will not be reduced
    /// for the random beacon protocol.
    random_beacon_reduction_lower_bound: Option<u32>,

    /// Consensus Round after which DKG should be aborted and randomness
    /// disabled for the epoch, if it hasn't already completed.
    random_beacon_dkg_timeout_round: Option<u32>,

    /// Minimum interval between consecutive rounds of generated randomness.
    random_beacon_min_round_interval_ms: Option<u64>,

    /// Version of the random beacon DKG protocol.
    /// 0 was deprecated (and currently not supported), 1 is the default
    /// version.
    random_beacon_dkg_version: Option<u64>,

    /// The maximum serialized transaction size (in bytes) accepted by
    /// consensus. `consensus_max_transaction_size_bytes` should include
    /// space for additional metadata, on top of the `max_tx_size_bytes`
    /// value.
    consensus_max_transaction_size_bytes: Option<u64>,
    /// The maximum size of transactions included in a consensus block.
    consensus_max_transactions_in_block_bytes: Option<u64>,
    /// The maximum number of transactions included in a consensus block.
    consensus_max_num_transactions_in_block: Option<u64>,

    /// The max number of consensus rounds a transaction can be deferred due to
    /// shared object congestion. Transactions will be cancelled after this
    /// many rounds.
    max_deferral_rounds_for_congestion_control: Option<u64>,

    /// Minimum interval of commit timestamps between consecutive checkpoints.
    min_checkpoint_interval_ms: Option<u64>,

    /// Version number to use for version_specific_data in `CheckpointSummary`.
    checkpoint_summary_version_specific_data: Option<u64>,

    /// The max number of transactions that can be included in a single Soft
    /// Bundle.
    max_soft_bundle_size: Option<u64>,

    /// Whether to try to form bridge committee
    // Note: this is not a feature flag because we want to distinguish between
    // `None` and `Some(false)`, as committee was already finalized on Testnet.
    bridge_should_try_to_finalize_committee: Option<bool>,

    /// The max accumulated txn execution cost per object in a mysticeti commit.
    /// Transactions in a commit will be deferred once their touch shared
    /// objects hit this limit.    
    max_accumulated_txn_cost_per_object_in_mysticeti_commit: Option<u64>,

    /// Maximum number of committee (validators taking part in consensus)
    /// validators at any moment. We do not allow the number of committee
    /// validators in any epoch to go above this.
    max_committee_members_count: Option<u64>,
}

// feature flags
impl ProtocolConfig {
    // Add checks for feature flag support here, e.g.:
    // pub fn check_new_protocol_feature_supported(&self) -> Result<(), Error> {
    //     if self.feature_flags.new_protocol_feature_supported {
    //         Ok(())
    //     } else {
    //         Err(Error(format!(
    //             "new_protocol_feature is not supported at {:?}",
    //             self.version
    //         )))
    //     }
    // }

    pub fn disable_invariant_violation_check_in_swap_loc(&self) -> bool {
        self.feature_flags
            .disable_invariant_violation_check_in_swap_loc
    }

    pub fn no_extraneous_module_bytes(&self) -> bool {
        self.feature_flags.no_extraneous_module_bytes
    }

    pub fn zklogin_auth(&self) -> bool {
        self.feature_flags.zklogin_auth
    }

    pub fn consensus_transaction_ordering(&self) -> ConsensusTransactionOrdering {
        self.feature_flags.consensus_transaction_ordering
    }

    pub fn enable_jwk_consensus_updates(&self) -> bool {
        self.feature_flags.enable_jwk_consensus_updates
    }

    // this function only exists for readability in the genesis code.
    pub fn create_authenticator_state_in_genesis(&self) -> bool {
        self.enable_jwk_consensus_updates()
    }

    pub fn dkg_version(&self) -> u64 {
        // Version 0 was deprecated and removed, the default is 1 if not set.
        self.random_beacon_dkg_version.unwrap_or(1)
    }

    pub fn enable_bridge(&self) -> bool {
        self.feature_flags.bridge
    }

    pub fn should_try_to_finalize_bridge_committee(&self) -> bool {
        if !self.enable_bridge() {
            return false;
        }
        // In the older protocol version, always try to finalize the committee.
        self.bridge_should_try_to_finalize_committee.unwrap_or(true)
    }

    pub fn accept_zklogin_in_multisig(&self) -> bool {
        self.feature_flags.accept_zklogin_in_multisig
    }

    pub fn zklogin_max_epoch_upper_bound_delta(&self) -> Option<u64> {
        self.feature_flags.zklogin_max_epoch_upper_bound_delta
    }

    pub fn hardened_otw_check(&self) -> bool {
        self.feature_flags.hardened_otw_check
    }

    pub fn enable_poseidon(&self) -> bool {
        self.feature_flags.enable_poseidon
    }

    pub fn enable_group_ops_native_function_msm(&self) -> bool {
        self.feature_flags.enable_group_ops_native_function_msm
    }

    pub fn per_object_congestion_control_mode(&self) -> PerObjectCongestionControlMode {
        self.feature_flags.per_object_congestion_control_mode
    }

    pub fn consensus_choice(&self) -> ConsensusChoice {
        self.feature_flags.consensus_choice
    }

    pub fn consensus_network(&self) -> ConsensusNetwork {
        self.feature_flags.consensus_network
    }

    pub fn enable_vdf(&self) -> bool {
        self.feature_flags.enable_vdf
    }

    pub fn passkey_auth(&self) -> bool {
        self.feature_flags.passkey_auth
    }

    pub fn max_transaction_size_bytes(&self) -> u64 {
        // Provide a default value if protocol config version is too low.
        self.consensus_max_transaction_size_bytes
            .unwrap_or(256 * 1024)
    }

    pub fn max_transactions_in_block_bytes(&self) -> u64 {
        // Provide a default value if protocol config version is too low.
        self.consensus_max_transactions_in_block_bytes
            .unwrap_or(512 * 1024)
    }

    pub fn max_num_transactions_in_block(&self) -> u64 {
        // 500 is the value used before this field is introduced.
        self.consensus_max_num_transactions_in_block.unwrap_or(500)
    }

    pub fn rethrow_serialization_type_layout_errors(&self) -> bool {
        self.feature_flags.rethrow_serialization_type_layout_errors
    }

    pub fn relocate_event_module(&self) -> bool {
        self.feature_flags.relocate_event_module
    }

    pub fn protocol_defined_base_fee(&self) -> bool {
        self.feature_flags.protocol_defined_base_fee
    }

    pub fn uncompressed_g1_group_elements(&self) -> bool {
        self.feature_flags.uncompressed_g1_group_elements
    }

    pub fn disallow_new_modules_in_deps_only_packages(&self) -> bool {
        self.feature_flags
            .disallow_new_modules_in_deps_only_packages
    }

    pub fn native_charging_v2(&self) -> bool {
        self.feature_flags.native_charging_v2
    }
}

#[cfg(not(msim))]
static POISON_VERSION_METHODS: AtomicBool = const { AtomicBool::new(false) };

// Use a thread local in sim tests for test isolation.
#[cfg(msim)]
thread_local! {
    static POISON_VERSION_METHODS: AtomicBool = const { AtomicBool::new(false) };
}

// Instantiations for each protocol version.
impl ProtocolConfig {
    /// Get the value ProtocolConfig that are in effect during the given
    /// protocol version.
    pub fn get_for_version(version: ProtocolVersion, chain: Chain) -> Self {
        // ProtocolVersion can be deserialized so we need to check it here as well.
        assert!(
            version >= ProtocolVersion::MIN,
            "Network protocol version is {:?}, but the minimum supported version by the binary is {:?}. Please upgrade the binary.",
            version,
            ProtocolVersion::MIN.0,
        );
        assert!(
            version <= ProtocolVersion::MAX_ALLOWED,
            "Network protocol version is {:?}, but the maximum supported version by the binary is {:?}. Please upgrade the binary.",
            version,
            ProtocolVersion::MAX_ALLOWED.0,
        );

        let mut ret = Self::get_for_version_impl(version, chain);
        ret.version = version;

        CONFIG_OVERRIDE.with(|ovr| {
            if let Some(override_fn) = &*ovr.borrow() {
                warn!(
                    "overriding ProtocolConfig settings with custom settings (you should not see this log outside of tests)"
                );
                override_fn(version, ret)
            } else {
                ret
            }
        })
    }

    /// Get the value ProtocolConfig that are in effect during the given
    /// protocol version. Or none if the version is not supported.
    pub fn get_for_version_if_supported(version: ProtocolVersion, chain: Chain) -> Option<Self> {
        if version.0 >= ProtocolVersion::MIN.0 && version.0 <= ProtocolVersion::MAX_ALLOWED.0 {
            let mut ret = Self::get_for_version_impl(version, chain);
            ret.version = version;
            Some(ret)
        } else {
            None
        }
    }

    #[cfg(not(msim))]
    pub fn poison_get_for_min_version() {
        POISON_VERSION_METHODS.store(true, Ordering::Relaxed);
    }

    #[cfg(not(msim))]
    fn load_poison_get_for_min_version() -> bool {
        POISON_VERSION_METHODS.load(Ordering::Relaxed)
    }

    #[cfg(msim)]
    pub fn poison_get_for_min_version() {
        POISON_VERSION_METHODS.with(|p| p.store(true, Ordering::Relaxed));
    }

    #[cfg(msim)]
    fn load_poison_get_for_min_version() -> bool {
        POISON_VERSION_METHODS.with(|p| p.load(Ordering::Relaxed))
    }

    pub fn convert_type_argument_error(&self) -> bool {
        self.feature_flags.convert_type_argument_error
    }

    /// Convenience to get the constants at the current minimum supported
    /// version. Mainly used by client code that may not yet be
    /// protocol-version aware.
    pub fn get_for_min_version() -> Self {
        if Self::load_poison_get_for_min_version() {
            panic!("get_for_min_version called on validator");
        }
        ProtocolConfig::get_for_version(ProtocolVersion::MIN, Chain::Unknown)
    }

    /// CAREFUL! - You probably want to use `get_for_version` instead.
    ///
    /// Convenience to get the constants at the current maximum supported
    /// version. Mainly used by genesis. Note well that this function uses
    /// the max version supported locally by the node, which is not
    /// necessarily the current version of the network. ALSO, this function
    /// disregards chain specific config (by using Chain::Unknown), thereby
    /// potentially returning a protocol config that is incorrect for some
    /// feature flags. Definitely safe for testing and for protocol version
    /// 11 and prior.
    #[expect(non_snake_case)]
    pub fn get_for_max_version_UNSAFE() -> Self {
        if Self::load_poison_get_for_min_version() {
            panic!("get_for_max_version_UNSAFE called on validator");
        }
        ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Unknown)
    }

    fn get_for_version_impl(version: ProtocolVersion, chain: Chain) -> Self {
        #[cfg(msim)]
        {
            // populate the fake simulator version # with a different base tx cost.
            if version > ProtocolVersion::MAX {
                let mut config = Self::get_for_version_impl(ProtocolVersion::MAX, Chain::Unknown);
                config.base_tx_cost_fixed = Some(config.base_tx_cost_fixed() + 1000);
                return config;
            }
        }

        // IMPORTANT: Never modify the value of any constant for a pre-existing protocol
        // version. To change the values here you must create a new protocol
        // version with the new values!
        let mut cfg = Self {
            version,

            feature_flags: Default::default(),

            max_tx_size_bytes: Some(128 * 1024),
            // We need this number to be at least 100x less than
            // `max_serialized_tx_effects_size_bytes`otherwise effects can be huge
            max_input_objects: Some(2048),
            max_serialized_tx_effects_size_bytes: Some(512 * 1024),
            max_serialized_tx_effects_size_bytes_system_tx: Some(512 * 1024 * 16),
            max_gas_payment_objects: Some(256),
            max_modules_in_publish: Some(64),
            max_package_dependencies: Some(32),
            max_arguments: Some(512),
            max_type_arguments: Some(16),
            max_type_argument_depth: Some(16),
            max_pure_argument_size: Some(16 * 1024),
            max_programmable_tx_commands: Some(1024),
            move_binary_format_version: Some(7),
            min_move_binary_format_version: Some(6),
            binary_module_handles: Some(100),
            binary_struct_handles: Some(300),
            binary_function_handles: Some(1500),
            binary_function_instantiations: Some(750),
            binary_signatures: Some(1000),
            binary_constant_pool: Some(4000),
            binary_identifiers: Some(10000),
            binary_address_identifiers: Some(100),
            binary_struct_defs: Some(200),
            binary_struct_def_instantiations: Some(100),
            binary_function_defs: Some(1000),
            binary_field_handles: Some(500),
            binary_field_instantiations: Some(250),
            binary_friend_decls: Some(100),
            binary_enum_defs: None,
            binary_enum_def_instantiations: None,
            binary_variant_handles: None,
            binary_variant_instantiation_handles: None,
            max_move_object_size: Some(250 * 1024),
            max_move_package_size: Some(100 * 1024),
            max_publish_or_upgrade_per_ptb: Some(5),
            // max gas budget is in NANOS and an absolute value 50IOTA
            max_tx_gas: Some(50_000_000_000),
            max_gas_price: Some(100_000),
            max_gas_computation_bucket: Some(5_000_000),
            max_loop_depth: Some(5),
            max_generic_instantiation_length: Some(32),
            max_function_parameters: Some(128),
            max_basic_blocks: Some(1024),
            max_value_stack_size: Some(1024),
            max_type_nodes: Some(256),
            max_push_size: Some(10000),
            max_struct_definitions: Some(200),
            max_function_definitions: Some(1000),
            max_fields_in_struct: Some(32),
            max_dependency_depth: Some(100),
            max_num_event_emit: Some(1024),
            max_num_new_move_object_ids: Some(2048),
            max_num_new_move_object_ids_system_tx: Some(2048 * 16),
            max_num_deleted_move_object_ids: Some(2048),
            max_num_deleted_move_object_ids_system_tx: Some(2048 * 16),
            max_num_transferred_move_object_ids: Some(2048),
            max_num_transferred_move_object_ids_system_tx: Some(2048 * 16),
            max_event_emit_size: Some(250 * 1024),
            max_move_vector_len: Some(256 * 1024),
            max_type_to_layout_nodes: None,
            max_ptb_value_size: None,

            max_back_edges_per_function: Some(10_000),
            max_back_edges_per_module: Some(10_000),

            max_verifier_meter_ticks_per_function: Some(16_000_000),

            max_meter_ticks_per_module: Some(16_000_000),
            max_meter_ticks_per_package: Some(16_000_000),

            object_runtime_max_num_cached_objects: Some(1000),
            object_runtime_max_num_cached_objects_system_tx: Some(1000 * 16),
            object_runtime_max_num_store_entries: Some(1000),
            object_runtime_max_num_store_entries_system_tx: Some(1000 * 16),
            // min gas budget is in NANOS and an absolute value 1000 NANOS or 0.000001IOTA
            base_tx_cost_fixed: Some(1_000),
            package_publish_cost_fixed: Some(1_000),
            base_tx_cost_per_byte: Some(0),
            package_publish_cost_per_byte: Some(80),
            obj_access_cost_read_per_byte: Some(15),
            obj_access_cost_mutate_per_byte: Some(40),
            obj_access_cost_delete_per_byte: Some(40),
            obj_access_cost_verify_per_byte: Some(200),
            obj_data_cost_refundable: Some(100),
            obj_metadata_cost_non_refundable: Some(50),
            gas_model_version: Some(1),
            storage_rebate_rate: Some(10000),
            // Change reward slashing rate to 100%.
            reward_slashing_rate: Some(10000),
            storage_gas_price: Some(76),
            base_gas_price: None,
            // The initial subsidy (target reward) for validators per epoch.
            // Refer to the IOTA tokenomics for the origin of this value.
            validator_target_reward: Some(767_000 * 1_000_000_000),
            max_transactions_per_checkpoint: Some(10_000),
            max_checkpoint_size_bytes: Some(30 * 1024 * 1024),

            // For now, perform upgrades with a bare quorum of validators.
            buffer_stake_for_protocol_upgrade_bps: Some(5000),

            // === Native Function Costs ===
            // `address` module
            // Cost params for the Move native function `address::from_bytes(bytes: vector<u8>)`
            address_from_bytes_cost_base: Some(52),
            // Cost params for the Move native function `address::to_u256(address): u256`
            address_to_u256_cost_base: Some(52),
            // Cost params for the Move native function `address::from_u256(u256): address`
            address_from_u256_cost_base: Some(52),

            // `config` module
            // Cost params for the Move native function `read_setting_impl``
            config_read_setting_impl_cost_base: Some(100),
            config_read_setting_impl_cost_per_byte: Some(40),

            // `dynamic_field` module
            // Cost params for the Move native function `hash_type_and_key<K: copy + drop +
            // store>(parent: address, k: K): address`
            dynamic_field_hash_type_and_key_cost_base: Some(100),
            dynamic_field_hash_type_and_key_type_cost_per_byte: Some(2),
            dynamic_field_hash_type_and_key_value_cost_per_byte: Some(2),
            dynamic_field_hash_type_and_key_type_tag_cost_per_byte: Some(2),
            // Cost params for the Move native function `add_child_object<Child: key>(parent:
            // address, child: Child)`
            dynamic_field_add_child_object_cost_base: Some(100),
            dynamic_field_add_child_object_type_cost_per_byte: Some(10),
            dynamic_field_add_child_object_value_cost_per_byte: Some(10),
            dynamic_field_add_child_object_struct_tag_cost_per_byte: Some(10),
            // Cost params for the Move native function `borrow_child_object_mut<Child: key>(parent:
            // &mut UID, id: address): &mut Child`
            dynamic_field_borrow_child_object_cost_base: Some(100),
            dynamic_field_borrow_child_object_child_ref_cost_per_byte: Some(10),
            dynamic_field_borrow_child_object_type_cost_per_byte: Some(10),
            // Cost params for the Move native function `remove_child_object<Child: key>(parent:
            // address, id: address): Child`
            dynamic_field_remove_child_object_cost_base: Some(100),
            dynamic_field_remove_child_object_child_cost_per_byte: Some(2),
            dynamic_field_remove_child_object_type_cost_per_byte: Some(2),
            // Cost params for the Move native function `has_child_object(parent: address, id:
            // address): bool`
            dynamic_field_has_child_object_cost_base: Some(100),
            // Cost params for the Move native function `has_child_object_with_ty<Child:
            // key>(parent: address, id: address): bool`
            dynamic_field_has_child_object_with_ty_cost_base: Some(100),
            dynamic_field_has_child_object_with_ty_type_cost_per_byte: Some(2),
            dynamic_field_has_child_object_with_ty_type_tag_cost_per_byte: Some(2),

            // `event` module
            // Cost params for the Move native function `event::emit<T: copy + drop>(event: T)`
            event_emit_cost_base: Some(52),
            event_emit_value_size_derivation_cost_per_byte: Some(2),
            event_emit_tag_size_derivation_cost_per_byte: Some(5),
            event_emit_output_cost_per_byte: Some(10),

            //  `object` module
            // Cost params for the Move native function `borrow_uid<T: key>(obj: &T): &UID`
            object_borrow_uid_cost_base: Some(52),
            // Cost params for the Move native function `delete_impl(id: address)`
            object_delete_impl_cost_base: Some(52),
            // Cost params for the Move native function `record_new_uid(id: address)`
            object_record_new_uid_cost_base: Some(52),

            // `transfer` module
            // Cost params for the Move native function `transfer_impl<T: key>(obj: T, recipient:
            // address)`
            transfer_transfer_internal_cost_base: Some(52),
            // Cost params for the Move native function `freeze_object<T: key>(obj: T)`
            transfer_freeze_object_cost_base: Some(52),
            // Cost params for the Move native function `share_object<T: key>(obj: T)`
            transfer_share_object_cost_base: Some(52),
            transfer_receive_object_cost_base: Some(52),

            // `tx_context` module
            // Cost params for the Move native function `transfer_impl<T: key>(obj: T, recipient:
            // address)`
            tx_context_derive_id_cost_base: Some(52),

            // `types` module
            // Cost params for the Move native function `is_one_time_witness<T: drop>(_: &T): bool`
            types_is_one_time_witness_cost_base: Some(52),
            types_is_one_time_witness_type_tag_cost_per_byte: Some(2),
            types_is_one_time_witness_type_cost_per_byte: Some(2),

            // `validator` module
            // Cost params for the Move native function `validate_metadata_bcs(metadata:
            // vector<u8>)`
            validator_validate_metadata_cost_base: Some(52),
            validator_validate_metadata_data_cost_per_byte: Some(2),

            // Crypto
            crypto_invalid_arguments_cost: Some(100),
            // bls12381::bls12381_min_pk_verify
            bls12381_bls12381_min_sig_verify_cost_base: Some(52),
            bls12381_bls12381_min_sig_verify_msg_cost_per_byte: Some(2),
            bls12381_bls12381_min_sig_verify_msg_cost_per_block: Some(2),

            // bls12381::bls12381_min_pk_verify
            bls12381_bls12381_min_pk_verify_cost_base: Some(52),
            bls12381_bls12381_min_pk_verify_msg_cost_per_byte: Some(2),
            bls12381_bls12381_min_pk_verify_msg_cost_per_block: Some(2),

            // ecdsa_k1::ecrecover
            ecdsa_k1_ecrecover_keccak256_cost_base: Some(52),
            ecdsa_k1_ecrecover_keccak256_msg_cost_per_byte: Some(2),
            ecdsa_k1_ecrecover_keccak256_msg_cost_per_block: Some(2),
            ecdsa_k1_ecrecover_sha256_cost_base: Some(52),
            ecdsa_k1_ecrecover_sha256_msg_cost_per_byte: Some(2),
            ecdsa_k1_ecrecover_sha256_msg_cost_per_block: Some(2),

            // ecdsa_k1::decompress_pubkey
            ecdsa_k1_decompress_pubkey_cost_base: Some(52),

            // ecdsa_k1::secp256k1_verify
            ecdsa_k1_secp256k1_verify_keccak256_cost_base: Some(52),
            ecdsa_k1_secp256k1_verify_keccak256_msg_cost_per_byte: Some(2),
            ecdsa_k1_secp256k1_verify_keccak256_msg_cost_per_block: Some(2),
            ecdsa_k1_secp256k1_verify_sha256_cost_base: Some(52),
            ecdsa_k1_secp256k1_verify_sha256_msg_cost_per_byte: Some(2),
            ecdsa_k1_secp256k1_verify_sha256_msg_cost_per_block: Some(2),

            // ecdsa_r1::ecrecover
            ecdsa_r1_ecrecover_keccak256_cost_base: Some(52),
            ecdsa_r1_ecrecover_keccak256_msg_cost_per_byte: Some(2),
            ecdsa_r1_ecrecover_keccak256_msg_cost_per_block: Some(2),
            ecdsa_r1_ecrecover_sha256_cost_base: Some(52),
            ecdsa_r1_ecrecover_sha256_msg_cost_per_byte: Some(2),
            ecdsa_r1_ecrecover_sha256_msg_cost_per_block: Some(2),

            // ecdsa_r1::secp256k1_verify
            ecdsa_r1_secp256r1_verify_keccak256_cost_base: Some(52),
            ecdsa_r1_secp256r1_verify_keccak256_msg_cost_per_byte: Some(2),
            ecdsa_r1_secp256r1_verify_keccak256_msg_cost_per_block: Some(2),
            ecdsa_r1_secp256r1_verify_sha256_cost_base: Some(52),
            ecdsa_r1_secp256r1_verify_sha256_msg_cost_per_byte: Some(2),
            ecdsa_r1_secp256r1_verify_sha256_msg_cost_per_block: Some(2),

            // ecvrf::verify
            ecvrf_ecvrf_verify_cost_base: Some(52),
            ecvrf_ecvrf_verify_alpha_string_cost_per_byte: Some(2),
            ecvrf_ecvrf_verify_alpha_string_cost_per_block: Some(2),

            // ed25519
            ed25519_ed25519_verify_cost_base: Some(52),
            ed25519_ed25519_verify_msg_cost_per_byte: Some(2),
            ed25519_ed25519_verify_msg_cost_per_block: Some(2),

            // groth16::prepare_verifying_key
            groth16_prepare_verifying_key_bls12381_cost_base: Some(52),
            groth16_prepare_verifying_key_bn254_cost_base: Some(52),

            // groth16::verify_groth16_proof_internal
            groth16_verify_groth16_proof_internal_bls12381_cost_base: Some(52),
            groth16_verify_groth16_proof_internal_bls12381_cost_per_public_input: Some(2),
            groth16_verify_groth16_proof_internal_bn254_cost_base: Some(52),
            groth16_verify_groth16_proof_internal_bn254_cost_per_public_input: Some(2),
            groth16_verify_groth16_proof_internal_public_input_cost_per_byte: Some(2),

            // hash::blake2b256
            hash_blake2b256_cost_base: Some(52),
            hash_blake2b256_data_cost_per_byte: Some(2),
            hash_blake2b256_data_cost_per_block: Some(2),
            // hash::keccak256
            hash_keccak256_cost_base: Some(52),
            hash_keccak256_data_cost_per_byte: Some(2),
            hash_keccak256_data_cost_per_block: Some(2),

            poseidon_bn254_cost_base: None,
            poseidon_bn254_cost_per_block: None,

            // hmac::hmac_sha3_256
            hmac_hmac_sha3_256_cost_base: Some(52),
            hmac_hmac_sha3_256_input_cost_per_byte: Some(2),
            hmac_hmac_sha3_256_input_cost_per_block: Some(2),

            // group ops
            group_ops_bls12381_decode_scalar_cost: Some(52),
            group_ops_bls12381_decode_g1_cost: Some(52),
            group_ops_bls12381_decode_g2_cost: Some(52),
            group_ops_bls12381_decode_gt_cost: Some(52),
            group_ops_bls12381_scalar_add_cost: Some(52),
            group_ops_bls12381_g1_add_cost: Some(52),
            group_ops_bls12381_g2_add_cost: Some(52),
            group_ops_bls12381_gt_add_cost: Some(52),
            group_ops_bls12381_scalar_sub_cost: Some(52),
            group_ops_bls12381_g1_sub_cost: Some(52),
            group_ops_bls12381_g2_sub_cost: Some(52),
            group_ops_bls12381_gt_sub_cost: Some(52),
            group_ops_bls12381_scalar_mul_cost: Some(52),
            group_ops_bls12381_g1_mul_cost: Some(52),
            group_ops_bls12381_g2_mul_cost: Some(52),
            group_ops_bls12381_gt_mul_cost: Some(52),
            group_ops_bls12381_scalar_div_cost: Some(52),
            group_ops_bls12381_g1_div_cost: Some(52),
            group_ops_bls12381_g2_div_cost: Some(52),
            group_ops_bls12381_gt_div_cost: Some(52),
            group_ops_bls12381_g1_hash_to_base_cost: Some(52),
            group_ops_bls12381_g2_hash_to_base_cost: Some(52),
            group_ops_bls12381_g1_hash_to_cost_per_byte: Some(2),
            group_ops_bls12381_g2_hash_to_cost_per_byte: Some(2),
            group_ops_bls12381_g1_msm_base_cost: Some(52),
            group_ops_bls12381_g2_msm_base_cost: Some(52),
            group_ops_bls12381_g1_msm_base_cost_per_input: Some(52),
            group_ops_bls12381_g2_msm_base_cost_per_input: Some(52),
            group_ops_bls12381_msm_max_len: Some(32),
            group_ops_bls12381_pairing_cost: Some(52),
            group_ops_bls12381_g1_to_uncompressed_g1_cost: None,
            group_ops_bls12381_uncompressed_g1_to_g1_cost: None,
            group_ops_bls12381_uncompressed_g1_sum_base_cost: None,
            group_ops_bls12381_uncompressed_g1_sum_cost_per_term: None,
            group_ops_bls12381_uncompressed_g1_sum_max_terms: None,

            // zklogin::check_zklogin_id
            check_zklogin_id_cost_base: Some(200),
            // zklogin::check_zklogin_issuer
            check_zklogin_issuer_cost_base: Some(200),

            vdf_verify_vdf_cost: None,
            vdf_hash_to_input_cost: None,

            bcs_per_byte_serialized_cost: Some(2),
            bcs_legacy_min_output_size_cost: Some(1),
            bcs_failure_cost: Some(52),
            hash_sha2_256_base_cost: Some(52),
            hash_sha2_256_per_byte_cost: Some(2),
            hash_sha2_256_legacy_min_input_len_cost: Some(1),
            hash_sha3_256_base_cost: Some(52),
            hash_sha3_256_per_byte_cost: Some(2),
            hash_sha3_256_legacy_min_input_len_cost: Some(1),
            type_name_get_base_cost: Some(52),
            type_name_get_per_byte_cost: Some(2),
            string_check_utf8_base_cost: Some(52),
            string_check_utf8_per_byte_cost: Some(2),
            string_is_char_boundary_base_cost: Some(52),
            string_sub_string_base_cost: Some(52),
            string_sub_string_per_byte_cost: Some(2),
            string_index_of_base_cost: Some(52),
            string_index_of_per_byte_pattern_cost: Some(2),
            string_index_of_per_byte_searched_cost: Some(2),
            vector_empty_base_cost: Some(52),
            vector_length_base_cost: Some(52),
            vector_push_back_base_cost: Some(52),
            vector_push_back_legacy_per_abstract_memory_unit_cost: Some(2),
            vector_borrow_base_cost: Some(52),
            vector_pop_back_base_cost: Some(52),
            vector_destroy_empty_base_cost: Some(52),
            vector_swap_base_cost: Some(52),
            debug_print_base_cost: Some(52),
            debug_print_stack_trace_base_cost: Some(52),

            max_size_written_objects: Some(5 * 1000 * 1000),
            // max size of written objects during a system TXn to allow for larger writes
            // akin to `max_size_written_objects` but for system TXns
            max_size_written_objects_system_tx: Some(50 * 1000 * 1000),

            // Limits the length of a Move identifier
            max_move_identifier_len: Some(128),
            max_move_value_depth: Some(128),
            max_move_enum_variants: None,

            gas_rounding_step: Some(1_000),

            execution_version: Some(1),

            // We maintain the same total size limit for events, but increase the number of
            // events that can be emitted.
            max_event_emit_size_total: Some(
                256 /* former event count limit */ * 250 * 1024, // size limit per event
            ),

            // Taking a baby step approach, we consider only 20% by stake as bad nodes so we
            // have a 80% by stake of nodes participating in the leader committee. That
            // allow us for more redundancy in case we have validators
            // under performing - since the responsibility is shared
            // amongst more nodes. We can increase that once we do have
            // higher confidence.
            consensus_bad_nodes_stake_threshold: Some(20),

            // Max of 10 votes per hour.
            max_jwk_votes_per_validator_per_epoch: Some(240),

            max_age_of_jwk_in_epochs: Some(1),

            consensus_max_transaction_size_bytes: Some(256 * 1024), // 256KB

            // Assume 1KB per transaction and 500 transactions per block.
            consensus_max_transactions_in_block_bytes: Some(512 * 1024),

            random_beacon_reduction_allowed_delta: Some(800),

            random_beacon_reduction_lower_bound: Some(1000),
            random_beacon_dkg_timeout_round: Some(3000),
            random_beacon_min_round_interval_ms: Some(500),

            random_beacon_dkg_version: Some(1),

            // Assume 20_000 TPS * 5% max stake per validator / (minimum) 4 blocks per round
            // = 250 transactions per block maximum Using a higher limit
            // that is 512, to account for bursty traffic and system transactions.
            consensus_max_num_transactions_in_block: Some(512),

            max_deferral_rounds_for_congestion_control: Some(10),

            min_checkpoint_interval_ms: Some(200),

            checkpoint_summary_version_specific_data: Some(1),

            max_soft_bundle_size: Some(5),

            bridge_should_try_to_finalize_committee: None,

            max_accumulated_txn_cost_per_object_in_mysticeti_commit: Some(10),

            max_committee_members_count: None,
            // When adding a new constant, set it to None in the earliest version, like this:
            // new_constant: None,
        };

        cfg.feature_flags.consensus_transaction_ordering = ConsensusTransactionOrdering::ByGasPrice;

        // MoveVM related flags
        {
            cfg.feature_flags
                .disable_invariant_violation_check_in_swap_loc = true;
            cfg.feature_flags.no_extraneous_module_bytes = true;
            cfg.feature_flags.hardened_otw_check = true;
            cfg.feature_flags.rethrow_serialization_type_layout_errors = true;
        }

        // zkLogin related flags
        {
            cfg.feature_flags.zklogin_auth = false;
            cfg.feature_flags.enable_jwk_consensus_updates = false;
            cfg.feature_flags.zklogin_max_epoch_upper_bound_delta = Some(30);
            cfg.feature_flags.accept_zklogin_in_multisig = false;
        }

        // Enable Mysticeti on mainnet.
        cfg.feature_flags.consensus_choice = ConsensusChoice::Mysticeti;
        // Use tonic networking for Mysticeti.
        cfg.feature_flags.consensus_network = ConsensusNetwork::Tonic;

        cfg.feature_flags.per_object_congestion_control_mode =
            PerObjectCongestionControlMode::TotalTxCount;

        // Do not allow bridge committee to finalize on mainnet.
        cfg.bridge_should_try_to_finalize_committee = Some(chain != Chain::Mainnet);

        cfg.feature_flags.bridge = false;

        // Devnet
        if chain != Chain::Mainnet && chain != Chain::Testnet {
            cfg.feature_flags.enable_poseidon = true;
            cfg.poseidon_bn254_cost_base = Some(260);
            cfg.poseidon_bn254_cost_per_block = Some(10);

            cfg.feature_flags.enable_group_ops_native_function_msm = true;

            cfg.feature_flags.enable_vdf = true;
            // Set to 30x and 2x the cost of a signature verification for now. This
            // should be updated along with other native crypto functions.
            cfg.vdf_verify_vdf_cost = Some(1500);
            cfg.vdf_hash_to_input_cost = Some(100);

            cfg.feature_flags.passkey_auth = true;
        }

        for cur in 2..=version.0 {
            match cur {
                1 => unreachable!(),
                // version 2 is a new framework version but with no config changes
                2 => {}
                3 => {
                    cfg.feature_flags.relocate_event_module = true;
                }
                4 => {
                    cfg.max_type_to_layout_nodes = Some(512);
                }
                5 => {
                    cfg.feature_flags.protocol_defined_base_fee = true;
                    cfg.base_gas_price = Some(1000);

                    cfg.feature_flags.disallow_new_modules_in_deps_only_packages = true;
                    cfg.feature_flags.convert_type_argument_error = true;
                    cfg.feature_flags.native_charging_v2 = true;

                    if chain != Chain::Mainnet && chain != Chain::Testnet {
                        cfg.feature_flags.uncompressed_g1_group_elements = true;
                    }

                    cfg.gas_model_version = Some(2);

                    cfg.poseidon_bn254_cost_per_block = Some(388);

                    cfg.bls12381_bls12381_min_sig_verify_cost_base = Some(44064);
                    cfg.bls12381_bls12381_min_pk_verify_cost_base = Some(49282);
                    cfg.ecdsa_k1_secp256k1_verify_keccak256_cost_base = Some(1470);
                    cfg.ecdsa_k1_secp256k1_verify_sha256_cost_base = Some(1470);
                    cfg.ecdsa_r1_secp256r1_verify_sha256_cost_base = Some(4225);
                    cfg.ecdsa_r1_secp256r1_verify_keccak256_cost_base = Some(4225);
                    cfg.ecvrf_ecvrf_verify_cost_base = Some(4848);
                    cfg.ed25519_ed25519_verify_cost_base = Some(1802);

                    // Manually changed to be "under cost"
                    cfg.ecdsa_r1_ecrecover_keccak256_cost_base = Some(1173);
                    cfg.ecdsa_r1_ecrecover_sha256_cost_base = Some(1173);
                    cfg.ecdsa_k1_ecrecover_keccak256_cost_base = Some(500);
                    cfg.ecdsa_k1_ecrecover_sha256_cost_base = Some(500);

                    cfg.groth16_prepare_verifying_key_bls12381_cost_base = Some(53838);
                    cfg.groth16_prepare_verifying_key_bn254_cost_base = Some(82010);
                    cfg.groth16_verify_groth16_proof_internal_bls12381_cost_base = Some(72090);
                    cfg.groth16_verify_groth16_proof_internal_bls12381_cost_per_public_input =
                        Some(8213);
                    cfg.groth16_verify_groth16_proof_internal_bn254_cost_base = Some(115502);
                    cfg.groth16_verify_groth16_proof_internal_bn254_cost_per_public_input =
                        Some(9484);

                    cfg.hash_keccak256_cost_base = Some(10);
                    cfg.hash_blake2b256_cost_base = Some(10);

                    // group ops
                    cfg.group_ops_bls12381_decode_scalar_cost = Some(7);
                    cfg.group_ops_bls12381_decode_g1_cost = Some(2848);
                    cfg.group_ops_bls12381_decode_g2_cost = Some(3770);
                    cfg.group_ops_bls12381_decode_gt_cost = Some(3068);

                    cfg.group_ops_bls12381_scalar_add_cost = Some(10);
                    cfg.group_ops_bls12381_g1_add_cost = Some(1556);
                    cfg.group_ops_bls12381_g2_add_cost = Some(3048);
                    cfg.group_ops_bls12381_gt_add_cost = Some(188);

                    cfg.group_ops_bls12381_scalar_sub_cost = Some(10);
                    cfg.group_ops_bls12381_g1_sub_cost = Some(1550);
                    cfg.group_ops_bls12381_g2_sub_cost = Some(3019);
                    cfg.group_ops_bls12381_gt_sub_cost = Some(497);

                    cfg.group_ops_bls12381_scalar_mul_cost = Some(11);
                    cfg.group_ops_bls12381_g1_mul_cost = Some(4842);
                    cfg.group_ops_bls12381_g2_mul_cost = Some(9108);
                    cfg.group_ops_bls12381_gt_mul_cost = Some(27490);

                    cfg.group_ops_bls12381_scalar_div_cost = Some(91);
                    cfg.group_ops_bls12381_g1_div_cost = Some(5091);
                    cfg.group_ops_bls12381_g2_div_cost = Some(9206);
                    cfg.group_ops_bls12381_gt_div_cost = Some(27804);

                    cfg.group_ops_bls12381_g1_hash_to_base_cost = Some(2962);
                    cfg.group_ops_bls12381_g2_hash_to_base_cost = Some(8688);

                    cfg.group_ops_bls12381_g1_msm_base_cost = Some(62648);
                    cfg.group_ops_bls12381_g2_msm_base_cost = Some(131192);
                    cfg.group_ops_bls12381_g1_msm_base_cost_per_input = Some(1333);
                    cfg.group_ops_bls12381_g2_msm_base_cost_per_input = Some(3216);

                    cfg.group_ops_bls12381_uncompressed_g1_to_g1_cost = Some(677);
                    cfg.group_ops_bls12381_g1_to_uncompressed_g1_cost = Some(2099);
                    cfg.group_ops_bls12381_uncompressed_g1_sum_base_cost = Some(77);
                    cfg.group_ops_bls12381_uncompressed_g1_sum_cost_per_term = Some(26);
                    cfg.group_ops_bls12381_uncompressed_g1_sum_max_terms = Some(1200);

                    cfg.group_ops_bls12381_pairing_cost = Some(26897);

                    cfg.validator_validate_metadata_cost_base = Some(20000);

                    cfg.max_committee_members_count = Some(50);
                }
                6 => {
                    cfg.max_ptb_value_size = Some(1024 * 1024);
                }
                // Use this template when making changes:
                //
                //     // modify an existing constant.
                //     move_binary_format_version: Some(7),
                //
                //     // Add a new constant (which is set to None in prior versions).
                //     new_constant: Some(new_value),
                //
                //     // Remove a constant (ensure that it is never accessed during this version).
                //     max_move_object_size: None,
                _ => panic!("unsupported version {:?}", version),
            }
        }
        cfg
    }

    // Extract the bytecode verifier config from this protocol config. `for_signing`
    // indicates whether this config is used for verification during signing or
    // execution.
    pub fn verifier_config(&self, signing_limits: Option<(usize, usize)>) -> VerifierConfig {
        let (max_back_edges_per_function, max_back_edges_per_module) = if let Some((
            max_back_edges_per_function,
            max_back_edges_per_module,
        )) = signing_limits
        {
            (
                Some(max_back_edges_per_function),
                Some(max_back_edges_per_module),
            )
        } else {
            (None, None)
        };

        VerifierConfig {
            max_loop_depth: Some(self.max_loop_depth() as usize),
            max_generic_instantiation_length: Some(self.max_generic_instantiation_length() as usize),
            max_function_parameters: Some(self.max_function_parameters() as usize),
            max_basic_blocks: Some(self.max_basic_blocks() as usize),
            max_value_stack_size: self.max_value_stack_size() as usize,
            max_type_nodes: Some(self.max_type_nodes() as usize),
            max_push_size: Some(self.max_push_size() as usize),
            max_dependency_depth: Some(self.max_dependency_depth() as usize),
            max_fields_in_struct: Some(self.max_fields_in_struct() as usize),
            max_function_definitions: Some(self.max_function_definitions() as usize),
            max_data_definitions: Some(self.max_struct_definitions() as usize),
            max_constant_vector_len: Some(self.max_move_vector_len()),
            max_back_edges_per_function,
            max_back_edges_per_module,
            max_basic_blocks_in_script: None,
            max_identifier_len: self.max_move_identifier_len_as_option(), /* Before protocol
                                                                           * version 9, there was
                                                                           * no limit */
            bytecode_version: self.move_binary_format_version(),
            max_variants_in_enum: self.max_move_enum_variants_as_option(),
        }
    }

    /// Override one or more settings in the config, for testing.
    /// This must be called at the beginning of the test, before
    /// get_for_(min|max)_version is called, since those functions cache
    /// their return value.
    pub fn apply_overrides_for_testing(
        override_fn: impl Fn(ProtocolVersion, Self) -> Self + Send + Sync + 'static,
    ) -> OverrideGuard {
        CONFIG_OVERRIDE.with(|ovr| {
            let mut cur = ovr.borrow_mut();
            assert!(cur.is_none(), "config override already present");
            *cur = Some(Box::new(override_fn));
            OverrideGuard
        })
    }
}

// Setters for tests.
// This is only needed for feature_flags. Please suffix each setter with
// `_for_testing`. Non-feature_flags should already have test setters defined
// through macros.
impl ProtocolConfig {
    pub fn set_zklogin_auth_for_testing(&mut self, val: bool) {
        self.feature_flags.zklogin_auth = val
    }
    pub fn set_enable_jwk_consensus_updates_for_testing(&mut self, val: bool) {
        self.feature_flags.enable_jwk_consensus_updates = val
    }

    pub fn set_accept_zklogin_in_multisig_for_testing(&mut self, val: bool) {
        self.feature_flags.accept_zklogin_in_multisig = val
    }

    pub fn set_per_object_congestion_control_mode_for_testing(
        &mut self,
        val: PerObjectCongestionControlMode,
    ) {
        self.feature_flags.per_object_congestion_control_mode = val;
    }

    pub fn set_consensus_choice_for_testing(&mut self, val: ConsensusChoice) {
        self.feature_flags.consensus_choice = val;
    }

    pub fn set_consensus_network_for_testing(&mut self, val: ConsensusNetwork) {
        self.feature_flags.consensus_network = val;
    }

    pub fn set_zklogin_max_epoch_upper_bound_delta_for_testing(&mut self, val: Option<u64>) {
        self.feature_flags.zklogin_max_epoch_upper_bound_delta = val
    }
    pub fn set_disable_bridge_for_testing(&mut self) {
        self.feature_flags.bridge = false
    }

    pub fn set_passkey_auth_for_testing(&mut self, val: bool) {
        self.feature_flags.passkey_auth = val
    }

    pub fn set_disallow_new_modules_in_deps_only_packages_for_testing(&mut self, val: bool) {
        self.feature_flags
            .disallow_new_modules_in_deps_only_packages = val;
    }
}

type OverrideFn = dyn Fn(ProtocolVersion, ProtocolConfig) -> ProtocolConfig + Send + Sync;

thread_local! {
    static CONFIG_OVERRIDE: RefCell<Option<Box<OverrideFn>>> = const { RefCell::new(None) };
}

#[must_use]
pub struct OverrideGuard;

impl Drop for OverrideGuard {
    fn drop(&mut self) {
        info!("restoring override fn");
        CONFIG_OVERRIDE.with(|ovr| {
            *ovr.borrow_mut() = None;
        });
    }
}

/// Defines which limit got crossed.
/// The value which crossed the limit and value of the limit crossed are
/// embedded
#[derive(PartialEq, Eq)]
pub enum LimitThresholdCrossed {
    None,
    Soft(u128, u128),
    Hard(u128, u128),
}

/// Convenience function for comparing limit ranges
/// V::MAX must be at >= U::MAX and T::MAX
pub fn check_limit_in_range<T: Into<V>, U: Into<V>, V: PartialOrd + Into<u128>>(
    x: T,
    soft_limit: U,
    hard_limit: V,
) -> LimitThresholdCrossed {
    let x: V = x.into();
    let soft_limit: V = soft_limit.into();

    debug_assert!(soft_limit <= hard_limit);

    // It is important to preserve this comparison order because if soft_limit ==
    // hard_limit we want LimitThresholdCrossed::Hard
    if x >= hard_limit {
        LimitThresholdCrossed::Hard(x.into(), hard_limit.into())
    } else if x < soft_limit {
        LimitThresholdCrossed::None
    } else {
        LimitThresholdCrossed::Soft(x.into(), soft_limit.into())
    }
}

#[macro_export]
macro_rules! check_limit {
    ($x:expr, $hard:expr) => {
        check_limit!($x, $hard, $hard)
    };
    ($x:expr, $soft:expr, $hard:expr) => {
        check_limit_in_range($x as u64, $soft, $hard)
    };
}

/// Used to check which limits were crossed if the TX is metered (not system tx)
/// Args are: is_metered, value_to_check, metered_limit, unmetered_limit
/// metered_limit is always less than or equal to unmetered_hard_limit
#[macro_export]
macro_rules! check_limit_by_meter {
    ($is_metered:expr, $x:expr, $metered_limit:expr, $unmetered_hard_limit:expr, $metric:expr) => {{
        // If this is metered, we use the metered_limit limit as the upper bound
        let (h, metered_str) = if $is_metered {
            ($metered_limit, "metered")
        } else {
            // Unmetered gets more headroom
            ($unmetered_hard_limit, "unmetered")
        };
        use iota_protocol_config::check_limit_in_range;
        let result = check_limit_in_range($x as u64, $metered_limit, h);
        match result {
            LimitThresholdCrossed::None => {}
            LimitThresholdCrossed::Soft(_, _) => {
                $metric.with_label_values(&[metered_str, "soft"]).inc();
            }
            LimitThresholdCrossed::Hard(_, _) => {
                $metric.with_label_values(&[metered_str, "hard"]).inc();
            }
        };
        result
    }};
}

#[cfg(all(test, not(msim)))]
mod test {
    use insta::assert_yaml_snapshot;

    use super::*;

    #[test]
    fn snapshot_tests() {
        println!("\n============================================================================");
        println!("!                                                                          !");
        println!("! IMPORTANT: never update snapshots from this test. only add new versions! !");
        println!("!                                                                          !");
        println!("============================================================================\n");
        for chain_id in &[Chain::Unknown, Chain::Mainnet, Chain::Testnet] {
            // make Chain::Unknown snapshots compatible with pre-chain-id snapshots so that
            // we don't break the release-time compatibility tests. Once Chain
            // Id configs have been released everywhere, we can remove this and
            // only test Mainnet and Testnet
            let chain_str = match chain_id {
                Chain::Unknown => "".to_string(),
                _ => format!("{:?}_", chain_id),
            };
            for i in MIN_PROTOCOL_VERSION..=MAX_PROTOCOL_VERSION {
                let cur = ProtocolVersion::new(i);
                assert_yaml_snapshot!(
                    format!("{}version_{}", chain_str, cur.as_u64()),
                    ProtocolConfig::get_for_version(cur, *chain_id)
                );
            }
        }
    }

    #[test]
    fn test_getters() {
        let prot: ProtocolConfig =
            ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Unknown);
        assert_eq!(
            prot.max_arguments(),
            prot.max_arguments_as_option().unwrap()
        );
    }

    #[test]
    fn test_setters() {
        let mut prot: ProtocolConfig =
            ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Unknown);
        prot.set_max_arguments_for_testing(123);
        assert_eq!(prot.max_arguments(), 123);

        prot.set_max_arguments_from_str_for_testing("321".to_string());
        assert_eq!(prot.max_arguments(), 321);

        prot.disable_max_arguments_for_testing();
        assert_eq!(prot.max_arguments_as_option(), None);

        prot.set_attr_for_testing("max_arguments".to_string(), "456".to_string());
        assert_eq!(prot.max_arguments(), 456);
    }

    #[test]
    fn lookup_by_string_test() {
        let prot: ProtocolConfig =
            ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Mainnet);
        // Does not exist
        assert!(prot.lookup_attr("some random string".to_string()).is_none());

        assert!(
            prot.lookup_attr("max_arguments".to_string())
                == Some(ProtocolConfigValue::u32(prot.max_arguments())),
        );

        // We didnt have this in version 1 on Mainnet
        assert!(
            prot.lookup_attr("poseidon_bn254_cost_base".to_string())
                .is_none()
        );
        assert!(
            prot.attr_map()
                .get("poseidon_bn254_cost_base")
                .unwrap()
                .is_none()
        );

        // But we did in version 1 on Devnet
        let prot: ProtocolConfig =
            ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Unknown);

        assert!(
            prot.lookup_attr("poseidon_bn254_cost_base".to_string())
                == Some(ProtocolConfigValue::u64(prot.poseidon_bn254_cost_base()))
        );
        assert!(
            prot.attr_map().get("poseidon_bn254_cost_base").unwrap()
                == &Some(ProtocolConfigValue::u64(prot.poseidon_bn254_cost_base()))
        );

        // Check feature flags
        let prot: ProtocolConfig =
            ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Mainnet);
        // Does not exist
        assert!(
            prot.feature_flags
                .lookup_attr("some random string".to_owned())
                .is_none()
        );
        assert!(
            !prot
                .feature_flags
                .attr_map()
                .contains_key("some random string")
        );

        // Was false in v1 on Mainnet
        assert!(prot.feature_flags.lookup_attr("enable_poseidon".to_owned()) == Some(false));
        assert!(
            prot.feature_flags
                .attr_map()
                .get("enable_poseidon")
                .unwrap()
                == &false
        );
        let prot: ProtocolConfig =
            ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Unknown);
        // Was true from v1 and up on Devnet
        assert!(prot.feature_flags.lookup_attr("enable_poseidon".to_owned()) == Some(true));
        assert!(
            prot.feature_flags
                .attr_map()
                .get("enable_poseidon")
                .unwrap()
                == &true
        );
    }

    #[test]
    fn limit_range_fn_test() {
        let low = 100u32;
        let high = 10000u64;

        assert!(check_limit!(1u8, low, high) == LimitThresholdCrossed::None);
        assert!(matches!(
            check_limit!(255u16, low, high),
            LimitThresholdCrossed::Soft(255u128, 100)
        ));
        // This wont compile because lossy
        // assert!(check_limit!(100000000u128, low, high) ==
        // LimitThresholdCrossed::None); This wont compile because lossy
        // assert!(check_limit!(100000000usize, low, high) ==
        // LimitThresholdCrossed::None);

        assert!(matches!(
            check_limit!(2550000u64, low, high),
            LimitThresholdCrossed::Hard(2550000, 10000)
        ));

        assert!(matches!(
            check_limit!(2550000u64, high, high),
            LimitThresholdCrossed::Hard(2550000, 10000)
        ));

        assert!(matches!(
            check_limit!(1u8, high),
            LimitThresholdCrossed::None
        ));

        assert!(check_limit!(255u16, high) == LimitThresholdCrossed::None);

        assert!(matches!(
            check_limit!(2550000u64, high),
            LimitThresholdCrossed::Hard(2550000, 10000)
        ));
    }
}
