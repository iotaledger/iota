// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_stardust_sdk::types::block::{
    address::Ed25519Address,
    output::{
        BasicOutput, BasicOutputBuilder, NativeToken, OutputId, TokenId,
        unlock_condition::{
            AddressUnlockCondition, StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
        },
    },
};

use crate::{
    balance::Balance,
    base_types::ObjectID,
    id::UID,
    timelock::{
        label::label_struct_tag_to_string,
        stardust_upgrade_label::{STARDUST_UPGRADE_LABEL_VALUE, stardust_upgrade_label_type},
        timelock::{TimeLock, VestedRewardError, is_timelocked_vested_reward, try_from_stardust},
    },
};

fn vested_reward_output(amount: u64, expiration_time_sec: u32) -> BasicOutput {
    BasicOutputBuilder::new_with_amount(amount)
        .add_unlock_condition(AddressUnlockCondition::new(
            Ed25519Address::from_str(
                "0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655",
            )
            .unwrap(),
        ))
        .add_unlock_condition(TimelockUnlockCondition::new(expiration_time_sec).unwrap())
        .finish()
        .unwrap()
}

#[test]
fn timelock_ser_deser_roundtrip() {
    let id = UID::new(ObjectID::random());
    let balance = Balance::new(100);
    let expiration_timestamp_ms = 10;
    let label = Option::Some(label_struct_tag_to_string(stardust_upgrade_label_type()));

    let timelock = TimeLock::new(id, balance, expiration_timestamp_ms, label);

    let timelock_bytes = timelock.to_bcs_bytes();
    let deserialized_timelock: TimeLock<Balance> = bcs::from_bytes(&timelock_bytes).unwrap();

    assert_eq!(deserialized_timelock.id(), timelock.id());
    assert_eq!(deserialized_timelock.locked(), timelock.locked());
    assert_eq!(
        deserialized_timelock.expiration_timestamp_ms(),
        timelock.expiration_timestamp_ms()
    );
    assert_eq!(deserialized_timelock.label(), timelock.label());
}

#[test]
fn is_timelocked_vested_reward_all_correct() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
    )
    .unwrap();
    let output = vested_reward_output(10, 1000);

    assert!(is_timelocked_vested_reward(output_id, &output, 100));
}

#[test]
fn is_timelocked_vested_reward_min_id() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18000000000000",
    )
    .unwrap();
    let output = vested_reward_output(10, 1000);

    assert!(is_timelocked_vested_reward(output_id, &output, 100));
}

#[test]
fn is_timelocked_vested_reward_max_id() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18ffffffff0000",
    )
    .unwrap();
    let output = vested_reward_output(10, 1000);

    assert!(is_timelocked_vested_reward(output_id, &output, 100));
}

#[test]
fn is_timelocked_vested_reward_incorrect_id() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb17123456780000",
    )
    .unwrap();
    let output = vested_reward_output(10, 1000);

    assert!(!is_timelocked_vested_reward(output_id, &output, 100));
}

#[test]
fn is_timelocked_vested_reward_no_timelock_unlock_condition() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
    )
    .unwrap();
    let output = BasicOutputBuilder::new_with_amount(10)
        .add_unlock_condition(AddressUnlockCondition::new(
            Ed25519Address::from_str(
                "0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655",
            )
            .unwrap(),
        ))
        .finish()
        .unwrap();

    assert!(!is_timelocked_vested_reward(output_id, &output, 100));
}

#[test]
fn is_timelocked_vested_reward_bigger_milestone_time() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
    )
    .unwrap();
    let output = vested_reward_output(10, 100);

    assert!(!is_timelocked_vested_reward(output_id, &output, 1000));
}

#[test]
fn is_timelocked_vested_reward_same_milestone_time() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
    )
    .unwrap();
    let output = vested_reward_output(10, 1000);

    assert!(!is_timelocked_vested_reward(output_id, &output, 1000));
}

#[test]
fn timelock_from_stardust_all_correct() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
    )
    .unwrap();
    let output = vested_reward_output(10, 1000);

    let timelock = try_from_stardust(output_id, &output, 100).unwrap();

    assert!(timelock.locked().value() == 10);
    assert!(timelock.expiration_timestamp_ms() == 1_000_000);
    assert!(timelock.label().as_ref().unwrap() == STARDUST_UPGRADE_LABEL_VALUE);
}

#[test]
fn timelock_from_stardust_with_expired_output() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
    )
    .unwrap();
    let output = vested_reward_output(10, 1000);

    let err = try_from_stardust(output_id, &output, 1000).unwrap_err();

    assert!(matches!(err, VestedRewardError::UnlockedVestedReward));
}

#[test]
fn timelock_from_stardust_with_incorrect_id() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb17123456780000",
    )
    .unwrap();
    let output = vested_reward_output(10, 1000);

    let err = try_from_stardust(output_id, &output, 100).unwrap_err();

    assert!(matches!(err, VestedRewardError::NotVestedReward));
}

#[test]
fn timelock_from_stardust_without_timelock_unlock_condition() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
    )
    .unwrap();
    let output = BasicOutputBuilder::new_with_amount(10)
        .add_unlock_condition(AddressUnlockCondition::new(
            Ed25519Address::from_str(
                "0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655",
            )
            .unwrap(),
        ))
        .add_unlock_condition(
            StorageDepositReturnUnlockCondition::new(
                Ed25519Address::from_str(
                    "0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655",
                )
                .unwrap(),
                100,
                100,
            )
            .unwrap(),
        )
        .finish()
        .unwrap();

    let err = try_from_stardust(output_id, &output, 1000).unwrap_err();

    assert!(matches!(err, VestedRewardError::NotVestedReward));
}

#[test]
fn timelock_from_stardust_extra_unlock_condition() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
    )
    .unwrap();
    let output = BasicOutputBuilder::new_with_amount(10)
        .add_unlock_condition(AddressUnlockCondition::new(
            Ed25519Address::from_str(
                "0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655",
            )
            .unwrap(),
        ))
        .add_unlock_condition(TimelockUnlockCondition::new(1000).unwrap())
        .add_unlock_condition(
            StorageDepositReturnUnlockCondition::new(
                Ed25519Address::from_str(
                    "0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655",
                )
                .unwrap(),
                100,
                100,
            )
            .unwrap(),
        )
        .finish()
        .unwrap();

    let err = try_from_stardust(output_id, &output, 100).unwrap_err();

    assert!(matches!(err, VestedRewardError::NotVestedReward));
}

#[test]
fn timelock_from_stardust_with_native_tokens() {
    let output_id = OutputId::from_str(
        "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
    )
    .unwrap();
    let output = BasicOutputBuilder::new_with_amount(10)
        .add_unlock_condition(AddressUnlockCondition::new(
            Ed25519Address::from_str(
                "0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655",
            )
            .unwrap(),
        ))
        .add_unlock_condition(TimelockUnlockCondition::new(1000).unwrap())
        .add_native_token(NativeToken::new(TokenId::null(), 1).unwrap())
        .finish()
        .unwrap();

    let err = try_from_stardust(output_id, &output, 100).unwrap_err();

    assert!(matches!(err, VestedRewardError::NotVestedReward));
}
