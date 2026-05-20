// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::{
    balance::{Balance, Supply},
    base_types::ObjectID,
    coin::TreasuryCap,
    collection_types::{Bag, Table, TableVec, VecMap},
    crypto::AuthorityStrongQuorumSignInfo,
    gas::GasCostSummary,
    gas_coin::IotaTreasuryCap,
    id::UID,
    iota_system_state::{
        IotaSystemState, IotaSystemStateTrait,
        iota_system_state_inner_v1::{
            IotaSystemStateV1, StorageFundV1, SystemParametersV1, ValidatorSetV1,
        },
    },
    message_envelope::Envelope,
    messages_checkpoint::CheckpointSummary,
    system_admin_cap::IotaSystemAdminCap,
};

fn empty_checkpoint_summary() -> CheckpointSummary {
    CheckpointSummary {
        epoch: 0,
        sequence_number: 0,
        network_total_transactions: 0,
        content_digest: Default::default(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: GasCostSummary::default(),
        end_of_epoch_data: None,
        timestamp_ms: 0,
        version_specific_data: Vec::new(),
        checkpoint_commitments: Vec::new(),
    }
}

fn empty_certified_summary() -> CertifiedCheckpointSummary {
    let sig = AuthorityStrongQuorumSignInfo {
        epoch: 0,
        signature: Default::default(),
        signers_map: Default::default(),
    };
    Envelope::new_from_data_and_sig(empty_checkpoint_summary(), sig)
}

/// Locks the BCS field order of `EpochInfoEntry` against silent
/// reordering. BCS encodes struct fields in declaration order, so
/// swapping any two fields would silently corrupt every on-disk row
/// in the `epoch_info` column family AND change the on-wire layout
/// under `EpochInfo::V1` in the snapshot.
///
/// Asserts that `bcs(entry)` equals the concatenation:
///   `first_checkpoint.to_le_bytes()
///    ++ uvarint(start_system_state.len()) ++ start_system_state
///    ++ bcs(last_checkpoint_summary: Option<...>)
///    ++ bcs(end_of_epoch_tx_events: Option<...>)`.
/// This both verifies the relative order of the four fields and
/// detects any encoding-shape change in the inner types.
#[test]
fn epoch_info_entry_field_order_is_locked() {
    let entry = EpochInfoEntry {
        // Distinct, recognizable u64 - easy to spot in a hex dump if
        // this assertion ever needs to be debugged.
        first_checkpoint: 0xDEAD_BEEF_CAFE_F00D,
        // Distinct payload so a misordered field would be obvious.
        start_system_state: vec![0xAA, 0xBB, 0xCC, 0xDD],
        last_checkpoint_summary: Some(empty_certified_summary()),
        end_of_epoch_tx_events: Some(TransactionEvents::default()),
    };

    let entry_bytes = bcs::to_bytes(&entry).expect("entry serialization");
    let start_system_state_bytes =
        bcs::to_bytes(&entry.start_system_state).expect("start_system_state serialization");
    let summary_bytes =
        bcs::to_bytes(&entry.last_checkpoint_summary).expect("summary serialization");
    let events_bytes = bcs::to_bytes(&entry.end_of_epoch_tx_events).expect("events serialization");

    let mut expected = Vec::with_capacity(entry_bytes.len());
    expected.extend_from_slice(&entry.first_checkpoint.to_le_bytes());
    expected.extend_from_slice(&start_system_state_bytes);
    expected.extend_from_slice(&summary_bytes);
    expected.extend_from_slice(&events_bytes);

    assert_eq!(
        entry_bytes, expected,
        "EpochInfoEntry BCS layout changed; re-anchor this test only if \
         the schema change is deliberate and reviewers have signed off"
    );
}

/// Build a minimal `IotaSystemStateV1` whose only meaningful fields are
/// `epoch` and `protocol_version`. Everything else is zeroed/defaulted —
/// the test only cares that the BCS round-trip via the outer
/// `IotaSystemState` enum recovers these two values via
/// `IotaSystemStateTrait`.
fn minimal_iota_system_state_v1(epoch: u64, protocol_version: u64) -> IotaSystemStateV1 {
    IotaSystemStateV1 {
        epoch,
        protocol_version,
        system_state_version: 1,
        iota_treasury_cap: IotaTreasuryCap {
            inner: TreasuryCap {
                id: UID::new(ObjectID::ZERO),
                total_supply: Supply { value: 0 },
            },
        },
        validators: ValidatorSetV1 {
            total_stake: 0,
            active_validators: Vec::new(),
            pending_active_validators: TableVec::default(),
            pending_removals: Vec::new(),
            staking_pool_mappings: Table::default(),
            inactive_validators: Table::default(),
            validator_candidates: Table::default(),
            at_risk_validators: VecMap {
                contents: Vec::new(),
            },
            extra_fields: Bag::default(),
        },
        storage_fund: StorageFundV1 {
            total_object_storage_rebates: Balance::new(0),
            non_refundable_balance: Balance::new(0),
        },
        parameters: SystemParametersV1 {
            epoch_duration_ms: 0,
            min_validator_count: 0,
            max_validator_count: 0,
            min_validator_joining_stake: 0,
            validator_low_stake_threshold: 0,
            validator_very_low_stake_threshold: 0,
            validator_low_stake_grace_period: 0,
            extra_fields: Bag::default(),
        },
        iota_system_admin_cap: IotaSystemAdminCap::default(),
        reference_gas_price: 0,
        validator_report_records: VecMap {
            contents: Vec::new(),
        },
        safe_mode: false,
        safe_mode_storage_charges: Balance::new(0),
        safe_mode_computation_rewards: Balance::new(0),
        safe_mode_storage_rebates: 0,
        safe_mode_non_refundable_storage_fee: 0,
        epoch_start_timestamp_ms: 0,
        extra_fields: Bag::default(),
    }
}

/// Mirrors the BCS encoding `grpc_indexes::write_epoch_info_entries`
/// performs into `EpochInfoEntry::start_system_state`:
/// `bcs::to_bytes(&IotaSystemState)`. Asserts that
/// 1. the outer-enum variant tag is pinned at discriminant `0` for `V1`
///    (downstream decoders branch on this first byte), and
/// 2. the bytes round-trip to a typed `IotaSystemState` whose
///    `IotaSystemStateTrait` accessors return the source `epoch` and
///    `protocol_version` — proving the inner per-version payload (here
///    `IotaSystemStateV1`) survives encode → decode.
///
/// Pairs with `iota-snapshot::tests::snapshot_round_trip`, which
/// covers the bit-identical opaque-bytes contract end-to-end; this
/// test isolates the typed encode/decode in `iota-types` so a
/// regression that breaks the `IotaSystemState` BCS shape is caught
/// here without needing the full snapshot pipeline.
#[test]
fn start_system_state_bcs_round_trips_to_typed_iota_system_state() {
    let source = IotaSystemState::V1(minimal_iota_system_state_v1(42, 7));

    let bytes = bcs::to_bytes(&source).expect("BCS encode IotaSystemState");
    assert_eq!(
        bytes[0], 0,
        "IotaSystemState::V1 must remain at BCS discriminant 0; \
         a silent enum reorder would break every decoder of \
         EpochInfoEntry::start_system_state"
    );

    let decoded: IotaSystemState = bcs::from_bytes(&bytes).expect("BCS decode IotaSystemState");
    assert_eq!(decoded.epoch(), 42);
    assert_eq!(decoded.protocol_version(), 7);
}
