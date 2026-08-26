// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Per-transaction profile of the physical resources whose costs computation
/// gas sums into a single scalar.
///
/// The profile decomposes execution into per-resource signals so that offline
/// calibration and observability can ask "how much was interpretation vs
/// native calls vs reads vs memory vs writes" — a question the single summed
/// gas number cannot answer. It is accumulated alongside gas metering without
/// changing any gas charge, and it must never be serialized into
/// `TransactionEffects`: it is surfaced through tracing and metrics only.
///
/// All counters are derived from deterministic quantities (abstract sizes,
/// counts, serialized bytes), never from wall-clock time or node-local cache
/// state, so they are identical on every validator.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceProfile {
    // CPU time: the sources of time spent holding a worker thread.
    /// Total number of bytecode instructions executed.
    pub instructions_executed: u64,
    /// Number of native function calls.
    pub num_native_calls: u64,
    /// Internal gas deducted for bytecode interpretation (instructions and
    /// operand-stack operations), i.e. the computation total minus native
    /// calls and per-byte charges. The bundled interpreter total.
    pub interpreter_gas: u64,
    /// The three untiered components of interpreter gas, recorded separately so
    /// each can be calibrated independently rather than as the bundled total:
    /// instruction dispatch count, operand-stack bytes moved (Σ of size
    /// increments), and pushes (Σ of stack-height increments). Interpreter-only
    /// — native calls' share is excluded.
    pub interp_instruction_count: u64,
    pub interp_stack_size_flow: u64,
    pub interp_stack_height_flow: u64,
    /// Container values constructed by bytecode — structs and vectors packed,
    /// container-typed constants loaded — plus elements appended to vectors.
    /// Each is a heap allocation, a cost the instruction count alone does
    /// not carry: a transaction building many small values runs far slower
    /// per instruction than arithmetic does. Deterministic (bytecode events).
    pub values_constructed: u64,
    /// Internal gas deducted by native functions (tiering-correct: the gas
    /// actually charged, not the pre-tiering declared amount). Together with
    /// `interpreter_gas` and the per-byte charges, this sums to the total
    /// computation gas deducted.
    pub native_gas: u64,
    /// `native_gas` split by the native function it was charged for, keyed
    /// by the full module id plus function name (e.g.
    /// `0x2::ed25519::ed25519_verify`), so same-named modules in different
    /// packages stay distinct. Per function rather than per module because
    /// real per-call cost varies far more within a module than the charged
    /// gas does (in `0x2::group_ops`, a pairing costs ~18x a G1 addition),
    /// so a module-level time-per-gas coefficient cannot cover its most
    /// expensive function. Native functions only exist in system packages,
    /// so the key set is bounded. Calibration excludes the storage-access
    /// modules (`dynamic_field`, `object`) from the native CPU term; the
    /// read term owns that cost.
    pub native_gas_by_function: BTreeMap<String, u64>,
    /// Native calls split by function, same keys as `native_gas_by_function`.
    /// Together the two maps give calibration a per-function
    /// (call count, input-size-dependent gas) pair, which spans the same
    /// space as (per-call cost, per-byte cost) — one gas column alone cannot
    /// represent a function whose per-byte gas is disproportionate to its
    /// per-call gas relative to real time (e.g. `ecvrf_verify`).
    pub native_calls_by_function: BTreeMap<String, u64>,
    /// Internal gas deducted by per-byte storage-read charges (input objects
    /// and dynamic-field bytes).
    pub storage_read_gas: u64,
    /// Internal gas deducted by per-byte package publish/upgrade charges.
    /// Kept separate from `storage_read_gas` because the two use different
    /// per-byte rates and price different work (module deserialize + verify
    /// vs. object reads).
    pub package_publish_gas: u64,
    /// Total computation gas used, in gas units, before bucketization. This is
    /// the fee-facing total; `interpreter_gas` + `native_gas` +
    /// `storage_read_gas` + `package_publish_gas` equal it in internal units
    /// (1 gas unit = 1000 internal units).
    pub computation_gas_used: u64,

    // Working memory: abstract byte sizes (the VM's `AbstractMemorySize`),
    // not real RAM bytes; the conversion to real bytes is calibrated offline.
    /// True high-water mark of the operand stack's abstract size: both
    /// increases and decreases are applied, so this is the peak resident
    /// size, not the total bytes ever pushed.
    pub stack_size_high_water_mark: u64,
    /// Total abstract bytes ever pushed onto the operand stack (decreases not
    /// applied). This matches the quantity the gas tiers escalate on, and is
    /// the flow counterpart of the peak above.
    pub stack_size_total_pushed: u64,
    /// High-water mark of the operand stack's height (slot count).
    pub stack_height_high_water_mark: u64,
    /// High-water mark of the abstract size of values held in frame locals.
    /// Locals are not charged by gas, and their bytes are not captured by the
    /// operand stack's own size (values parked in locals and only borrowed
    /// onto the stack), so this is recorded separately. Values grown in place
    /// through a `&mut` reference (e.g. `vector::push_back`) are invisible to
    /// the store/move hooks; the growth is captured at frame drop, when the
    /// dropped values' full size is visible.
    pub locals_size_high_water_mark: u64,
    /// Serialized bytes of child objects retained in the object-runtime
    /// cache, plus the abstract sizes of child objects added during
    /// execution. The cache grows monotonically within a transaction, so this
    /// is also its high-water mark.
    pub object_runtime_cached_bytes: u64,

    // Read I/O.
    /// Number of input objects loaded before execution. Excludes system
    /// packages (matching the storage-read charge) and all other package
    /// dependencies, which are counted by `packages_loaded` instead — a
    /// transaction's input objects include its packages, and double-counting
    /// them here leaves the two counters inseparable in calibration.
    pub input_object_count: u64,
    /// Serialized bytes of the objects counted by `input_object_count`
    /// (packages excluded; their bytes are `package_bytes_loaded`).
    pub input_object_bytes: u64,
    /// Number of child/dynamic-field object loads issued to the store during
    /// execution (including loads that found no object, and received
    /// objects).
    pub child_object_reads: u64,
    /// Serialized bytes of child/dynamic-field objects fetched from the
    /// store during execution.
    pub child_object_read_bytes: u64,
    /// Number of distinct non-system packages fetched for this transaction
    /// (call targets, publish/upgrade dependencies, linkage contexts). Counted
    /// per package per transaction at the store-fetch call, which runs
    /// regardless of node-local cache state, so the count is deterministic.
    /// Module loads driven by the VM's own loader cache are deliberately not
    /// counted — whether they reach the store depends on node-local cache
    /// state.
    pub packages_loaded: u64,
    /// Serialized bytes of the distinct non-system packages counted above.
    pub package_bytes_loaded: u64,

    // Commit write.
    /// Number of objects written (created or mutated) at commit.
    pub written_object_count: u64,
    /// Total post-transaction serialized bytes of written objects.
    pub written_bytes: u64,
    /// Number of objects removed from storage (deleted or wrapped) at
    /// commit.
    pub deleted_object_count: u64,

    // Cardinality.
    /// Number of events emitted.
    pub event_count: u64,
    /// Total serialized bytes of emitted events.
    pub event_bytes: u64,
}
