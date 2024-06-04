// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::{anyhow, bail, ensure, Result};
use iota_sdk::{
    types::block::{
        address::Address,
        output::{self as sdk_output, NativeTokens, TokenId},
    },
    U256,
};
use sui_types::{
    balance::Balance,
    base_types::{ObjectID, SuiAddress},
    coin::Coin,
    collection_types::Bag,
    dynamic_field::Field,
    in_memory_storage::InMemoryStorage,
    object::Object,
    TypeTag,
};

use crate::stardust::{
    migration::executor::FoundryLedgerData,
    types::{output as migration_output, token_scheme::MAX_ALLOWED_U64_SUPPLY, Alias, Nft},
};

pub(super) fn verify_native_tokens<NtKind: NativeTokenKind>(
    native_tokens: &NativeTokens,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    native_tokens_bag: impl Into<Option<Bag>>,
    created_native_tokens: Option<&[ObjectID]>,
    storage: &InMemoryStorage,
) -> Result<()> {
    // Token types should be unique as the token ID is guaranteed unique within
    // NativeTokens
    let created_native_tokens = created_native_tokens
        .map(|object_ids| {
            object_ids
                .iter()
                .map(|id| {
                    let obj = storage
                        .get_object(id)
                        .ok_or_else(|| anyhow!("missing native token field for {id}"))?;
                    NtKind::from_object(obj).map(|nt| (nt.token_type(), nt.value()))
                })
                .collect::<Result<HashMap<String, u64>, _>>()
        })
        .unwrap_or(Ok(HashMap::new()))?;

    ensure!(
        created_native_tokens.len() == native_tokens.len(),
        "native token count mismatch: found {}, expected: {}",
        created_native_tokens.len(),
        native_tokens.len(),
    );

    if let Some(native_tokens_bag) = native_tokens_bag.into() {
        ensure!(
            native_tokens_bag.size == native_tokens.len() as u64,
            "native tokens bag length mismatch: found {}, expected {}",
            native_tokens_bag.size,
            native_tokens.len()
        );
    }

    for native_token in native_tokens.iter() {
        let foundry_data = foundry_data
            .get(native_token.token_id())
            .ok_or_else(|| anyhow!("missing foundry data for token {}", native_token.token_id()))?;

        let expected_token_type = foundry_data.canonical_coin_type();
        // The token amounts are scaled so that the total circulating supply does not
        // exceed `u64::MAX`
        let reduced_amount = foundry_data
            .token_scheme_u64
            .adjust_tokens(native_token.amount());

        if let Some(created_value) = created_native_tokens.get(&expected_token_type) {
            ensure!(
                *created_value == reduced_amount,
                "created token amount mismatch: found {created_value}, expected {reduced_amount}"
            );
        } else {
            bail!(
                "native token object was not created for token: {}",
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

pub(super) fn verify_issuer_feature(
    original: Option<&sdk_output::feature::IssuerFeature>,
    created: Option<SuiAddress>,
) -> Result<()> {
    if let Some(issuer) = original {
        let sui_issuer_address = issuer.address().to_string().parse::<SuiAddress>()?;
        if let Some(obj_issuer) = created {
            ensure!(
                obj_issuer == sui_issuer_address,
                "issuer mismatch: found {}, expected {}",
                obj_issuer,
                sui_issuer_address
            );
        } else {
            bail!("missing issuer on object");
        }
    } else {
        ensure!(created.is_none(), "erroneous issuer on object");
    }
    Ok(())
}

// Checks whether an object exists for this address and whether it is the
// expected alias or nft object. We do not expect an object for Ed25519
// addresses.
pub(super) fn verify_parent(address: &Address, storage: &InMemoryStorage) -> Result<()> {
    let object_id = ObjectID::from(address.to_string().parse::<SuiAddress>()?);
    let parent = storage.get_object(&object_id);
    match address {
        Address::Alias(address) => {
            if let Some(parent_obj) = parent {
                parent_obj
                    .to_rust::<Alias>()
                    .ok_or_else(|| anyhow!("invalid alias object for {address}"))?;
            }
        }
        Address::Nft(address) => {
            if let Some(parent_obj) = parent {
                parent_obj
                    .to_rust::<Nft>()
                    .ok_or_else(|| anyhow!("invalid nft object for {address}"))?;
            }
        }
        Address::Ed25519(address) => {
            ensure!(
                parent.is_none(),
                "unexpected parent found for ed25519 address {address}",
            );
        }
    }
    Ok(())
}

pub(super) trait NativeTokenKind {
    fn token_type(&self) -> String;

    fn value(&self) -> u64;

    fn from_object(obj: &Object) -> Result<Self>
    where
        Self: Sized;
}

impl NativeTokenKind for (TypeTag, Coin) {
    fn token_type(&self) -> String {
        self.0.to_canonical_string(false)
    }

    fn value(&self) -> u64 {
        self.1.value()
    }

    fn from_object(obj: &Object) -> Result<Self> {
        obj.coin_type_maybe()
            .zip(obj.as_coin_maybe())
            .ok_or_else(|| anyhow!("expected a native token coin, found {:?}", obj.type_()))
    }
}

impl NativeTokenKind for Field<String, Balance> {
    fn token_type(&self) -> String {
        self.name.clone()
    }

    fn value(&self) -> u64 {
        self.value.value()
    }

    fn from_object(obj: &Object) -> Result<Self> {
        obj.to_rust::<Field<String, Balance>>()
            .ok_or_else(|| anyhow!("expected a native token field, found {:?}", obj.type_()))
    }
}

pub fn truncate_to_max_allowed_u64_supply(value: U256) -> u64 {
    if value > U256::from(MAX_ALLOWED_U64_SUPPLY) {
        MAX_ALLOWED_U64_SUPPLY
    } else {
        value.as_u64()
    }
}
