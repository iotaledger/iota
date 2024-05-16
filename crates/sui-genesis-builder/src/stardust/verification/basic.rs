// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, bail, ensure};
use iota_sdk::types::block::output::BasicOutput;
use sui_types::{base_types::SuiAddress, object::Object};

pub fn verify_basic_output(object: &Object, output: &BasicOutput) -> anyhow::Result<()> {
    let object = object
        .to_rust::<crate::stardust::types::output::BasicOutput>()
        .ok_or_else(|| anyhow!("invalid basic output object"))?;

    // Amount
    ensure!(object.iota.value() == output.amount(), "amount mismatch");

    // Native Tokens
    ensure!(
        object.native_tokens.size == output.native_tokens().len() as u64,
        "native tokens length mismatch"
    );

    // Storage Deposit Return Unlock Condition
    if let Some(sdruc) = output.unlock_conditions().storage_deposit_return() {
        let sui_return_address = sdruc.return_address().to_string().parse::<SuiAddress>()?;
        if let Some(obj_sdruc) = object.storage_deposit_return {
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
            object.storage_deposit_return.is_none(),
            "erroneous storage deposit return on object"
        );
    }

    // Timelock Unlock Condition
    if let Some(timelock) = output.unlock_conditions().timelock() {
        if let Some(obj_timelock) = object.timelock {
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
        ensure!(object.timelock.is_none(), "erroneous timelock on object");
    }

    // Expiration Unlock Condition
    if let Some(expiration) = output.unlock_conditions().expiration() {
        if let Some(obj_expiration) = object.expiration {
            let sui_address = output.address().to_string().parse::<SuiAddress>()?;
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
        ensure!(
            object.expiration.is_none(),
            "erroneous expiration on object"
        );
    }

    // Metadata Feature
    if let Some(metadata) = output.features().metadata() {
        if let Some(obj_metadata) = object.metadata {
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
        ensure!(object.metadata.is_none(), "erroneous metadata on object");
    }

    // Tag Feature
    if let Some(tag) = output.features().tag() {
        if let Some(obj_tag) = object.tag {
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
        ensure!(object.tag.is_none(), "erroneous tag on object");
    }

    // Sender Feature
    if let Some(sender) = output.features().sender() {
        let sui_sender_address = sender.address().to_string().parse::<SuiAddress>()?;
        if let Some(obj_sender) = object.sender {
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
        ensure!(object.sender.is_none(), "erroneous sender on object");
    }
    Ok(())
}
