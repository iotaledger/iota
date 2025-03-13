// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::{Result, anyhow, bail, ensure};
use iota_sdk::{
    U256,
    types::block::{
        address::Address,
        output::{self as sdk_output, NativeTokens, OutputId, TokenId},
    },
};
use iota_types::{
    TypeTag,
    balance::Balance,
    base_types::{IotaAddress, ObjectID},
    coin::Coin,
    collection_types::Bag,
    dynamic_field::Field,
    in_memory_storage::InMemoryStorage,
    object::{Object, Owner},
    stardust::{
        output::{Alias, Nft, unlock_conditions},
        stardust_to_iota_address,
    },
};
use tracing::warn;

use crate::stardust::{
    migration::executor::FoundryLedgerData,
    types::{address_swap_map::AddressSwapMap, token_scheme::MAX_ALLOWED_U64_SUPPLY},
};

pub const BASE_TOKEN_KEY: &str = "base_token";

/// Counter used to count the generated tokens amounts.
pub(super) struct TokensAmountCounter {
    // A map of token type -> (real_generated_supply, expected_circulating_supply)
    inner: HashMap<String, (u64, u64)>,
}

impl TokensAmountCounter {
    /// Setup the tokens amount counter.
    pub(super) fn new(initial_iota_supply: u64) -> Self {
        let mut res = TokensAmountCounter {
            inner: HashMap::new(),
        };
        res.update_total_value_max_for_iota(initial_iota_supply);
        res
    }

    pub(super) fn into_inner(self) -> impl IntoIterator<Item = (String, (u64, u64))> {
        self.inner
    }

    pub(super) fn update_total_value_for_iota(&mut self, value: u64) {
        self.update_total_value(BASE_TOKEN_KEY, value);
    }

    fn update_total_value_max_for_iota(&mut self, max: u64) {
        self.update_total_value_max(BASE_TOKEN_KEY, max);
    }

    pub(super) fn update_total_value(&mut self, key: &str, value: u64) {
        self.inner
            .entry(key.to_string())
            .and_modify(|v| v.0 += value)
            .or_insert((value, 0));
    }

    pub(super) fn update_total_value_max(&mut self, key: &str, max: u64) {
        self.inner
            .entry(key.to_string())
            .and_modify(|v| v.1 = max)
            .or_insert((0, max));
    }
}

pub(super) fn verify_native_tokens<NtKind: NativeTokenKind>(
    native_tokens: &NativeTokens,
    foundry_data: &HashMap<TokenId, FoundryLedgerData>,
    native_tokens_bag: impl Into<Option<Bag>>,
    created_native_tokens: Option<&[ObjectID]>,
    storage: &InMemoryStorage,
    tokens_counter: &mut TokensAmountCounter,
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
                    NtKind::from_object(obj).map(|nt| (nt.bag_key(), nt.value()))
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

        let expected_bag_key = foundry_data.to_canonical_string(/* with_prefix */ false);
        // The token amounts are scaled so that the total circulating supply does not
        // exceed `u64::MAX`
        let reduced_amount = foundry_data
            .token_scheme_u64
            .adjust_tokens(native_token.amount());

        if let Some(&created_value) = created_native_tokens.get(&expected_bag_key) {
            ensure!(
                created_value == reduced_amount,
                "created token amount mismatch: found {created_value}, expected {reduced_amount}"
            );
            tokens_counter.update_total_value(&expected_bag_key, created_value);
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
    created: Option<&unlock_conditions::StorageDepositReturnUnlockCondition>,
) -> Result<()> {
    // Storage Deposit Return Unlock Condition
    if let Some(sdruc) = original {
        let iota_return_address = stardust_to_iota_address(sdruc.return_address())?;
        if let Some(obj_sdruc) = created {
            ensure!(
                obj_sdruc.return_address == iota_return_address,
                "storage deposit return address mismatch: found {}, expected {}",
                obj_sdruc.return_address,
                iota_return_address
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
    created: Option<&unlock_conditions::TimelockUnlockCondition>,
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
    created: Option<&unlock_conditions::ExpirationUnlockCondition>,
    address: &Address,
) -> Result<()> {
    // Expiration Unlock Condition
    if let Some(expiration) = original {
        if let Some(obj_expiration) = created {
            let iota_address = stardust_to_iota_address(address)?;
            let iota_return_address = stardust_to_iota_address(expiration.return_address())?;
            ensure!(
                obj_expiration.owner == iota_address,
                "expiration owner mismatch: found {}, expected {}",
                obj_expiration.owner,
                iota_address
            );
            ensure!(
                obj_expiration.return_address == iota_return_address,
                "expiration return address mismatch: found {}, expected {}",
                obj_expiration.return_address,
                iota_return_address
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
    created: Option<IotaAddress>,
) -> Result<()> {
    if let Some(sender) = original {
        let iota_sender_address = stardust_to_iota_address(sender.address())?;
        if let Some(obj_sender) = created {
            ensure!(
                obj_sender == iota_sender_address,
                "sender mismatch: found {}, expected {}",
                obj_sender,
                iota_sender_address
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
    created: Option<IotaAddress>,
) -> Result<()> {
    if let Some(issuer) = original {
        let iota_issuer_address = stardust_to_iota_address(issuer.address())?;
        if let Some(obj_issuer) = created {
            ensure!(
                obj_issuer == iota_issuer_address,
                "issuer mismatch: found {}, expected {}",
                obj_issuer,
                iota_issuer_address
            );
        } else {
            bail!("missing issuer on object");
        }
    } else {
        ensure!(created.is_none(), "erroneous issuer on object");
    }
    Ok(())
}

pub(super) fn verify_address_owner(
    owning_address: &Address,
    obj: &Object,
    name: &str,
    address_swap_map: &AddressSwapMap,
) -> Result<()> {
    let expected_owner = address_swap_map.stardust_to_iota_address_owner(owning_address)?;

    ensure!(
        obj.owner == expected_owner,
        "{name} owner mismatch: found {}, expected {}",
        obj.owner,
        expected_owner
    );
    Ok(())
}

pub(super) fn verify_shared_object(obj: &Object, name: &str) -> Result<()> {
    let expected_owner = Owner::Shared {
        initial_shared_version: Default::default(),
    };
    ensure!(
        obj.owner.is_shared(),
        "{name} shared owner mismatch: found {}, expected {}",
        obj.owner,
        expected_owner
    );
    Ok(())
}

// Checks whether an object exists for this address and whether it is the
// expected alias or nft object. We do not expect an object for Ed25519
// addresses.
pub(super) fn verify_parent(
    output_id: &OutputId,
    address: &Address,
    storage: &InMemoryStorage,
) -> Result<()> {
    let object_id = ObjectID::from(stardust_to_iota_address(address)?);
    let parent = storage.get_object(&object_id);
    match address {
        Address::Alias(address) => {
            if let Some(parent_obj) = parent {
                if parent_obj.to_rust::<Alias>().is_none() {
                    warn!(
                        "verification failed for output id {output_id}: unexpected parent found for alias address {address}"
                    );
                }
            }
        }
        Address::Nft(address) => {
            if let Some(parent_obj) = parent {
                if parent_obj.to_rust::<Nft>().is_none() {
                    warn!(
                        "verification failed for output id {output_id}: unexpected parent found for nft address {address}"
                    );
                }
            }
        }
        Address::Ed25519(address) => {
            if parent.is_some() {
                warn!(
                    "verification failed for output id {output_id}: unexpected parent found for ed25519 address {address}"
                );
            }
        }
    }
    Ok(())
}

pub(super) fn verify_coin(output_amount: u64, created_coin: &Coin) -> Result<()> {
    ensure!(
        created_coin.value() == output_amount,
        "coin amount mismatch: found {}, expected {}",
        created_coin.value(),
        output_amount
    );
    Ok(())
}

pub(super) trait NativeTokenKind {
    fn bag_key(&self) -> String;

    fn value(&self) -> u64;

    fn from_object(obj: &Object) -> Result<Self>
    where
        Self: Sized;
}

impl NativeTokenKind for (TypeTag, Coin) {
    fn bag_key(&self) -> String {
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
    fn bag_key(&self) -> String {
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
