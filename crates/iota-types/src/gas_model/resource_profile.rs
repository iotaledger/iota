// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Per-transaction breakdown of the physical resources that computation gas
/// sums into a single number.
///
/// Accumulated alongside gas metering without changing any charge, and
/// surfaced through tracing only; it must never be serialized into
/// `TransactionEffects`. All counters come from deterministic quantities
/// (abstract sizes, counts, serialized bytes), so they are identical on every
/// validator.
///
/// For a failed transaction the profile is partial: the interpreter, native,
/// and working-memory counters cover execution up to the failure, the
/// read-I/O, event, and package-load counters are zero, and the write
/// counters cover every mutable input, since a failed transaction still
/// mutates them all, plus any extra gas coins smashed into the primary one.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceProfile {
    // CPU time.
    /// The charged instruction count. Past the native-call threshold this also
    /// includes native gas charged as virtual instructions; see
    /// `interp_instruction_count` for bytecode instructions only.
    pub instructions_executed: u64,
    /// Number of native function calls.
    pub num_native_calls: u64,
    /// Internal gas deducted for bytecode interpretation: the computation
    /// total minus native calls and per-byte charges.
    pub interpreter_gas: u64,
    /// The untiered components of interpreter gas, excluding native calls:
    /// instruction count, operand-stack bytes pushed, and number of pushes.
    pub interp_instruction_count: u64,
    pub interp_stack_size_flow: u64,
    pub interp_stack_height_flow: u64,
    /// Abstract size of the arguments passed to each native function, keyed
    /// like `native_calls_by_function`. References are followed only into
    /// primitive data (scalars and vectors of scalars); a reference to
    /// structured data counts at its constant reference size.
    pub native_input_bytes_by_function: BTreeMap<String, u64>,
    /// Internal gas actually deducted by native functions, after tiering.
    pub native_gas: u64,
    /// `native_gas` split by native function, keyed by module id plus
    /// function name (e.g. `0x2::ed25519::ed25519_verify`).
    pub native_gas_by_function: BTreeMap<String, u64>,
    /// Native calls split by function, same keys as `native_gas_by_function`.
    pub native_calls_by_function: BTreeMap<String, u64>,
    /// Internal gas deducted by per-byte storage-read charges.
    pub storage_read_gas: u64,
    /// Internal gas deducted by per-byte package publish/upgrade charges.
    pub package_publish_gas: u64,
    /// Total computation gas used, in gas units, before bucketization.
    /// `interpreter_gas + native_gas + storage_read_gas + package_publish_gas`
    /// equals it in internal units (1 gas unit = 1000 internal units).
    pub computation_gas_used: u64,

    // Working memory, in abstract sizes (`AbstractMemorySize`), not RAM bytes.
    /// Peak size of the operand stack.
    pub stack_size_high_water_mark: u64,
    /// Peak height (slot count) of the operand stack.
    pub stack_height_high_water_mark: u64,
    /// Peak size of values held in frame locals, including values grown in
    /// place through `&mut` references (captured when the frame is dropped).
    pub locals_size_high_water_mark: u64,
    /// Serialized bytes of child objects loaded into the object runtime,
    /// plus the abstract sizes of child objects added during execution.
    pub object_runtime_cached_bytes: u64,

    // Read I/O.
    /// Number of non-package input objects loaded before execution.
    pub input_object_count: u64,
    /// Serialized bytes of the objects counted by `input_object_count`.
    pub input_object_bytes: u64,
    /// Number of child object loads issued to the store, including loads that
    /// found no object and received objects.
    pub child_object_reads: u64,
    /// Serialized bytes of child objects fetched from the store.
    pub child_object_read_bytes: u64,
    /// Number of distinct non-system packages the adapter fetched directly
    /// (call targets, publish/upgrade dependencies, linkage contexts).
    /// Module loads by the VM loader are not counted, since they depend on
    /// the loader's per-epoch cache.
    pub packages_loaded: u64,
    /// Serialized bytes of the packages counted by `packages_loaded`.
    pub package_bytes_loaded: u64,

    // Commit write.
    /// Number of objects created or mutated.
    pub written_object_count: u64,
    /// Total serialized bytes of written objects.
    pub written_bytes: u64,
    /// Number of objects deleted or wrapped.
    pub deleted_object_count: u64,
    /// Number of events emitted.
    pub event_count: u64,
    /// Total serialized bytes of emitted events.
    pub event_bytes: u64,
}
