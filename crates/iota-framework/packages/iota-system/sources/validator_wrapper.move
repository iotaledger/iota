// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::validator_wrapper {
    use iota::versioned::Versioned;
    use iota_system::validator::{ValidatorV1, ValidatorV2};
    use iota::versioned;

    const EInvalidVersion: u64 = 0;

    public struct Validator has store {
        inner: Versioned
    }

    // ValidatorV1 corresponds to version 1.
    public(package) fun create_v1(validator: ValidatorV1, ctx: &mut TxContext): Validator {
        Validator {
            inner: versioned::create(1, validator, ctx)
        }
    }

    // ValidatorV2 corresponds to version 2.
    public(package) fun create_v2(validator: ValidatorV2, ctx: &mut TxContext): Validator {
        Validator {
            inner: versioned::create(2, validator, ctx)
        }
    }

    /// This function should always return the latest supported version.
    /// If the inner version is old, we upgrade it lazily in-place.
    public(package) fun load_validator_maybe_upgrade(self: &mut Validator): &mut ValidatorV2 {
        upgrade_to_latest(self);
        versioned::load_value_mut(&mut self.inner)
    }

    /// Destroy the wrapper and retrieve the inner validator object.
    public(package) fun destroy(mut self: Validator): ValidatorV2 {
        upgrade_to_latest(&mut self);
        let Validator { inner } = self;
        versioned::destroy(inner)
    }

    #[test_only]
    /// Load the inner validator with assumed type. This should be used for testing only.
    public(package) fun get_inner_validator_ref(self: &Validator): &ValidatorV2 {
        versioned::load_value(&self.inner)
    }

    fun upgrade_to_latest(self: &mut Validator) {
        // When new versions are added, we need to explicitly upgrade here.
        let version = version(self);
        if (version == 1) {
            let (v1, cap)
                = versioned::remove_value_for_upgrade<ValidatorV1>(&mut self.inner);
            versioned::upgrade(&mut self.inner, 2, v1.v1_to_v2(), cap);
        };
        assert!(version == 2, EInvalidVersion);
    }

    fun version(self: &Validator): u64 {
        versioned::version(&self.inner)
    }
}
