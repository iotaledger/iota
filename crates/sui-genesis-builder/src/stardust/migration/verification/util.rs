// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, ensure, Result};
use iota_sdk::{
    types::block::{
        address::Address,
        output::{self as sdk_output, NativeTokens},
    },
    U256,
};
use sui_types::{balance::Balance, base_types::SuiAddress, coin::Coin, dynamic_field::Field};

use crate::stardust::types::output as migration_output;

pub(super) fn verify_native_tokens(
    native_tokens: &NativeTokens,
    mut created_native_tokens: Vec<impl NativeTokenKind>,
) -> Result<()> {
    ensure!(
        native_tokens.len() == created_native_tokens.len(),
        "native token count mismatch: found {}, expected: {}",
        native_tokens.len(),
        created_native_tokens.len(),
    );

    let token_max = U256::from(u64::MAX);
    for native_token in native_tokens.iter() {
        // The token amounts are capped at u64 max
        let reduced_amount = native_token.amount().min(token_max).as_u64();
        if let Some(idx) = created_native_tokens
            .iter()
            .position(|coin| coin.value() == reduced_amount)
        {
            // Remove the coin so we don't find it again.
            created_native_tokens.remove(idx);
        } else {
            bail!(
                "native token coin was not created for token: {}",
                native_token.token_id()
            );
        }
    }

    Ok(())
}

pub(super) fn verify_storage_deposit_unlock_condition(
    original: Option<&sdk_output::unlock_condition::StorageDepositReturnUnlockCondition>,
    created: Option<&migration_output::StorageDepositReturnUnlockCondition>,
) -> Result<()> {
    // Storage Deposit Return Unlock Condition
    if let Some(sdruc) = original {
        let sui_return_address = sdruc.return_address().to_string().parse::<SuiAddress>()?;
        if let Some(obj_sdruc) = created {
            ensure!(
                obj_sdruc.return_address == sui_return_address,
                "storage deposit return address mismatch: found {}, expected {}",
                obj_sdruc.return_address,
                sui_return_address
            );
            ensure!(
                obj_sdruc.return_amount == sdruc.amount(),
                "storage deposit return amount mismatch: found {}, expected {}",
                obj_sdruc.return_amount,
                sdruc.amount()
            );
        } else {
            bail!("missing storage deposit return on object");
        }
    } else {
        ensure!(
            created.is_none(),
            "erroneous storage deposit return on object"
        );
    }
    Ok(())
}

pub(super) fn verify_timelock_unlock_condition(
    original: Option<&sdk_output::unlock_condition::TimelockUnlockCondition>,
    created: Option<&migration_output::TimelockUnlockCondition>,
) -> Result<()> {
    // Timelock Unlock Condition
    if let Some(timelock) = original {
        if let Some(obj_timelock) = created {
            ensure!(
                obj_timelock.unix_time == timelock.timestamp(),
                "timelock timestamp mismatch: found {}, expected {}",
                obj_timelock.unix_time,
                timelock.timestamp()
            );
        } else {
            bail!("missing timelock on object");
        }
    } else {
        ensure!(created.is_none(), "erroneous timelock on object");
    }
    Ok(())
}

pub(super) fn verify_expiration_unlock_condition(
    original: Option<&sdk_output::unlock_condition::ExpirationUnlockCondition>,
    created: Option<&migration_output::ExpirationUnlockCondition>,
    address: &Address,
) -> Result<()> {
    // Expiration Unlock Condition
    if let Some(expiration) = original {
        if let Some(obj_expiration) = created {
            let sui_address = address.to_string().parse::<SuiAddress>()?;
            let sui_return_address = expiration
                .return_address()
                .to_string()
                .parse::<SuiAddress>()?;
            ensure!(
                obj_expiration.owner == sui_address,
                "expiration owner mismatch: found {}, expected {}",
                obj_expiration.owner,
                sui_address
            );
            ensure!(
                obj_expiration.return_address == sui_return_address,
                "expiration return address mismatch: found {}, expected {}",
                obj_expiration.return_address,
                sui_return_address
            );
            ensure!(
                obj_expiration.unix_time == expiration.timestamp(),
                "expiration timestamp mismatch: found {}, expected {}",
                obj_expiration.unix_time,
                expiration.timestamp()
            );
        } else {
            bail!("missing expiration on object");
        }
    } else {
        ensure!(created.is_none(), "erroneous expiration on object");
    }
    Ok(())
}

pub(super) fn verify_metadata_feature(
    original: Option<&sdk_output::feature::MetadataFeature>,
    created: Option<&Vec<u8>>,
) -> Result<()> {
    if let Some(metadata) = original {
        if let Some(obj_metadata) = created {
            ensure!(
                obj_metadata.as_slice() == metadata.data(),
                "metadata mismatch: found {:x?}, expected {:x?}",
                obj_metadata.as_slice(),
                metadata.data()
            );
        } else {
            bail!("missing metadata on object");
        }
    } else {
        ensure!(created.is_none(), "erroneous metadata on object");
    }
    Ok(())
}

pub(super) fn verify_tag_feature(
    original: Option<&sdk_output::feature::TagFeature>,
    created: Option<&Vec<u8>>,
) -> Result<()> {
    if let Some(tag) = original {
        if let Some(obj_tag) = created {
            ensure!(
                obj_tag.as_slice() == tag.tag(),
                "tag mismatch: found {:x?}, expected {:x?}",
                obj_tag.as_slice(),
                tag.tag()
            );
        } else {
            bail!("missing tag on object");
        }
    } else {
        ensure!(created.is_none(), "erroneous tag on object");
    }
    Ok(())
}

pub(super) fn verify_sender_feature(
    original: Option<&sdk_output::feature::SenderFeature>,
    created: Option<SuiAddress>,
) -> Result<()> {
    if let Some(sender) = original {
        let sui_sender_address = sender.address().to_string().parse::<SuiAddress>()?;
        if let Some(obj_sender) = created {
            ensure!(
                obj_sender == sui_sender_address,
                "sender mismatch: found {}, expected {}",
                obj_sender,
                sui_sender_address
            );
        } else {
            bail!("missing sender on object");
        }
    } else {
        ensure!(created.is_none(), "erroneous sender on object");
    }
    Ok(())
}

pub(super) trait NativeTokenKind {
    fn value(&self) -> u64;
}

impl NativeTokenKind for Coin {
    fn value(&self) -> u64 {
        self.value()
    }
}

impl<K> NativeTokenKind for Field<K, Balance> {
    fn value(&self) -> u64 {
        self.value.value()
    }
}
