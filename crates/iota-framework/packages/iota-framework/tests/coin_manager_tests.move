// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::coin_manager_tests {

    use std::ascii;
    use std::string;

    use iota::coin_manager;
    use iota::coin::{Self, CoinMetadata};
    use iota::test_scenario;
    use iota::test_utils::assert_eq;
    use iota::url::{Self, Url};

    public struct COIN_MANAGER_TESTS has drop {}
    
    public struct BonusMetadata has store {
        website: Url,
        is_amazing: bool
    }

    #[test]
    fun test_coin_manager() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());
        
        assert!(wrapper.decimals() == 0);

        // We should start out with a Supply of 0.
        assert!(wrapper.total_supply() == 0);
        
        // Mint some coin!
        cmcap.mint_and_transfer(&mut wrapper, 10, sender, scenario.ctx());
        
        // We should now have a Supply of 10.
        assert!(wrapper.total_supply() == 10);
        
        // No maximum supply set, so we can do this again!
        cmcap.mint_and_transfer(&mut wrapper, 10, sender, scenario.ctx());

        transfer::public_transfer(cmcap, scenario.ctx().sender());
        metacap.renounce_metadata_ownership(&mut wrapper);
        transfer::public_share_object(wrapper);

        scenario.end();
    }
    
    #[test]
    fun test_coin_manager_helper() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cmcap, metacap, mut wrapper) = coin_manager::create(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        assert!(wrapper.decimals() == 0);

        // We should start out with a Supply of 0.
        assert!(wrapper.total_supply() == 0);
        
        // Mint some coin!
        cmcap.mint_and_transfer(&mut wrapper, 10, sender, scenario.ctx());
        
        // We should now have a Supply of 10.
        assert!(wrapper.total_supply() == 10);
        
        // No maximum supply set, so we can do this again!
        cmcap.mint_and_transfer(&mut wrapper, 10, sender, scenario.ctx());

        transfer::public_transfer(cmcap, scenario.ctx().sender());
        metacap.renounce_metadata_ownership(&mut wrapper);
        transfer::public_share_object(wrapper);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = coin_manager::EMaximumSupplyReached)]
    fun test_max_supply_higher_that_total() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());
        
        // We should start out with a Supply of 0.
        assert!(wrapper.total_supply() == 0);

        // Enforce a Max Supply.
        cmcap.enforce_maximum_supply(&mut wrapper, 10);
        
        // Mint some coin!
        cmcap.mint_and_transfer(&mut wrapper, 10, sender, scenario.ctx());
        
        // We should now have a Supply of 10.
        assert!(wrapper.total_supply() == 10);
        
        // This should fail.
        cmcap.mint_and_transfer(&mut wrapper, 1, sender, scenario.ctx());

        transfer::public_transfer(cmcap, scenario.ctx().sender());
        metacap.renounce_metadata_ownership(&mut wrapper);

        transfer::public_share_object(wrapper);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = coin_manager::EMaximumSupplyReached)]
    fun test_max_supply_equals_total() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());
        
        // We should start out with a Supply of 0.
        assert!(wrapper.total_supply() == 0);
        
        // Mint some coin!
        cmcap.mint_and_transfer(&mut wrapper, 10, sender, scenario.ctx());

        // We should now have a Supply of 10.
        assert!(wrapper.total_supply() == 10);

        // Enforce a Max Supply.
        cmcap.enforce_maximum_supply(&mut wrapper, 10);
        
        // This should fail.
        cmcap.mint_and_transfer(&mut wrapper, 1, sender, scenario.ctx());

        transfer::public_transfer(cmcap, scenario.ctx().sender());
        metacap.renounce_metadata_ownership(&mut wrapper);

        transfer::public_share_object(wrapper);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = coin_manager::EMaximumSupplyLowerThanTotalSupply)]
    fun test_max_supply_lower_than_total() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());

        // We should start out with a Supply of 0.
        assert!(wrapper.total_supply() == 0);

        // Mint some coin!
        cmcap.mint_and_transfer(&mut wrapper, 10, sender, scenario.ctx());
        
        // We should now have a Supply of 10.
        assert!(wrapper.total_supply() == 10);

        // Update the maximum supply to be lower than the total supply, this should not be allowed.
        cmcap.enforce_maximum_supply(&mut wrapper, 9);
        
        transfer::public_transfer(cmcap, scenario.ctx().sender());
        metacap.renounce_metadata_ownership(&mut wrapper);

        transfer::public_share_object(wrapper);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = coin_manager::EMaximumSupplyHigherThanPossible)]
    fun test_max_supply_higher_than_maximum() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());

        // Check the default maximum supply.
        assert_eq(wrapper.maximum_supply(), 18_446_744_073_709_551_614u64);

        // Update the maximum supply to be higher than is maximum possible.
        cmcap.enforce_maximum_supply(&mut wrapper, 18_446_744_073_709_551_615u64);
        
        transfer::public_transfer(cmcap, scenario.ctx().sender());
        metacap.renounce_metadata_ownership(&mut wrapper);

        transfer::public_share_object(wrapper);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = coin_manager::EMaximumSupplyAlreadySet)]
    fun test_max_supply_once() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());
        
        // Enforce a Max Supply.
        cmcap.enforce_maximum_supply(&mut wrapper, 10);

        // Update it, this should not be allowed.
        cmcap.enforce_maximum_supply(&mut wrapper, 20);
        
        transfer::public_transfer(cmcap, scenario.ctx().sender());
        metacap.renounce_metadata_ownership(&mut wrapper);

        transfer::public_share_object(wrapper);

        scenario.end();
    }
    
    #[test]
    fun test_renounce_ownership() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());
        
        // We should start out with a Supply of 0.
        assert!(wrapper.total_supply() == 0);

        // Enforce a Max Supply.
        cmcap.enforce_maximum_supply(&mut wrapper, 10);
        
        // Mint some coin!
        cmcap.mint_and_transfer(&mut wrapper, 5, sender, scenario.ctx());
        
        // We should now have a Supply of 5.
        assert!(wrapper.total_supply() == 5);
        
        // We should now have a Max Supply of 10.
        assert!(wrapper.maximum_supply() == 10);

        // The coin is not immutable right now, we still have a `CoinManagerCap`.
        assert!(!wrapper.supply_is_immutable());
        assert!(!wrapper.metadata_is_immutable());
        
        // Lets turn it immutable!
        cmcap.renounce_treasury_ownership(&mut wrapper);

        // The coin should be immutable right now.
        assert!(wrapper.supply_is_immutable());
        // But metadata should still be mutable.
        assert!(!wrapper.metadata_is_immutable());
        
        // We should now have a Max Supply of 5, due to renouncing of ownership.
        assert!(wrapper.maximum_supply() == 5);

        metacap.renounce_metadata_ownership(&mut wrapper);
        assert!(wrapper.metadata_is_immutable());

        transfer::public_share_object(wrapper);
        scenario.end();
    }

    #[allow(deprecated_usage)]
    #[test]
    fun test_additional_metadata() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());

        let bonus = BonusMetadata {
            website: url::new_unsafe(ascii::string(b"https://example.com")),
            is_amazing: false
        };

        metacap.add_additional_metadata(&mut wrapper, bonus);

        assert!(!wrapper.additional_metadata<COIN_MANAGER_TESTS, BonusMetadata>().is_amazing);
        
        let bonus2 = BonusMetadata {
            website: url::new_unsafe(ascii::string(b"https://iota.org")),
            is_amazing: true
        };

        let oldmeta = metacap.replace_additional_metadata<COIN_MANAGER_TESTS, BonusMetadata, BonusMetadata>(&mut wrapper, bonus2);

        let BonusMetadata { website: _, is_amazing: _ } = oldmeta;
        
        assert!(wrapper.additional_metadata<COIN_MANAGER_TESTS, BonusMetadata>().is_amazing);
        
        cmcap.renounce_treasury_ownership(&mut wrapper);
        metacap.renounce_metadata_ownership(&mut wrapper);
        transfer::public_share_object(wrapper);

        scenario.end();
    }

    #[test]
    fun test_get_additional_metadata() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());

        let bonus = BonusMetadata {
            website: url::new_unsafe(ascii::string(b"https://example.com")),
            is_amazing: false
        };

        metacap.add_additional_metadata(&mut wrapper, bonus);

        assert!(!wrapper.get_additional_metadata<COIN_MANAGER_TESTS, BonusMetadata>().is_amazing);
        
        let bonus2 = BonusMetadata {
            website: url::new_unsafe(ascii::string(b"https://iota.org")),
            is_amazing: true
        };

        let oldmeta = metacap.replace_additional_metadata<COIN_MANAGER_TESTS, BonusMetadata, BonusMetadata>(&mut wrapper, bonus2);

        let BonusMetadata { website: _, is_amazing: _ } = oldmeta;
        
        assert!(wrapper.get_additional_metadata<COIN_MANAGER_TESTS, BonusMetadata>().is_amazing);
        
        cmcap.renounce_treasury_ownership(&mut wrapper);
        metacap.renounce_metadata_ownership(&mut wrapper);
        transfer::public_share_object(wrapper);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = iota::dynamic_field::EFieldAlreadyExists)]
    fun test_double_adding_additional_metadata() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());

        // Add an additional metadata.
        let bonus1 = BonusMetadata {
            website: url::new_unsafe(ascii::string(b"https://example1.com")),
            is_amazing: false
        };

        metacap.add_additional_metadata(&mut wrapper, bonus1);

        // Add an additional metadata one more time.
        let bonus2 = BonusMetadata {
            website: url::new_unsafe(ascii::string(b"https://example2.com")),
            is_amazing: false
        };

        metacap.add_additional_metadata(&mut wrapper, bonus2);

        cmcap.renounce_treasury_ownership(&mut wrapper);
        metacap.renounce_metadata_ownership(&mut wrapper);
        transfer::public_share_object(wrapper);

        scenario.end();
    }
    
    #[test]
    fun test_coin_manager_immutable() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"TEST",
            b"TEST",
            b"TEST",
            option::none(),
            scenario.ctx(),
        );

        transfer::public_freeze_object(meta);
        test_scenario::next_tx(&mut scenario, sender);

        let immeta = test_scenario::take_immutable<CoinMetadata<COIN_MANAGER_TESTS>>(&scenario);
        let (cmcap, mut wrapper) = coin_manager::new_with_immutable_metadata(cap, &immeta, scenario.ctx());
        
        assert!(wrapper.metadata_is_immutable());
        
        assert!(wrapper.decimals() == 0);

        // We should start out with a Supply of 0.
        assert!(wrapper.total_supply() == 0);
        
        // Mint some coin!
        cmcap.mint_and_transfer(&mut wrapper, 10, sender, scenario.ctx());

        transfer::public_transfer(cmcap, scenario.ctx().sender());
        transfer::public_share_object(wrapper);
        test_scenario::return_immutable(immeta);

        scenario.end();
    }

    #[test]
    fun test_coin_manager_update_metadata() {
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);
        let witness = COIN_MANAGER_TESTS{};

        // Create a `Coin`.
        let (cap, meta) = coin::create_currency(
            witness,
            0, 
            b"SYMBOL1",
            b"NAME1",
            b"DESCRIPTION1",
            option::some(url::new_unsafe(ascii::string(b"https://url1.com"))),
            scenario.ctx(),
        );

        let (cmcap, metacap, mut wrapper) = coin_manager::new(cap, meta, scenario.ctx());

        // Check the original metadata.
        assert_eq(wrapper.name(), string::utf8(b"NAME1"));
        assert_eq(wrapper.symbol(), ascii::string(b"SYMBOL1"));
        assert_eq(wrapper.description(), string::utf8(b"DESCRIPTION1"));
        assert_eq(wrapper.icon_url(), option::some(url::new_unsafe(ascii::string(b"https://url1.com"))));

        // Update the metadata.
        coin_manager::update_name(&metacap, &mut wrapper, string::utf8(b"NAME2"));
        coin_manager::update_symbol(&metacap, &mut wrapper, ascii::string(b"SYMBOL2"));
        coin_manager::update_description(&metacap, &mut wrapper, string::utf8(b"DESCRIPTION2"));
        coin_manager::update_icon_url(&metacap, &mut wrapper, ascii::string(b"https://url2.com"));

        // Check the metadata again.
        assert_eq(wrapper.name(), string::utf8(b"NAME2"));
        assert_eq(wrapper.symbol(), ascii::string(b"SYMBOL2"));
        assert_eq(wrapper.description(), string::utf8(b"DESCRIPTION2"));
        assert_eq(wrapper.icon_url(), option::some(url::new_unsafe(ascii::string(b"https://url2.com"))));

        transfer::public_transfer(cmcap, scenario.ctx().sender());
        metacap.renounce_metadata_ownership(&mut wrapper);
        transfer::public_share_object(wrapper);

        scenario.end();
    }
}
