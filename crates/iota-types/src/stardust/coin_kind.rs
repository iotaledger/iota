// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Coin kinds introduced with the stardust models.
//!
//! We define as coin kinds objects that hold some kind of `Balance`
//! and comply with the `Coin` type layout:
//!
//! ```rust
//! struct T {
//!     id: iota_types::id::UID,
//!     balance: iota_types::balance::Balance,
//!     // ...
//! }
//! ```

use std::mem::size_of;

use crate::{
    balance::Balance,
    gas_coin::GAS,
    object::{ID_END_INDEX, Object},
    stardust::output::{AliasOutput, BasicOutput, NftOutput},
    timelock::timelock::TimeLock,
};

/// Infer whether the object is a kind of gas coin.
pub fn is_gas_coin_kind(object: &Object) -> bool {
    let Some(struct_tag) = object.struct_tag() else {
        return false;
    };
    struct_tag == AliasOutput::tag(GAS::type_tag())
        || struct_tag == BasicOutput::tag(GAS::type_tag())
        || struct_tag == NftOutput::tag(GAS::type_tag())
        || struct_tag == TimeLock::<Balance>::type_(Balance::type_(GAS::type_tag()).into())
        || object.is_gas_coin()
}

/// Return the `Balance` of a gas-coin kind.
///
/// Useful to avoid deserialization of the entire object.
pub fn get_gas_balance_maybe(object: &Object) -> Option<Balance> {
    if !is_gas_coin_kind(object) {
        return None;
    }
    let inner = object.data.try_as_move()?;
    bcs::from_bytes(&inner.contents()[ID_END_INDEX..][..size_of::<Balance>()]).ok()
}

#[cfg(test)]
mod tests {
    use iota_protocol_config::ProtocolConfig;

    use crate::{
        balance::Balance,
        base_types::{IotaAddress, ObjectID, TxContext},
        id::UID,
        object::{Object, Owner},
        stardust::{
            coin_kind::{get_gas_balance_maybe, is_gas_coin_kind},
            coin_type::CoinType,
            output::{AliasOutput, BasicOutput, NftOutput},
        },
        timelock::timelock::{TimeLock, to_genesis_object},
    };

    fn nft_output(balance: u64, coin_type: CoinType) -> anyhow::Result<Object> {
        let id = UID::new(ObjectID::random());
        let balance = Balance::new(balance);
        let output = NftOutput {
            id,
            balance,
            native_tokens: Default::default(),
            storage_deposit_return: Default::default(),
            timelock: Default::default(),
            expiration: Default::default(),
        };
        output.to_genesis_object(
            IotaAddress::ZERO,
            &ProtocolConfig::get_for_min_version(),
            &TxContext::random_for_testing_only(),
            1.into(),
            coin_type,
        )
    }

    #[test]
    fn is_coin_kind_nft_output() {
        let object = nft_output(100, CoinType::Iota).unwrap();
        assert!(is_gas_coin_kind(&object));
    }

    #[test]
    fn get_gas_balance_nft_output() {
        let value = 100;
        let object = nft_output(value, CoinType::Iota).unwrap();
        let gas_coin_balance = get_gas_balance_maybe(&object).unwrap();
        assert_eq!(gas_coin_balance.value(), value);
    }

    fn alias_output(balance: u64, coin_type: CoinType) -> anyhow::Result<Object> {
        let id = UID::new(ObjectID::random());
        let balance = Balance::new(balance);
        let output = AliasOutput {
            id,
            balance,
            native_tokens: Default::default(),
        };
        output.to_genesis_object(
            Owner::AddressOwner(IotaAddress::ZERO),
            &ProtocolConfig::get_for_min_version(),
            &TxContext::random_for_testing_only(),
            1.into(),
            coin_type,
        )
    }

    #[test]
    fn is_coin_kind_alias_output() {
        let object = alias_output(100, CoinType::Iota).unwrap();
        assert!(is_gas_coin_kind(&object));
    }

    #[test]
    fn get_gas_balance_alias_output() {
        let value = 100;
        let object = alias_output(value, CoinType::Iota).unwrap();
        let gas_coin_balance = get_gas_balance_maybe(&object).unwrap();
        assert_eq!(gas_coin_balance.value(), value);
    }

    fn basic_output(balance: u64, coin_type: CoinType) -> anyhow::Result<Object> {
        let id = UID::new(ObjectID::random());
        let balance = Balance::new(balance);
        let output = BasicOutput {
            id,
            balance,
            native_tokens: Default::default(),
            storage_deposit_return: Default::default(),
            timelock: Default::default(),
            expiration: Default::default(),
            metadata: Default::default(),
            tag: Default::default(),
            sender: Default::default(),
        };
        output.to_genesis_object(
            IotaAddress::ZERO,
            &ProtocolConfig::get_for_min_version(),
            &TxContext::random_for_testing_only(),
            1.into(),
            &coin_type,
        )
    }

    #[test]
    fn is_coin_kind_basic_output() {
        let object = basic_output(100, CoinType::Iota).unwrap();
        assert!(is_gas_coin_kind(&object));
    }

    #[test]
    fn get_gas_balance_basic_output() {
        let value = 100;
        let object = basic_output(value, CoinType::Iota).unwrap();
        let gas_coin_balance = get_gas_balance_maybe(&object).unwrap();
        assert_eq!(gas_coin_balance.value(), value);
    }

    fn timelock(balance: u64) -> anyhow::Result<Object> {
        let id = UID::new(ObjectID::random());
        let balance = Balance::new(balance);
        let expiration_timestamp_ms = 10;
        let label = None;

        let timelock = TimeLock::new(id, balance, expiration_timestamp_ms, label);
        Ok(to_genesis_object(
            timelock,
            IotaAddress::ZERO,
            &ProtocolConfig::get_for_min_version(),
            &TxContext::random_for_testing_only(),
            1.into(),
        )?)
    }

    #[test]
    fn is_coin_kind_timelock() {
        let object = timelock(100).unwrap();
        assert!(is_gas_coin_kind(&object));
    }

    #[test]
    fn get_gas_balance_timelock() {
        let value = 100;
        let object = timelock(value).unwrap();
        let gas_coin_balance = get_gas_balance_maybe(&object).unwrap();
        assert_eq!(gas_coin_balance.value(), value);
    }
}
