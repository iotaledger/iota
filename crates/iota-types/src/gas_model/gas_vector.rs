// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Deterministic products of a [`ResourceProfile`] and the protocol config's
//! gas-vector coefficients: the predicted `cpu_time` and the weighted
//! `moved_bytes` sum attested in `AttestationData::V2`, and the validity
//! comparison tying the two to the memory-bandwidth ceiling.
//!
//! The attesting validator prices its dry-run's profile with these functions,
//! and every validator recomputes the same values from the actual counters
//! after execution — so all arithmetic is integer, checked, and rounds
//! divisions up (uncertainty resolves upward). No floats, no node-local
//! state. Native functions are priced directly on their deterministic
//! observables (call count and abstract input bytes), never on the gas the
//! cost tables charge for them.

use iota_protocol_config::{GasVectorCoefficientsV1, ProtocolConfig};

use super::resource_profile::ResourceProfile;

/// Femtoseconds per nanosecond: coefficient values are stored in
/// femtoseconds, predictions are returned in nanoseconds.
const FS_PER_NS: u128 = 1_000_000;

/// Denominator of basis-point fixed-point values (10_000 = ×1.0).
const BPS_DENOMINATOR: u128 = 10_000;

const NS_PER_SEC: u128 = 1_000_000_000;

/// Predicted execution time of `profile` in reference-machine nanoseconds:
/// `(fixed overhead + Σ counter × coefficient) × safety multiplier`, every
/// division rounded up.
///
/// Returns `None` when the table cannot price the profile: a native function
/// is missing from `native_functions`, the arithmetic overflows, or the
/// result is zero (a zero `cpu_time` is not attestable). Callers fall back
/// to not producing a gas vector for such a transaction.
pub fn predicted_cpu_time_ns(
    profile: &ResourceProfile,
    table: &GasVectorCoefficientsV1,
) -> Option<u64> {
    let scalar_terms: [(u64, u64); 17] = [
        (
            profile.interp_instruction_count,
            table.interp_instruction_count_fs,
        ),
        (
            profile.interp_stack_size_flow,
            table.interp_stack_size_flow_fs,
        ),
        (
            profile.interp_stack_height_flow,
            table.interp_stack_height_flow_fs,
        ),
        (
            profile.stack_size_high_water_mark,
            table.stack_size_high_water_mark_fs,
        ),
        (
            profile.locals_size_high_water_mark,
            table.locals_size_high_water_mark_fs,
        ),
        (
            profile.object_runtime_cached_bytes,
            table.object_runtime_cached_bytes_fs,
        ),
        (profile.input_object_count, table.input_object_count_fs),
        (profile.input_object_bytes, table.input_object_bytes_fs),
        (profile.child_object_reads, table.child_object_reads_fs),
        (
            profile.child_object_read_bytes,
            table.child_object_read_bytes_fs,
        ),
        (profile.packages_loaded, table.packages_loaded_fs),
        (profile.package_bytes_loaded, table.package_bytes_loaded_fs),
        (profile.written_object_count, table.written_object_count_fs),
        (profile.written_bytes, table.written_bytes_fs),
        (profile.deleted_object_count, table.deleted_object_count_fs),
        (profile.event_count, table.event_count_fs),
        (profile.event_bytes, table.event_bytes_fs),
    ];

    let mut total_fs: u128 = table.fixed_overhead_fs as u128;
    for (count, coefficient_fs) in scalar_terms {
        total_fs = total_fs.checked_add((count as u128).checked_mul(coefficient_fs as u128)?)?;
    }
    for (function, calls) in &profile.native_calls_by_function {
        let cost = table.native_functions.get(function)?;
        total_fs =
            total_fs.checked_add((*calls as u128).checked_mul(cost.cpu_time_call_fs as u128)?)?;
    }
    for (function, input_bytes) in &profile.native_input_bytes_by_function {
        let cost = table.native_functions.get(function)?;
        total_fs = total_fs.checked_add(
            (*input_bytes as u128).checked_mul(cost.cpu_time_input_byte_fs as u128)?,
        )?;
    }

    let with_margin_fs = total_fs
        .checked_mul(table.safety_multiplier_bps as u128)?
        .div_ceil(BPS_DENOMINATOR);
    let ns = with_margin_fs.div_ceil(FS_PER_NS);
    if ns == 0 {
        return None;
    }
    u64::try_from(ns).ok()
}

/// The weighted sum of the bytes `profile` moved through the shared
/// memory/store path during execution: reads and working-set growth at their
/// byte counts, plus a per-operation equivalent for each storage read and
/// each streaming native's input bytes at its per-function weight (divisions
/// rounded up).
///
/// Returns `None` when a native function in the profile is missing from the
/// table (the same rule as the cpu_time prediction) or on arithmetic
/// overflow. Zero is a valid result — a pure-compute transaction moves
/// nothing.
pub fn moved_bytes(profile: &ResourceProfile, table: &GasVectorCoefficientsV1) -> Option<u64> {
    let plain_bytes: u128 = [
        profile.input_object_bytes,
        profile.child_object_read_bytes,
        profile.stack_size_high_water_mark,
        profile.locals_size_high_water_mark,
        profile.object_runtime_cached_bytes,
    ]
    .iter()
    .map(|&bytes| bytes as u128)
    .sum();

    let read_ops =
        (profile.input_object_count as u128).checked_add(profile.child_object_reads as u128)?;
    let read_op_equivalent = read_ops.checked_mul(table.moved_bytes_per_read_op as u128)?;

    let mut native_equivalent: u128 = 0;
    for (function, input_bytes) in &profile.native_input_bytes_by_function {
        let cost = table.native_functions.get(function)?;
        let weighted = (*input_bytes as u128)
            .checked_mul(cost.moved_bytes_per_input_byte_bps as u128)?
            .div_ceil(BPS_DENOMINATOR);
        native_equivalent = native_equivalent.checked_add(weighted)?;
    }

    let total = plain_bytes
        .checked_add(read_op_equivalent)?
        .checked_add(native_equivalent)?;
    u64::try_from(total).ok()
}

/// Whether a declared `cpu_time` is long enough for the declared
/// `moved_bytes` at the given bandwidth — that is, whether the transaction's
/// average declared rate `moved_bytes / cpu_time` stays at or below
/// `bandwidth_bytes_per_sec`. Compared exactly by cross-multiplication, so
/// there is no division and no rounding.
pub fn cpu_time_covers_moved_bytes(
    cpu_time_ns: u64,
    moved_bytes: u64,
    bandwidth_bytes_per_sec: u64,
) -> bool {
    // u64 × u64 (and u64 × NS_PER_SEC) cannot overflow u128.
    (moved_bytes as u128) * NS_PER_SEC <= (cpu_time_ns as u128) * (bandwidth_bytes_per_sec as u128)
}

/// The shortest `cpu_time` a transaction moving `moved_bytes` may declare at
/// the given bandwidth: `ceil(moved_bytes / bandwidth)` in nanoseconds.
/// [`cpu_time_covers_moved_bytes`] holds at this value by construction.
/// `None` when the bandwidth is zero or the floor exceeds `u64`.
pub fn min_cpu_time_ns(moved_bytes: u64, bandwidth_bytes_per_sec: u64) -> Option<u64> {
    if bandwidth_bytes_per_sec == 0 {
        return None;
    }
    // u64 × NS_PER_SEC cannot overflow u128.
    let floor_ns = ((moved_bytes as u128) * NS_PER_SEC).div_ceil(bandwidth_bytes_per_sec as u128);
    u64::try_from(floor_ns).ok()
}

/// The attested triple: predicted execution time in reference-machine
/// nanoseconds, weighted moved bytes, and write-cost bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GasVector {
    pub cpu_time: u64,
    pub moved_bytes: u64,
    pub write_bytes: u64,
}

/// The gas vector a dry-run's resource profile declares under `config`'s
/// constants — the payload of `AttestationData::V2`.
///
/// `cpu_time` is [`predicted_cpu_time_ns`] raised — when the memory-bandwidth
/// ceiling is configured — to [`min_cpu_time_ns`]: a declared duration can
/// never be shorter than the time the memory path needs for the declared
/// bytes. `write_bytes` is written-object bytes plus event bytes (a
/// deletion's write-cost equivalent joins when its constant ships).
///
/// `None` when the config carries no coefficient table, the table cannot
/// price the profile, or the arithmetic overflows; the caller then attests
/// the previous payload version instead. Deterministic: every validator
/// computing this from the same profile and config gets the same vector,
/// which is what makes attested-vs-actual divergence checkable.
pub fn declared_gas_vector(
    profile: &ResourceProfile,
    config: &ProtocolConfig,
) -> Option<GasVector> {
    let table = config.gas_vector_coefficients()?;
    let moved_bytes = self::moved_bytes(profile, table)?;
    let mut cpu_time = predicted_cpu_time_ns(profile, table)?;
    // A zero bandwidth in the table means no ceiling is calibrated, so the
    // rate rule is not applied.
    let bandwidth = table.memory_bandwidth_bytes_per_sec;
    if bandwidth != 0 {
        cpu_time = cpu_time.max(min_cpu_time_ns(moved_bytes, bandwidth)?);
    }
    let write_bytes = profile.written_bytes.checked_add(profile.event_bytes)?;
    Some(GasVector {
        cpu_time,
        moved_bytes,
        write_bytes,
    })
}

/// The outcome of recomputing the gas vector from a transaction's actual
/// execution counters and comparing it to the attested one.
///
/// Deterministic: the actual counters and the config are identical on every
/// validator, so every validator reaches the same outcome for the same
/// commit. Anything other than [`Self::Match`] is evidence about the
/// attestation — either the attestor mispriced its dry-run, or shared-object
/// state changed between the dry-run and execution (the attested object
/// versions adjudicate which) — and never grounds to abort the user's
/// transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasVectorComparison {
    /// Recomputation reproduces the attested vector exactly.
    Match,
    /// Recomputation succeeded but differs from the attested vector.
    Divergent { recomputed: GasVector },
    /// The actual profile cannot be priced by the coefficient table (for
    /// example, execution reached a native function the table does not
    /// list), so no vector can be recomputed. The attestor priced *its*
    /// dry-run, so this too marks a difference between the two runs.
    Unpriceable,
}

impl GasVectorComparison {
    /// Label for the comparison-outcome metric.
    pub fn metric_label(&self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Divergent { .. } => "divergent",
            Self::Unpriceable => "unpriceable",
        }
    }
}

/// Recomputes the gas vector from `actual_profile` with the same function the
/// attestor used on its dry-run profile, and compares it to `attested`.
pub fn compare_attested_gas_vector(
    attested: GasVector,
    actual_profile: &ResourceProfile,
    config: &ProtocolConfig,
) -> GasVectorComparison {
    match declared_gas_vector(actual_profile, config) {
        None => GasVectorComparison::Unpriceable,
        Some(recomputed) if recomputed == attested => GasVectorComparison::Match,
        Some(recomputed) => GasVectorComparison::Divergent { recomputed },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use iota_protocol_config::NativeFunctionCostV1;

    use super::*;

    fn table_pricing_instructions_and_one_hash() -> GasVectorCoefficientsV1 {
        GasVectorCoefficientsV1 {
            interp_instruction_count_fs: 1_500_000, // 1.5 ns per instruction
            native_functions: BTreeMap::from([(
                "0x2::hash::blake2b256".to_owned(),
                NativeFunctionCostV1 {
                    cpu_time_call_fs: 800_000_000,          // 800 ns per call
                    cpu_time_input_byte_fs: 400_000,        // 0.4 ns per input byte
                    moved_bytes_per_input_byte_bps: 31_000, // ×3.1
                },
            )]),
            moved_bytes_per_read_op: 5_300,
            fixed_overhead_fs: 25_000_000_000, // 25 µs
            safety_multiplier_bps: 15_000,     // ×1.5
            ..Default::default()
        }
    }

    #[test]
    fn predicted_cpu_time_matches_hand_computation() {
        let profile = ResourceProfile {
            interp_instruction_count: 1_000,
            native_calls_by_function: BTreeMap::from([("0x2::hash::blake2b256".to_owned(), 3)]),
            native_input_bytes_by_function: BTreeMap::from([(
                "0x2::hash::blake2b256".to_owned(),
                6_000,
            )]),
            ..Default::default()
        };
        // fs total: 25e9 + 1000×1.5e6 + 3×8e8 + 6000×4e5 = 31_300_000_000
        // ×1.5 = 46_950_000_000 fs = 46_950 ns, both divisions exact.
        assert_eq!(
            predicted_cpu_time_ns(&profile, &table_pricing_instructions_and_one_hash()),
            Some(46_950)
        );
    }

    #[test]
    fn predicted_cpu_time_rounds_divisions_up() {
        // One instruction at 1 fs, multiplier ×1.0001: 1 fs × 10_001 / 10_000
        // rounds up to 2 fs, and 2 fs rounds up to 1 ns.
        let table = GasVectorCoefficientsV1 {
            interp_instruction_count_fs: 1,
            safety_multiplier_bps: 10_001,
            ..Default::default()
        };
        let profile = ResourceProfile {
            interp_instruction_count: 1,
            ..Default::default()
        };
        assert_eq!(predicted_cpu_time_ns(&profile, &table), Some(1));
    }

    #[test]
    fn native_function_missing_from_table_is_not_priced() {
        let profile = ResourceProfile {
            interp_instruction_count: 1_000,
            native_calls_by_function: BTreeMap::from([(
                "0x2::ed25519::ed25519_verify".to_owned(),
                1,
            )]),
            native_input_bytes_by_function: BTreeMap::from([(
                "0x2::ed25519::ed25519_verify".to_owned(),
                160,
            )]),
            ..Default::default()
        };
        let table = table_pricing_instructions_and_one_hash();
        assert_eq!(predicted_cpu_time_ns(&profile, &table), None);
        assert_eq!(moved_bytes(&profile, &table), None);
    }

    #[test]
    fn zero_prediction_is_not_priced() {
        // An all-zero table cannot produce an attestable (nonzero) cpu_time.
        assert_eq!(
            predicted_cpu_time_ns(
                &ResourceProfile::default(),
                &GasVectorCoefficientsV1::default()
            ),
            None
        );
    }

    #[test]
    fn arithmetic_overflow_is_not_priced() {
        let table = GasVectorCoefficientsV1 {
            interp_instruction_count_fs: u64::MAX,
            interp_stack_size_flow_fs: u64::MAX,
            safety_multiplier_bps: 10_000,
            ..Default::default()
        };
        // Each u64::MAX × u64::MAX product fits in u128; their sum does not.
        let profile = ResourceProfile {
            interp_instruction_count: u64::MAX,
            interp_stack_size_flow: u64::MAX,
            ..Default::default()
        };
        assert_eq!(predicted_cpu_time_ns(&profile, &table), None);
    }

    #[test]
    fn moved_bytes_is_the_weighted_sum() {
        let profile = ResourceProfile {
            input_object_bytes: 1_000,
            child_object_read_bytes: 200,
            stack_size_high_water_mark: 50,
            locals_size_high_water_mark: 30,
            object_runtime_cached_bytes: 20,
            input_object_count: 2,
            child_object_reads: 3,
            native_input_bytes_by_function: BTreeMap::from([(
                "0x2::hash::blake2b256".to_owned(),
                3,
            )]),
            ..Default::default()
        };
        // plain 1300 + 5 ops × 5300 + ceil(3 × 31_000 / 10_000) = 1300 +
        // 26_500 + 10 = 27_810 (the hash term rounds 9.3 up to 10).
        assert_eq!(
            moved_bytes(&profile, &table_pricing_instructions_and_one_hash()),
            Some(27_810)
        );
    }

    #[test]
    fn moved_bytes_zero_for_pure_compute() {
        assert_eq!(
            moved_bytes(
                &ResourceProfile::default(),
                &table_pricing_instructions_and_one_hash()
            ),
            Some(0)
        );
    }

    fn config_with(
        table: Option<GasVectorCoefficientsV1>,
        bandwidth: Option<u64>,
    ) -> ProtocolConfig {
        let mut config = ProtocolConfig::get_for_max_version_UNSAFE();
        if let Some(mut table) = table {
            if let Some(bandwidth) = bandwidth {
                table.memory_bandwidth_bytes_per_sec = bandwidth;
            }
            config.set_gas_vector_coefficients_for_testing(table);
        }
        config
    }

    #[test]
    fn declared_gas_vector_matches_its_components() {
        let profile = ResourceProfile {
            interp_instruction_count: 1_000,
            written_bytes: 700,
            event_bytes: 44,
            ..Default::default()
        };
        let table = table_pricing_instructions_and_one_hash();
        let config = config_with(Some(table.clone()), Some(1_000_000_000));
        let vector = declared_gas_vector(&profile, &config).unwrap();
        // Prediction dominates the bandwidth floor here (nothing moved).
        assert_eq!(
            Some(vector.cpu_time),
            predicted_cpu_time_ns(&profile, &table)
        );
        assert_eq!(Some(vector.moved_bytes), moved_bytes(&profile, &table));
        assert_eq!(vector.write_bytes, 744);
    }

    #[test]
    fn declared_cpu_time_is_raised_to_the_bandwidth_floor() {
        // A cheap prediction moving many bytes: 1 MB at 1 GB/s needs 1 ms,
        // far above the ~38 µs prediction, so the floor wins and the rate
        // comparison holds by construction.
        let profile = ResourceProfile {
            interp_instruction_count: 1_000,
            input_object_bytes: 1_000_000,
            ..Default::default()
        };
        let config = config_with(
            Some(table_pricing_instructions_and_one_hash()),
            Some(1_000_000_000),
        );
        let vector = declared_gas_vector(&profile, &config).unwrap();
        assert_eq!(
            vector.cpu_time,
            min_cpu_time_ns(vector.moved_bytes, 1_000_000_000).unwrap()
        );
        assert!(cpu_time_covers_moved_bytes(
            vector.cpu_time,
            vector.moved_bytes,
            1_000_000_000
        ));
        // Without the bandwidth constant the floor is dormant.
        let config = config_with(Some(table_pricing_instructions_and_one_hash()), None);
        let raw = declared_gas_vector(&profile, &config).unwrap();
        assert!(raw.cpu_time < vector.cpu_time);
    }

    #[test]
    fn declared_gas_vector_requires_the_table() {
        let profile = ResourceProfile {
            interp_instruction_count: 1_000,
            ..Default::default()
        };
        assert_eq!(
            declared_gas_vector(&profile, &config_with(None, Some(1_000_000_000))),
            None
        );
    }

    #[test]
    fn min_cpu_time_rounds_up_and_rejects_zero_bandwidth() {
        // 3 bytes at 2 B/s = 1.5 s, rounded up to 1_500_000_000 ns.
        assert_eq!(min_cpu_time_ns(3, 2), Some(1_500_000_000));
        assert_eq!(min_cpu_time_ns(0, 5), Some(0));
        assert_eq!(min_cpu_time_ns(1, 0), None);
    }

    #[test]
    fn rate_comparison_is_exact_at_the_boundary() {
        // 1000 bytes in 1000 ns at 1 GB/s: exactly one byte per ns — holds.
        assert!(cpu_time_covers_moved_bytes(1_000, 1_000, 1_000_000_000));
        // One more byte in the same time exceeds the bandwidth.
        assert!(!cpu_time_covers_moved_bytes(1_000, 1_001, 1_000_000_000));
        // Zero declared time covers zero bytes and nothing else.
        assert!(cpu_time_covers_moved_bytes(0, 0, 1_000_000_000));
        assert!(!cpu_time_covers_moved_bytes(0, 1, 1_000_000_000));
    }

    #[test]
    fn comparison_matches_when_the_actual_profile_equals_the_attested_one() {
        let profile = ResourceProfile {
            interp_instruction_count: 1_000,
            written_bytes: 700,
            event_bytes: 44,
            ..Default::default()
        };
        let config = config_with(
            Some(table_pricing_instructions_and_one_hash()),
            Some(1_000_000_000),
        );
        let attested = declared_gas_vector(&profile, &config).unwrap();
        assert_eq!(
            compare_attested_gas_vector(attested, &profile, &config),
            GasVectorComparison::Match
        );
    }

    #[test]
    fn comparison_reports_divergence_with_the_recomputed_vector() {
        let attested_profile = ResourceProfile {
            interp_instruction_count: 1_000,
            ..Default::default()
        };
        let actual_profile = ResourceProfile {
            interp_instruction_count: 2_000,
            written_bytes: 10,
            ..Default::default()
        };
        let config = config_with(
            Some(table_pricing_instructions_and_one_hash()),
            Some(1_000_000_000),
        );
        let attested = declared_gas_vector(&attested_profile, &config).unwrap();
        let recomputed = declared_gas_vector(&actual_profile, &config).unwrap();
        assert_ne!(attested, recomputed);
        assert_eq!(
            compare_attested_gas_vector(attested, &actual_profile, &config),
            GasVectorComparison::Divergent { recomputed }
        );
    }

    #[test]
    fn comparison_reports_unpriceable_actual_profiles() {
        // The actual run reached a native function the table does not list,
        // so no vector can be recomputed — itself a divergence signal, since
        // the attestor priced its dry-run.
        let actual_profile = ResourceProfile {
            interp_instruction_count: 1_000,
            native_calls_by_function: BTreeMap::from([(
                "0x2::ed25519::ed25519_verify".to_owned(),
                1,
            )]),
            ..Default::default()
        };
        let config = config_with(
            Some(table_pricing_instructions_and_one_hash()),
            Some(1_000_000_000),
        );
        let attested = GasVector {
            cpu_time: 50_000,
            moved_bytes: 0,
            write_bytes: 0,
        };
        assert_eq!(
            compare_attested_gas_vector(attested, &actual_profile, &config),
            GasVectorComparison::Unpriceable
        );
    }

    #[test]
    fn comparison_metric_labels_are_distinct() {
        let labels = [
            GasVectorComparison::Match.metric_label(),
            GasVectorComparison::Divergent {
                recomputed: GasVector {
                    cpu_time: 1,
                    moved_bytes: 0,
                    write_bytes: 0,
                },
            }
            .metric_label(),
            GasVectorComparison::Unpriceable.metric_label(),
        ];
        assert_eq!(labels, ["match", "divergent", "unpriceable"]);
    }
}
