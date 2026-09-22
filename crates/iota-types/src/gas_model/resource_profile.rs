// Copyright (c) 2026 IOTA Stiftung
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
/// `TransactionEffects`: it is surfaced through tracing only.
///
/// All counters are derived from deterministic quantities (abstract sizes,
/// counts, serialized bytes), never from wall-clock time or node-local cache
/// state, so they are identical on every validator.
///
/// A transaction whose execution fails still gets a profile, but a partial
/// one: the interpreter, native, and working-memory counters reflect
/// execution up to the failure, while the read-I/O, event, and package-load
/// counters are zero — they are copied from the object runtime and linkage
/// view only when a programmable transaction completes — and the write
/// counters cover only what the failed transaction still commits (the gas
/// coin mutation). Partial in the same way on every validator, so still
/// deterministic; consumers comparing against successful transactions should
/// filter on execution status.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceProfile {
    // CPU time: the sources of time spent holding a worker thread.
    /// The charged instruction count. Past the native-call threshold this
    /// also accumulates native gas amounts, which are charged as virtual
    /// instructions, so it can exceed the number of bytecode instructions
    /// actually executed; `interp_instruction_count` is the clean dispatch
    /// count.
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
    /// Abstract size of the argument values passed to each native function,
    /// keyed like `native_calls_by_function`. References are followed only
    /// when they point at primitive data (a `&vector<u8>` argument counts
    /// the vector's bytes, not the reference), which covers every native
    /// whose cost scales with its input; a reference to structured data
    /// counts at its constant reference size, so computing the size never
    /// walks data that no gas charge covers (a flat-priced native taking
    /// `&T`, like `object::borrow_uid`, would otherwise trigger a walk of
    /// the whole object on every call). The one structured byte-consumer,
    /// `bcs::to_bytes`, sizes its input itself as part of its charged work,
    /// so its byte count is recoverable from `native_gas_by_function` and
    /// its known per-byte rate. Deterministic, like all abstract sizes.
    /// Together with the call count this prices native work directly
    /// (per call + per input byte), independent of the gas cost parameters;
    /// for streaming natives (hashing) it also feeds the memory-bandwidth
    /// dimension's per-function moved-bytes weights.
    pub native_input_bytes_by_function: BTreeMap<String, u64>,
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
    /// Number of distinct non-system packages the adapter fetched directly
    /// for this transaction (call targets, publish/upgrade dependencies,
    /// linkage contexts). These fetches are issued per transaction regardless
    /// of node-local cache state, so the count is deterministic. Module loads
    /// driven by the VM's loader are deliberately not counted: the loader's
    /// module cache belongs to the per-epoch executor, so whether a load
    /// reaches the store depends on node-local history. A package reached
    /// only as a transitive dependency of a call is therefore not part of
    /// this count.
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
    /// Number of events emitted; events are committed with the transaction.
    pub event_count: u64,
    /// Total serialized bytes of emitted events.
    pub event_bytes: u64,
}
