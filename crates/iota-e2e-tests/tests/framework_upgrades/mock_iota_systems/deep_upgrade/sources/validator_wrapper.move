// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::validator_wrapper {
    use iota::versioned::{Self, Versioned};
    use iota_system::validator::{Self, ValidatorV1, ValidatorV2};

    const VALIDATOR_VERSION_V1: u64 = 18446744073709551605;  // u64::MAX - 10
    const VALIDATOR_VERSION_V3: u64 = 18446744073709551607;  // u64::MAX - 8

    const EInvalidVersion: u64 = 0;

    public struct Validator has store {
        inner: Versioned
    }

    // Validator corresponds to version 1.
    public(package) fun create_v1(validator: ValidatorV1, ctx: &mut TxContext): Validator {
        Validator {
            inner: versioned::create(VALIDATOR_VERSION_V1, validator, ctx)
        }
    }

    /// This function should always return the latest supported version.
    /// If the inner version is old, we upgrade it lazily in-place.
    public(package) fun load_validator_maybe_upgrade(self: &mut Validator): &mut ValidatorV2 {
        upgrade_to_latest(self);
        versioned::load_value_mut<ValidatorV2>(&mut self.inner)
    }

    /// Destroy the wrapper and retrieve the inner validator object.
    public(package) fun destroy(mut self: Validator): ValidatorV2 {
        upgrade_to_latest(&mut self);
        let Validator { inner } = self;
        versioned::destroy(inner)
    }

    fun upgrade_to_latest(self: &mut Validator) {
        let version = version(self);
        if (version == VALIDATOR_VERSION_V1) {
            let (v1, cap) = versioned::remove_value_for_upgrade(&mut self.inner);
            let v3 = validator::v1_to_v2(v1);
            versioned::upgrade(&mut self.inner, VALIDATOR_VERSION_V3, v3, cap);
        };
        assert!(version(self) == VALIDATOR_VERSION_V3, EInvalidVersion);
    }

    fun version(self: &Validator): u64 {
        versioned::version(&self.inner)
    }
}
