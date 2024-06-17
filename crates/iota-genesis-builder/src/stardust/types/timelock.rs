// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_sdk::types::block::output::{BasicOutput, OutputId};
use iota_types::{
    balance::Balance,
    base_types::{IotaAddress, MoveObjectType, ObjectID, SequenceNumber, TxContext},
    id::UID,
    object::{Data, MoveObject, Object, Owner},
    timelock::{
        label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
        timelock::TimeLock,
    },
};

/// All basic outputs whose IDs start with this prefix represent vested rewards
/// that were created during the stardust upgrade on IOTA mainnet.
const VESTED_REWARD_ID_PREFIX: &str = "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18";

#[derive(Debug, thiserror::Error)]
pub enum VestedRewardError {
    #[error("failed to create genesis move object, owner: {owner}, timelock: {timelock:#?}")]
    ObjectCreation {
        owner: IotaAddress,
        timelock: TimeLock<Balance>,
        source: iota_types::error::ExecutionError,
    },
    #[error("a vested reward must not contain native tokens")]
    NativeTokensNotSupported,
    #[error("a basic output is not a vested reward")]
    NotVestedReward,
    #[error("a vested reward must have two unlock conditions")]
    UnlockConditionsNumberMismatch,
    #[error("only timelocked vested rewards can be migrated as `TimeLock<Balance<IOTA>>`")]
    UnlockedVestedReward,
}

/// Checks if an output is a timelocked vested reward.
pub fn is_timelocked_vested_reward(
    output_id: OutputId,
    basic_output: &BasicOutput,
    target_milestone_timestamp_sec: u32,
) -> bool {
    is_vested_reward(output_id, basic_output)
        && basic_output
            .unlock_conditions()
            .is_time_locked(target_milestone_timestamp_sec)
}

/// Checks if an output is a vested reward, if it has a specific ID prefix,
/// and if it contains a timelock unlock condition.
fn is_vested_reward(output_id: OutputId, basic_output: &BasicOutput) -> bool {
    let has_vesting_prefix = output_id.to_string().starts_with(VESTED_REWARD_ID_PREFIX);

    has_vesting_prefix && basic_output.unlock_conditions().timelock().is_some()
}

/// Creates a `TimeLock<Balance<IOTA>>` from a Stardust-based Basic Output
/// that represents a vested reward.
pub fn try_from_stardust(
    output_id: OutputId,
    basic_output: &BasicOutput,
    target_milestone_timestamp_sec: u32,
) -> Result<TimeLock<Balance>, VestedRewardError> {
    if !is_vested_reward(output_id, basic_output) {
        return Err(VestedRewardError::NotVestedReward);
    }

    if !basic_output
        .unlock_conditions()
        .is_time_locked(target_milestone_timestamp_sec)
    {
        return Err(VestedRewardError::UnlockedVestedReward);
    }

    if basic_output.unlock_conditions().len() != 2 {
        return Err(VestedRewardError::UnlockConditionsNumberMismatch);
    }

    if basic_output.native_tokens().len() > 0 {
        return Err(VestedRewardError::NativeTokensNotSupported);
    }

    let id = UID::new(ObjectID::new(output_id.hash()));
    let locked = Balance::new(basic_output.amount());

    // We already checked the existence of the timelock unlock condition at this
    // point.
    let timelock_uc = basic_output
        .unlock_conditions()
        .timelock()
        .expect("a vested reward should contain a timelock unlock condition");
    let expiration_timestamp_ms = Into::<u64>::into(timelock_uc.timestamp()) * 1000;

    let label = Option::Some(label_struct_tag_to_string(stardust_upgrade_label_type()));

    Ok(iota_types::timelock::timelock::TimeLock::new(
        id,
        locked,
        expiration_timestamp_ms,
        label,
    ))
}

/// Creates a genesis object from a time-locked balance.
pub fn to_genesis_object(
    timelock: TimeLock<Balance>,
    owner: IotaAddress,
    protocol_config: &ProtocolConfig,
    tx_context: &TxContext,
    version: SequenceNumber,
    type_tag: &TypeTag,
) -> Result<Object, VestedRewardError> {
    let move_object = unsafe {
        // Safety: we know from the definition of `TimeLock` in the timelock package
        // that it is not publicly transferable (`store` ability is absent).
        MoveObject::new_from_execution(
            MoveObjectType::timelocked_iota_balance(),
            false,
            version,
            timelock.to_bcs_bytes(),
            protocol_config,
        )
        .map_err(|source| VestedRewardError::ObjectCreation {
            owner,
            timelock,
            source,
        })?
    };

    Ok(Object::new_from_genesis(
        Data::Move(move_object),
        Owner::AddressOwner(owner),
        tx_context.digest(),
    ))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iota_sdk::types::block::{
        address::Ed25519Address,
        output::{
            unlock_condition::{
                AddressUnlockCondition, StorageDepositReturnUnlockCondition,
                TimelockUnlockCondition,
            },
            BasicOutput, BasicOutputBuilder, NativeToken, OutputId, TokenId,
        },
    };
    use iota_types::timelock::stardust_upgrade_label::STARDUST_UPGRADE_LABEL_VALUE;

    use crate::stardust::types::timelock::{self, VestedRewardError};

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
    fn is_timelocked_vested_reward_all_correct() {
        let output_id = OutputId::from_str(
            "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
        )
        .unwrap();
        let output = vested_reward_output(10, 1000);

        assert!(timelock::is_timelocked_vested_reward(
            output_id, &output, 100
        ));
    }

    #[test]
    fn is_timelocked_vested_reward_min_id() {
        let output_id = OutputId::from_str(
            "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18000000000000",
        )
        .unwrap();
        let output = vested_reward_output(10, 1000);

        assert!(timelock::is_timelocked_vested_reward(
            output_id, &output, 100
        ));
    }

    #[test]
    fn is_timelocked_vested_reward_max_id() {
        let output_id = OutputId::from_str(
            "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18ffffffff0000",
        )
        .unwrap();
        let output = vested_reward_output(10, 1000);

        assert!(timelock::is_timelocked_vested_reward(
            output_id, &output, 100
        ));
    }

    #[test]
    fn is_timelocked_vested_reward_incorrect_id() {
        let output_id = OutputId::from_str(
            "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb17123456780000",
        )
        .unwrap();
        let output = vested_reward_output(10, 1000);

        assert!(!timelock::is_timelocked_vested_reward(
            output_id, &output, 100
        ));
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

        assert!(!timelock::is_timelocked_vested_reward(
            output_id, &output, 100
        ));
    }

    #[test]
    fn is_timelocked_vested_reward_bigger_milestone_time() {
        let output_id = OutputId::from_str(
            "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
        )
        .unwrap();
        let output = vested_reward_output(10, 100);

        assert!(!timelock::is_timelocked_vested_reward(
            output_id, &output, 1000
        ));
    }

    #[test]
    fn is_timelocked_vested_reward_same_milestone_time() {
        let output_id = OutputId::from_str(
            "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
        )
        .unwrap();
        let output = vested_reward_output(10, 1000);

        assert!(!timelock::is_timelocked_vested_reward(
            output_id, &output, 1000
        ));
    }

    #[test]
    fn timelock_from_stardust_all_correct() {
        let output_id = OutputId::from_str(
            "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18123456780000",
        )
        .unwrap();
        let output = vested_reward_output(10, 1000);

        let timelock = timelock::try_from_stardust(output_id, &output, 100).unwrap();

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

        let err = timelock::try_from_stardust(output_id, &output, 1000).unwrap_err();

        assert!(matches!(err, VestedRewardError::UnlockedVestedReward));
    }

    #[test]
    fn timelock_from_stardust_with_incorrect_id() {
        let output_id = OutputId::from_str(
            "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb17123456780000",
        )
        .unwrap();
        let output = vested_reward_output(10, 1000);

        let err = timelock::try_from_stardust(output_id, &output, 100).unwrap_err();

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

        let err = timelock::try_from_stardust(output_id, &output, 1000).unwrap_err();

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

        let err = timelock::try_from_stardust(output_id, &output, 100).unwrap_err();

        assert!(matches!(
            err,
            VestedRewardError::UnlockConditionsNumberMismatch
        ));
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

        let err = timelock::try_from_stardust(output_id, &output, 100).unwrap_err();

        assert!(matches!(err, VestedRewardError::NativeTokensNotSupported));
    }
}
