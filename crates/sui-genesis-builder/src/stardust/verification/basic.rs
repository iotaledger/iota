// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, bail, ensure};
use iota_sdk::types::block::output::BasicOutput;
use sui_types::{base_types::SuiAddress, in_memory_storage::InMemoryStorage};

use crate::stardust::migration::CreatedObjects;

pub fn verify_basic_output(
    output: &BasicOutput,
    created_objects: &CreatedObjects,
    storage: &InMemoryStorage,
) -> anyhow::Result<()> {
    // If the output contains only an address unlock condition then no object should be created, only a coin.
    if output.unlock_conditions().len() > 1 {
        let created_output = created_objects
            .output()
            .and_then(|id| {
                storage
                    .get_object(id)
                    .ok_or_else(|| anyhow!("missing object"))
            })?
            .to_rust::<crate::stardust::types::output::BasicOutput>()
            .ok_or_else(|| anyhow!("invalid basic output object"))?;

        // Amount
        ensure!(
            created_output.iota.value() == output.amount(),
            "amount mismatch: found {}, expected {}",
            created_output.iota.value(),
            output.amount()
        );

        // Native Tokens
        ensure!(
            created_output.native_tokens.size == output.native_tokens().len() as u64,
            "native tokens length mismatch: found {}, expected {}",
            created_output.native_tokens.size,
            output.native_tokens().len()
        );

        // Storage Deposit Return Unlock Condition
        if let Some(sdruc) = output.unlock_conditions().storage_deposit_return() {
            let sui_return_address = sdruc.return_address().to_string().parse::<SuiAddress>()?;
            if let Some(obj_sdruc) = created_output.storage_deposit_return {
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
                created_output.storage_deposit_return.is_none(),
                "erroneous storage deposit return on object"
            );
        }

        // Timelock Unlock Condition
        if let Some(timelock) = output.unlock_conditions().timelock() {
            if let Some(obj_timelock) = created_output.timelock {
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
            ensure!(
                created_output.timelock.is_none(),
                "erroneous timelock on object"
            );
        }

        // Expiration Unlock Condition
        if let Some(expiration) = output.unlock_conditions().expiration() {
            if let Some(obj_expiration) = created_output.expiration {
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
                created_output.expiration.is_none(),
                "erroneous expiration on object"
            );
        }

        // Metadata Feature
        if let Some(metadata) = output.features().metadata() {
            if let Some(obj_metadata) = created_output.metadata {
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
            ensure!(
                created_output.metadata.is_none(),
                "erroneous metadata on object"
            );
        }

        // Tag Feature
        if let Some(tag) = output.features().tag() {
            if let Some(obj_tag) = created_output.tag {
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
            ensure!(created_output.tag.is_none(), "erroneous tag on object");
        }

        // Sender Feature
        if let Some(sender) = output.features().sender() {
            let sui_sender_address = sender.address().to_string().parse::<SuiAddress>()?;
            if let Some(obj_sender) = created_output.sender {
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
            ensure!(
                created_output.sender.is_none(),
                "erroneous sender on object"
            );
        }
    } else {
        ensure!(
            created_objects.output().is_err(),
            "unexpected output object created for simple deposit"
        );

        // Validate coin value.
        let created_coin = created_objects
            .coin()
            .and_then(|id| {
                storage
                    .get_object(id)
                    .ok_or_else(|| anyhow!("missing coin"))
            })?
            .as_coin_maybe()
            .ok_or_else(|| anyhow::anyhow!("expected a coin"))?;
        ensure!(
            created_coin.value() == output.amount(),
            "coin amount mismatch: found {}, expected {}",
            created_coin.value(),
            output.amount()
        );
    }

    ensure!(
        created_objects.package().is_err(),
        "unexpected package found"
    );

    Ok(())
}
