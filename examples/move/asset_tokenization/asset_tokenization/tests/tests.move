#[test_only]

module asset_tokenization::tests {
    // Imports
    use asset_tokenization::tokenized_asset::{Self, PlatformCap, TokenizedAsset};
    use asset_tokenization::proxy::{Self, Registry, ProtectedTP};
    use asset_tokenization::unlock::{Self};

    use std::string::{Self};
    use std::ascii;
    use std::string::{String, utf8};

    use iota::transfer_policy::{TransferPolicy};
    use iota::test_scenario::{Self, Scenario};
    use iota::url;
    use iota::tx_context::dummy;
    use iota::package::{Self};
    use iota::transfer_policy;
    use iota::coin;
    use iota::display;
    use iota::kiosk::{Self, Kiosk, KioskOwnerCap};
    use iota::kiosk_test_utils::{Self};

    // Constants
    const EWrongTotalSupply: u64 = 1;
    const EWrongSupply: u64 = 2;
    const EWrongBalanceValue: u64 = 3;

    const ADMIN: address = @0xAAAA;
    const USER: address = @0xBBBB;

    public struct TESTS has drop {}
    public struct WRONG_WITNESS has drop {}

    // Functions
    #[test]
    fun test_init() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        initialize(test, ADMIN);

        // Test that the admin got the PlatformCap
        // and that registry object was created
        test_scenario::next_tx(test, ADMIN);
        {
            // Check that registry was created
            let registry = test_scenario::take_shared<Registry>(test);
            test_scenario::return_shared<Registry>(registry);
        
            //Check that PlatformCap was created and passed to admin 
            let platform_cap = test_scenario::take_from_sender<PlatformCap>(test);
            test_scenario::return_to_sender(test, platform_cap);
        };
        test_scenario::end(scenario);
    }

    #[test]
    fun test_setup_tp() {
        let witness = TESTS {};
        let publisher = package::test_claim(witness, &mut dummy());

        let registry = proxy::test_registry(&mut dummy());
        let (policy, cap) = proxy::setup_tp<TESTS>(&registry, &publisher, &mut dummy());

        let coin = transfer_policy::destroy_and_withdraw(policy, cap, &mut dummy());
        coin::destroy_zero(coin);

        proxy::test_burn_registry(registry);
        package::burn_publisher(publisher);
    }


    // Cannot create type and publisher from different packages
    // #[test]
    // #[expected_failure(abort_code=proxy::ETypeNotFromPackage)]
    // fun test_setup_tp_failure() {}



     #[test]
    fun test_setup_display() {
        let witness = TESTS {};
        let publisher = package::test_claim(witness, &mut dummy());

        let registry = proxy::test_registry(&mut dummy());
        let mut display = proxy::new_display<TESTS>(&registry, &publisher, &mut dummy());

        display::add(&mut display, utf8(b"description"), utf8(b"test"));

        proxy::test_burn_registry(registry);
        package::burn_publisher(publisher);
        transfer::public_transfer(display, ADMIN);
    }

    // Cannot create type and publisher from different packages
    // #[test]
    // #[expected_failure(abort_code=proxy::ETypeNotFromPackage)]
    // fun test_setup_display_failure() {}


    #[test]
    fun test_get_publisher_mut() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        initialize(test, ADMIN);

        test_scenario::next_tx(test, ADMIN);
        {
            let mut registry = test_scenario::take_shared<Registry>(test);
            let platform_cap = test_scenario::take_from_sender<PlatformCap>(test);

            let _publisher_mut = proxy::publisher_mut(&platform_cap, &mut registry);

            test_scenario::return_shared<Registry>(registry);
            test_scenario::return_to_sender(test, platform_cap);
        };
        test_scenario::end(scenario);
    }


    #[test]
    fun test_create_new_asset() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "), 
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);

        let total_supply = tokenized_asset::total_supply(&asset_cap);
        let supply = tokenized_asset::supply(&asset_cap);

        assert!(total_supply == 100, EWrongTotalSupply);
        assert!(supply == 0, EWrongSupply);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        test_scenario::end(scenario);
    }

    #[test]
    #[expected_failure(abort_code=tokenized_asset::EBadWitness)]
    fun test_create_new_asset_with_wrong_witness() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = WRONG_WITNESS {};

        let (asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "), 
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);

        let total_supply = tokenized_asset::total_supply(&asset_cap);
        let supply = tokenized_asset::supply(&asset_cap);

        assert!(total_supply == 100, EWrongTotalSupply);
        assert!(supply == 0, EWrongSupply);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        test_scenario::end(scenario);
    }

    #[test]
    #[expected_failure(abort_code=tokenized_asset::EInsufficientTotalSupply)]
    fun test_create_new_asset_with_insufficient_total_supply() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 0, ascii::string(b"CORE "), 
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        test_scenario::end(scenario);
    }


    #[test]
    fun test_mint_ft() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);

        let keys = vector::empty<String>();
        let values = vector::empty<String>();

        let ft = tokenized_asset::mint(&mut asset_cap, keys, values, 1, ctx);
        let value = tokenized_asset::value(&ft);

        assert!(value == 1, EWrongBalanceValue);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(ft, USER);
        test_scenario::end(scenario);
    }


    #[test]
    #[expected_failure(abort_code=tokenized_asset::ENoSupply)]
    fun test_no_supply_to_mint() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 5, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), false, ctx);

        let keys = vector::empty<String>();
        let values = vector::empty<String>();

        let ft = tokenized_asset::mint(&mut asset_cap, keys, values, 6, ctx);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(ft, USER);
        test_scenario::end(scenario);
    }


    #[test]
    fun test_mint_nft() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);

        let (keys, values) = create_vectors();

        let nft = tokenized_asset::mint(&mut asset_cap, keys, values, 1, ctx);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(nft, USER);
        test_scenario::end(scenario);
    }


   #[test]
   #[expected_failure(abort_code=tokenized_asset::EUniqueAsset)]
    fun test_mint_non_unit_nft() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), false, ctx);

        let (keys, values) = create_vectors();

        let nft = tokenized_asset::mint(&mut asset_cap, keys, values, 5, ctx);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(nft, USER);
        test_scenario::end(scenario);
    }

    #[test]
    fun test_split_fts() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), false, ctx);

        let keys = vector::empty<String>();
        let values = vector::empty<String>();

        let mut ft = tokenized_asset::mint(&mut asset_cap, keys, values, 5, ctx);

        let new_tokenized_asset = tokenized_asset::split(&mut ft, 3, ctx);

        let ft_decreased_balance = tokenized_asset::value(&ft);
        let new_tokenized_asset_balance = tokenized_asset::value(&new_tokenized_asset);

        assert!(ft_decreased_balance == 2, EWrongBalanceValue);
        assert!(new_tokenized_asset_balance == 3, EWrongBalanceValue);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(ft, USER);
        transfer::public_transfer(new_tokenized_asset, USER);
        test_scenario::end(scenario);
    }

    #[test]
    #[expected_failure(abort_code=tokenized_asset::EInsufficientBalance)]
    fun test_split_fts_full_balance() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), false, ctx);

        let keys = vector::empty<String>();
        let values = vector::empty<String>();

        let mut ft = tokenized_asset::mint(&mut asset_cap, keys, values, 5, ctx);

        let new_tokenized_asset = tokenized_asset::split(&mut ft, 5, ctx);

        let ft_decreased_balance = tokenized_asset::value(&ft);
        let new_tokenized_asset_balance = tokenized_asset::value(&new_tokenized_asset);

        assert!(ft_decreased_balance == 0, EWrongBalanceValue);
        assert!(new_tokenized_asset_balance == 5, EWrongBalanceValue);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(ft, USER);
        transfer::public_transfer(new_tokenized_asset, USER);
        test_scenario::end(scenario);
    }



    #[test]
    #[expected_failure(abort_code=tokenized_asset::EInsufficientBalance)]
        fun test_split_fts_with_insufficient_balance_to_split() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);
        
        let keys = vector::empty<String>();
        let values = vector::empty<String>();

        let mut ft = tokenized_asset::mint(&mut asset_cap, keys, values, 3, ctx);

        let new_tokenized_asset = tokenized_asset::split(&mut ft, 5, ctx);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(ft, USER);
        transfer::public_transfer(new_tokenized_asset, USER);
        test_scenario::end(scenario);
    }

    #[test]
    #[expected_failure(abort_code=tokenized_asset::EInsufficientBalance)]
        fun test_split_fts_with_insufficient_balance_value() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);
        
        let keys = vector::empty<String>();
        let values = vector::empty<String>();

        let mut ft = tokenized_asset::mint(&mut asset_cap, keys, values, 1, ctx);

        let new_tokenized_asset = tokenized_asset::split(&mut ft, 5, ctx);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(ft, USER);
        transfer::public_transfer(new_tokenized_asset, USER);
        test_scenario::end(scenario);
    }

    #[test]
    #[expected_failure(abort_code=tokenized_asset::EZeroBalance)]
    fun test_split_fts_zero_balance() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), false, ctx);

        let keys = vector::empty<String>();
        let values = vector::empty<String>();

        let mut ft = tokenized_asset::mint(&mut asset_cap, keys, values, 5, ctx);

        let new_tokenized_asset = tokenized_asset::split(&mut ft, 0, ctx);

        let ft_decreased_balance = tokenized_asset::value(&ft);
        let new_tokenized_asset_balance = tokenized_asset::value(&new_tokenized_asset);

        assert!(ft_decreased_balance == 5, EWrongBalanceValue);
        assert!(new_tokenized_asset_balance == 0, EWrongBalanceValue);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(ft, USER);
        transfer::public_transfer(new_tokenized_asset, USER);
        test_scenario::end(scenario);
    }

    #[test]
    #[expected_failure(abort_code=tokenized_asset::EUniqueAsset)]
        fun test_split_nfts() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);
        
        let (keys, values) = create_vectors();

        let mut nft = tokenized_asset::mint(&mut asset_cap, keys, values, 1, ctx);

        let new_tokenized_asset = tokenized_asset::split(&mut nft, 1, ctx);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(nft, USER);
        transfer::public_transfer(new_tokenized_asset, USER);
        test_scenario::end(scenario);
    }


    #[test]
    fun test_join_fts() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);

        let keys = vector::empty<String>();
        let values = vector::empty<String>();

        let mut ft1 = tokenized_asset::mint(&mut asset_cap, keys, values, 2, ctx);
        let ft2 = tokenized_asset::mint(&mut asset_cap, keys, values, 3, ctx);

        tokenized_asset::join(&mut ft1, ft2);

        let joined_balance = tokenized_asset::value(&ft1);
        assert!(joined_balance == 5, EWrongBalanceValue);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(ft1, USER);
        test_scenario::end(scenario);
    }


    #[test]
    #[expected_failure(abort_code=tokenized_asset::EUniqueAsset)]
        fun test_join_nfts() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);
        
        let (keys1, values1) = create_vectors();
        let mut nft1 = tokenized_asset::mint(&mut asset_cap, keys1, values1, 1, ctx);

        let (keys2, values2) = create_vectors();
        let nft2 = tokenized_asset::mint(&mut asset_cap, keys2, values2, 1, ctx);

        tokenized_asset::join(&mut nft1, nft2);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        transfer::public_transfer(nft1, USER);
        test_scenario::end(scenario);
    }

    #[test]
    fun test_burn() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, ctx);

        let keys = vector::empty<String>();
        let values = vector::empty<String>();
        let new_tokenized_asset = tokenized_asset::mint(&mut asset_cap, keys, values, 1, ctx);

        tokenized_asset::burn(&mut asset_cap, new_tokenized_asset);

        let supply = tokenized_asset::supply(&asset_cap);
        assert!(supply == 0, EWrongSupply);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        test_scenario::end(scenario);
    }


    #[test]
    #[expected_failure(abort_code=tokenized_asset::ENonBurnable)]
    fun test_burn_unburnable() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), false, ctx);

        let keys = vector::empty<String>();
        let values = vector::empty<String>();
        let new_tokenized_asset = tokenized_asset::mint(&mut asset_cap, keys, values, 1, ctx);

        tokenized_asset::burn(&mut asset_cap, new_tokenized_asset);

        transfer::public_freeze_object(asset_metadata);
        transfer::public_transfer(asset_cap, tx_context::sender(ctx));
        test_scenario::end(scenario);
    }
    #[test]
    fun test_join_flow() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let witness = TESTS {};

        initialize(test, ADMIN);
        test_scenario::next_tx(test, ADMIN);
        {
            let registry = test_scenario::take_shared<Registry>(test);
            let publisher = package::claim(witness, test_scenario::ctx(test));

            let (policy, cap) = proxy::setup_tp<TESTS>(&registry, &publisher, test_scenario::ctx(test));
            transfer::public_transfer(policy, ADMIN);
            transfer::public_transfer(cap, ADMIN);
            transfer::public_transfer(publisher, ADMIN);
            test_scenario::return_shared(registry);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let (kiosk, kiosk_cap) = kiosk_test_utils::get_kiosk(test_scenario::ctx(test));
            transfer::public_share_object(kiosk);
            transfer::public_transfer(kiosk_cap, ADMIN);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let policy = test_scenario::take_from_sender<TransferPolicy<TokenizedAsset<TESTS>>>(test);
            let protected_tp = test_scenario::take_shared<ProtectedTP<TokenizedAsset<TESTS>>>(test);
            let witness = TESTS {};
            let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
                string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, test_scenario::ctx(test));

            let keys = vector::empty<String>();
            let values = vector::empty<String>();

            let ft1 = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));
            let ft2 = tokenized_asset::mint(&mut asset_cap, keys, values, 3, test_scenario::ctx(test));

            let ft1_id = object::id(&ft1);
            let ft2_id = object::id(&ft2);

            let mut kiosk = test_scenario::take_shared<Kiosk>(test);
            let kiosk_cap = test_scenario::take_from_sender<KioskOwnerCap>(test);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft1);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft2);

            kiosk::list<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft2_id, 0);
            let coin = kiosk_test_utils::get_iota(0, test_scenario::ctx(test));
            let (mut ft1, promise_ft1) = kiosk::borrow_val<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft1_id);

            let (ft2, transfer_request) = kiosk::purchase<TokenizedAsset<TESTS>>(&mut kiosk, ft2_id, coin);

            let promise = unlock::asset_from_kiosk_to_join(&ft1, &ft2, &protected_tp, transfer_request); 

            let burn_proof = tokenized_asset::join(&mut ft1, ft2);

            unlock::prove_join(&ft1, promise, burn_proof);

            kiosk::return_val<TokenizedAsset<TESTS>>(&mut kiosk, ft1, promise_ft1);

            test_scenario::return_shared(protected_tp);
            test_scenario::return_shared(kiosk);
            test_scenario::return_to_sender(&scenario, kiosk_cap);
            test_scenario::return_to_sender(&scenario, policy);
            transfer::public_transfer(asset_cap, ADMIN);
            transfer::public_transfer(asset_metadata, ADMIN);
        };
        test_scenario::end(scenario);
    }

    #[test]
    #[expected_failure(abort_code=unlock::EWrongItem)]
    fun test_join_flow_wrong_item() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let witness = TESTS {};

        initialize(test, ADMIN);
        test_scenario::next_tx(test, ADMIN);
        {
            let registry = test_scenario::take_shared<Registry>(test);
            let publisher = package::claim(witness, test_scenario::ctx(test));

            let (policy, cap) = proxy::setup_tp<TESTS>(&registry, &publisher, test_scenario::ctx(test));
            transfer::public_transfer(policy, ADMIN);
            transfer::public_transfer(cap, ADMIN);
            transfer::public_transfer(publisher, ADMIN);
            test_scenario::return_shared(registry);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let (kiosk, kiosk_cap) = kiosk_test_utils::get_kiosk(test_scenario::ctx(test));
            transfer::public_share_object(kiosk);
            transfer::public_transfer(kiosk_cap, ADMIN);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let policy = test_scenario::take_from_sender<TransferPolicy<TokenizedAsset<TESTS>>>(test);
            let protected_tp = test_scenario::take_shared<ProtectedTP<TokenizedAsset<TESTS>>>(test);
            let witness = TESTS {};
            let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
                string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, test_scenario::ctx(test));

            let keys = vector::empty<String>();
            let values = vector::empty<String>();

            let ft1 = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));
            let ft2 = tokenized_asset::mint(&mut asset_cap, keys, values, 3, test_scenario::ctx(test));

            let ft1_id = object::id(&ft1);
            let ft2_id = object::id(&ft2);

            let mut kiosk = test_scenario::take_shared<Kiosk>(test);
            let kiosk_cap = test_scenario::take_from_sender<KioskOwnerCap>(test);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft1);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft2);

            kiosk::list<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft2_id, 0);
            let coin = kiosk_test_utils::get_iota(0, test_scenario::ctx(test));
            let (mut ft1, promise_ft1) = kiosk::borrow_val<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft1_id);

            let (ft2, transfer_request) = kiosk::purchase<TokenizedAsset<TESTS>>(&mut kiosk, ft2_id, coin);

            let promise = unlock::asset_from_kiosk_to_join(&ft2, &ft1, &protected_tp, transfer_request); 

            let burn_proof = tokenized_asset::join(&mut ft1, ft2);

            unlock::prove_join(&ft1, promise, burn_proof);

            kiosk::return_val<TokenizedAsset<TESTS>>(&mut kiosk, ft1, promise_ft1);

            test_scenario::return_shared(protected_tp);
            test_scenario::return_shared(kiosk);
            test_scenario::return_to_sender(&scenario, kiosk_cap);
            test_scenario::return_to_sender(&scenario, policy);
            transfer::public_transfer(asset_cap, ADMIN);
            transfer::public_transfer(asset_metadata, ADMIN);
        };
        test_scenario::end(scenario);
    }


    #[test]
    #[expected_failure(abort_code=unlock::ENotExpectedBalance)]
    fun test_join_flow_not_expected_balance() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let witness = TESTS {};

        initialize(test, ADMIN);
        test_scenario::next_tx(test, ADMIN);
        {
            let registry = test_scenario::take_shared<Registry>(test);
            let publisher = package::claim(witness, test_scenario::ctx(test));

            let (policy, cap) = proxy::setup_tp<TESTS>(&registry, &publisher, test_scenario::ctx(test));
            transfer::public_transfer(policy, ADMIN);
            transfer::public_transfer(cap, ADMIN);
            transfer::public_transfer(publisher, ADMIN);
            test_scenario::return_shared(registry);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let (kiosk, kiosk_cap) = kiosk_test_utils::get_kiosk(test_scenario::ctx(test));
            transfer::public_share_object(kiosk);
            transfer::public_transfer(kiosk_cap, ADMIN);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let policy = test_scenario::take_from_sender<TransferPolicy<TokenizedAsset<TESTS>>>(test);
            let protected_tp = test_scenario::take_shared<ProtectedTP<TokenizedAsset<TESTS>>>(test);
            let witness = TESTS {};
            let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
                string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, test_scenario::ctx(test));

            let keys = vector::empty<String>();
            let values = vector::empty<String>();

            let ft1 = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));
            let ft2 = tokenized_asset::mint(&mut asset_cap, keys, values, 3, test_scenario::ctx(test));
            let ft3 = tokenized_asset::mint(&mut asset_cap, keys, values, 3, test_scenario::ctx(test));

            let ft1_id = object::id(&ft1);
            let ft2_id = object::id(&ft2);
            let ft3_id = object::id(&ft3);

            let mut kiosk = test_scenario::take_shared<Kiosk>(test);
            let kiosk_cap = test_scenario::take_from_sender<KioskOwnerCap>(test);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft1);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft2);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft3);

            kiosk::list<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft2_id, 0);
            let coin = kiosk_test_utils::get_iota(0, test_scenario::ctx(test));
            let (mut ft1, promise_ft1) = kiosk::borrow_val<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft1_id);
            let (ft3, promise_ft3) = kiosk::borrow_val<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft3_id);
            let (ft2, transfer_request) = kiosk::purchase<TokenizedAsset<TESTS>>(&mut kiosk, ft2_id, coin);

            let promise = unlock::asset_from_kiosk_to_join(&ft1, &ft2, &protected_tp, transfer_request); 

            let burn_proof = tokenized_asset::join(&mut ft1, ft2);

            unlock::prove_join(&ft3, promise, burn_proof);

            kiosk::return_val<TokenizedAsset<TESTS>>(&mut kiosk, ft1, promise_ft1);
            kiosk::return_val<TokenizedAsset<TESTS>>(&mut kiosk, ft3, promise_ft3);

            test_scenario::return_shared(protected_tp);
            test_scenario::return_shared(kiosk);
            test_scenario::return_to_sender(&scenario, kiosk_cap);
            test_scenario::return_to_sender(&scenario, policy);
            transfer::public_transfer(asset_cap, ADMIN);
            transfer::public_transfer(asset_metadata, ADMIN);
        };
        test_scenario::end(scenario);
    }


    #[test]
    #[expected_failure(abort_code=unlock::ENotPromisedItem)]
    fun test_join_flow_not_promised_item() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let witness = TESTS {};

        initialize(test, ADMIN);
        test_scenario::next_tx(test, ADMIN);
        {
            let registry = test_scenario::take_shared<Registry>(test);
            let publisher = package::claim(witness, test_scenario::ctx(test));

            let (policy, cap) = proxy::setup_tp<TESTS>(&registry, &publisher, test_scenario::ctx(test));
            transfer::public_transfer(policy, ADMIN);
            transfer::public_transfer(cap, ADMIN);
            transfer::public_transfer(publisher, ADMIN);
            test_scenario::return_shared(registry);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let (kiosk, kiosk_cap) = kiosk_test_utils::get_kiosk(test_scenario::ctx(test));
            transfer::public_share_object(kiosk);
            transfer::public_transfer(kiosk_cap, ADMIN);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let policy = test_scenario::take_from_sender<TransferPolicy<TokenizedAsset<TESTS>>>(test);
            let protected_tp = test_scenario::take_shared<ProtectedTP<TokenizedAsset<TESTS>>>(test);
            let witness = TESTS {};
            let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
                string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, test_scenario::ctx(test));

            let keys = vector::empty<String>();
            let values = vector::empty<String>();

            let ft1 = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));
            let ft2 = tokenized_asset::mint(&mut asset_cap, keys, values, 3, test_scenario::ctx(test));
            let ft3 = tokenized_asset::mint(&mut asset_cap, keys, values, 5, test_scenario::ctx(test));

            let ft1_id = object::id(&ft1);
            let ft2_id = object::id(&ft2);
            let ft3_id = object::id(&ft3);

            let mut kiosk = test_scenario::take_shared<Kiosk>(test);
            let kiosk_cap = test_scenario::take_from_sender<KioskOwnerCap>(test);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft1);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft2);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft3);

            kiosk::list<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft2_id, 0);
            let coin = kiosk_test_utils::get_iota(0, test_scenario::ctx(test));

            let (mut ft1, promise_ft1) = kiosk::borrow_val<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft1_id);
            let (ft3, promise_ft3) = kiosk::borrow_val<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft3_id);
            let (ft2, transfer_request) = kiosk::purchase<TokenizedAsset<TESTS>>(&mut kiosk, ft2_id, coin);

            let promise = unlock::asset_from_kiosk_to_join(&ft1, &ft2, &protected_tp, transfer_request); 

            let burn_proof = tokenized_asset::join(&mut ft1, ft2);

            unlock::prove_join(&ft3, promise, burn_proof);
            
            kiosk::return_val<TokenizedAsset<TESTS>>(&mut kiosk, ft1, promise_ft1);
            kiosk::return_val<TokenizedAsset<TESTS>>(&mut kiosk, ft3, promise_ft3);

            test_scenario::return_shared(protected_tp);
            test_scenario::return_shared(kiosk);
            test_scenario::return_to_sender(&scenario, kiosk_cap);
            test_scenario::return_to_sender(&scenario, policy);
            transfer::public_transfer(asset_cap, ADMIN);
            transfer::public_transfer(asset_metadata, ADMIN);
        };
        test_scenario::end(scenario);
    }


     #[test]
    #[expected_failure(abort_code=unlock::ENotBurnedItem)]
    fun test_join_flow_not_burned_item() {
        let mut scenario= test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let witness = TESTS {};

        initialize(test, ADMIN);
        test_scenario::next_tx(test, ADMIN);
        {
            let registry = test_scenario::take_shared<Registry>(test);
            let publisher = package::claim(witness, test_scenario::ctx(test));

            let (policy, cap) = proxy::setup_tp<TESTS>(&registry, &publisher, test_scenario::ctx(test));
            transfer::public_transfer(policy, ADMIN);
            transfer::public_transfer(cap, ADMIN);
            transfer::public_transfer(publisher, ADMIN);
            test_scenario::return_shared(registry);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let (kiosk, kiosk_cap) = kiosk_test_utils::get_kiosk(test_scenario::ctx(test));
            transfer::public_share_object(kiosk);
            transfer::public_transfer(kiosk_cap, ADMIN);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let policy = test_scenario::take_from_sender<TransferPolicy<TokenizedAsset<TESTS>>>(test);
            let protected_tp = test_scenario::take_shared<ProtectedTP<TokenizedAsset<TESTS>>>(test);
            let witness = TESTS {};
            let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
                string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, test_scenario::ctx(test));

            let keys = vector::empty<String>();
            let values = vector::empty<String>();

            let ft1 = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));
            let ft2 = tokenized_asset::mint(&mut asset_cap, keys, values, 3, test_scenario::ctx(test));
            let ft3 = tokenized_asset::mint(&mut asset_cap, keys, values, 3, test_scenario::ctx(test));

            let ft1_id = object::id(&ft1);
            let ft2_id = object::id(&ft2);
            let ft3_id = object::id(&ft3);

            let mut kiosk = test_scenario::take_shared<Kiosk>(test);
            let kiosk_cap = test_scenario::take_from_sender<KioskOwnerCap>(test);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft1);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft2);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft3);

            kiosk::list<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft2_id, 0);
            let coin2 = kiosk_test_utils::get_iota(0, test_scenario::ctx(test));

            kiosk::list<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft3_id, 0);
            let coin3 = kiosk_test_utils::get_iota(0, test_scenario::ctx(test));

            let (mut ft1, promise_ft1) = kiosk::borrow_val<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft1_id);
            let (ft2, transfer_request2) = kiosk::purchase<TokenizedAsset<TESTS>>(&mut kiosk, ft2_id, coin2);
            let (ft3, transfer_request3) = kiosk::purchase<TokenizedAsset<TESTS>>(&mut kiosk, ft3_id, coin3);


            let promise2 = unlock::asset_from_kiosk_to_join(&ft1, &ft2, &protected_tp, transfer_request2); 
            
            let promise3 = unlock::asset_from_kiosk_to_join(&ft1, &ft3, &protected_tp, transfer_request3); 
            
            let burn_proof2 = tokenized_asset::join(&mut ft1, ft2);
            let burn_proof3 = tokenized_asset::join(&mut ft1, ft3);
            let ft1_split = tokenized_asset::split(&mut ft1, 3, test_scenario::ctx(test));

            unlock::prove_join(&ft1, promise2, burn_proof3);
            unlock::prove_join(&ft1, promise3, burn_proof2);
            
            kiosk::return_val<TokenizedAsset<TESTS>>(&mut kiosk, ft1, promise_ft1);
            transfer::public_transfer(ft1_split, ADMIN);
            test_scenario::return_shared(protected_tp);
            test_scenario::return_shared(kiosk);
            test_scenario::return_to_sender(&scenario, kiosk_cap);
            test_scenario::return_to_sender(&scenario, policy);
            transfer::public_transfer(asset_cap, ADMIN);
            transfer::public_transfer(asset_metadata, ADMIN);
        };
        test_scenario::end(scenario);
    }


    #[test]
    fun test_burn_flow() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let witness = TESTS {};

        initialize(test, ADMIN);
        test_scenario::next_tx(test, ADMIN);
        {
            let registry = test_scenario::take_shared<Registry>(test);
            let publisher = package::claim(witness, test_scenario::ctx(test));

            let (policy, cap) = proxy::setup_tp<TESTS>(&registry, &publisher, test_scenario::ctx(test));
            transfer::public_transfer(policy, ADMIN);
            transfer::public_transfer(cap, ADMIN);
            transfer::public_transfer(publisher, ADMIN);
            test_scenario::return_shared(registry);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let (kiosk, kiosk_cap) = kiosk_test_utils::get_kiosk(test_scenario::ctx(test));
            transfer::public_share_object(kiosk);
            transfer::public_transfer(kiosk_cap, ADMIN);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let policy = test_scenario::take_from_sender<TransferPolicy<TokenizedAsset<TESTS>>>(test);
            let protected_tp = test_scenario::take_shared<ProtectedTP<TokenizedAsset<TESTS>>>(test);
            let witness = TESTS {};
            let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
                string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, test_scenario::ctx(test));

            let keys = vector::empty<String>();
            let values = vector::empty<String>();

            let ft = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));

            let ft_id = object::id(&ft);

            let mut kiosk = test_scenario::take_shared<Kiosk>(test);
            let kiosk_cap = test_scenario::take_from_sender<KioskOwnerCap>(test);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft);

            kiosk::list<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft_id, 0);
            let coin = kiosk_test_utils::get_iota(0, test_scenario::ctx(test));

            let (ft, transfer_request) = kiosk::purchase<TokenizedAsset<TESTS>>(&mut kiosk, ft_id, coin);

            let burn_promise = unlock::asset_from_kiosk_to_burn(&ft, &asset_cap, &protected_tp, transfer_request); 

            tokenized_asset::burn(&mut asset_cap, ft);

            unlock::prove_burn(&asset_cap, burn_promise);

            test_scenario::return_shared(protected_tp);
            test_scenario::return_shared(kiosk);
            test_scenario::return_to_sender(&scenario, kiosk_cap);
            test_scenario::return_to_sender(&scenario, policy);
            transfer::public_transfer(asset_cap, ADMIN);
            transfer::public_transfer(asset_metadata, ADMIN);
        };
        test_scenario::end(scenario);
    }

    #[test]
    #[expected_failure(abort_code=unlock::EWrongItem)]
    fun test_burn_flow_wrong_item() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let witness = TESTS {};

        initialize(test, ADMIN);
        test_scenario::next_tx(test, ADMIN);
        {
            let registry = test_scenario::take_shared<Registry>(test);
            let publisher = package::claim(witness, test_scenario::ctx(test));

            let (policy, cap) = proxy::setup_tp<TESTS>(&registry, &publisher, test_scenario::ctx(test));
            transfer::public_transfer(policy, ADMIN);
            transfer::public_transfer(cap, ADMIN);
            transfer::public_transfer(publisher, ADMIN);
            test_scenario::return_shared(registry);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let (kiosk, kiosk_cap) = kiosk_test_utils::get_kiosk(test_scenario::ctx(test));
            transfer::public_share_object(kiosk);
            transfer::public_transfer(kiosk_cap, ADMIN);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let policy = test_scenario::take_from_sender<TransferPolicy<TokenizedAsset<TESTS>>>(test);
            let protected_tp = test_scenario::take_shared<ProtectedTP<TokenizedAsset<TESTS>>>(test);
            let witness = TESTS {};
            let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
                string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, test_scenario::ctx(test));

            let keys = vector::empty<String>();
            let values = vector::empty<String>();

            let ft = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));
            let wrong_ft = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));


            let ft_id = object::id(&ft);

            let mut kiosk = test_scenario::take_shared<Kiosk>(test);
            let kiosk_cap = test_scenario::take_from_sender<KioskOwnerCap>(test);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft);

            kiosk::list<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft_id, 0);
            let coin = kiosk_test_utils::get_iota(0, test_scenario::ctx(test));

            let (ft, transfer_request) = kiosk::purchase<TokenizedAsset<TESTS>>(&mut kiosk, ft_id, coin);

            let burn_promise = unlock::asset_from_kiosk_to_burn(&wrong_ft, &asset_cap, &protected_tp, transfer_request); 

            tokenized_asset::burn(&mut asset_cap, wrong_ft);

            unlock::prove_burn(&asset_cap, burn_promise);

            transfer::public_transfer(ft, ADMIN);
            test_scenario::return_shared(protected_tp);
            test_scenario::return_shared(kiosk);
            test_scenario::return_to_sender(&scenario, kiosk_cap);
            test_scenario::return_to_sender(&scenario, policy);
            transfer::public_transfer(asset_cap, ADMIN);
            transfer::public_transfer(asset_metadata, ADMIN);
        };
        test_scenario::end(scenario);
    }


    #[test]
    #[expected_failure(abort_code=unlock::ENotExpectedSupply)]
    fun test_burn_flow_not_expected_supply() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let witness = TESTS {};

        initialize(test, ADMIN);
        test_scenario::next_tx(test, ADMIN);
        {
            let registry = test_scenario::take_shared<Registry>(test);
            let publisher = package::claim(witness, test_scenario::ctx(test));

            let (policy, cap) = proxy::setup_tp<TESTS>(&registry, &publisher, test_scenario::ctx(test));
            transfer::public_transfer(policy, ADMIN);
            transfer::public_transfer(cap, ADMIN);
            transfer::public_transfer(publisher, ADMIN);
            test_scenario::return_shared(registry);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let (kiosk, kiosk_cap) = kiosk_test_utils::get_kiosk(test_scenario::ctx(test));
            transfer::public_share_object(kiosk);
            transfer::public_transfer(kiosk_cap, ADMIN);
        };

        test_scenario::next_tx(test, ADMIN);
        {
            let policy = test_scenario::take_from_sender<TransferPolicy<TokenizedAsset<TESTS>>>(test);
            let protected_tp = test_scenario::take_shared<ProtectedTP<TokenizedAsset<TESTS>>>(test);
            let witness = TESTS {};
            let (mut asset_cap, asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
                string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), true, test_scenario::ctx(test));

            let keys = vector::empty<String>();
            let values = vector::empty<String>();

            let ft = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));

            let ft_id = object::id(&ft);

            let mut kiosk = test_scenario::take_shared<Kiosk>(test);
            let kiosk_cap = test_scenario::take_from_sender<KioskOwnerCap>(test);
            kiosk::lock<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, &policy, ft);

            kiosk::list<TokenizedAsset<TESTS>>(&mut kiosk, &kiosk_cap, ft_id, 0);
            let coin = kiosk_test_utils::get_iota(0, test_scenario::ctx(test));

            let (ft, transfer_request) = kiosk::purchase<TokenizedAsset<TESTS>>(&mut kiosk, ft_id, coin);

            let burn_promise = unlock::asset_from_kiosk_to_burn(&ft, &asset_cap, &protected_tp, transfer_request); 

            tokenized_asset::burn(&mut asset_cap, ft);

            let ft2 = tokenized_asset::mint(&mut asset_cap, keys, values, 2, test_scenario::ctx(test));

            unlock::prove_burn(&asset_cap, burn_promise);

            test_scenario::return_shared(protected_tp);
            test_scenario::return_shared(kiosk);
            test_scenario::return_to_sender(&scenario, kiosk_cap);
            test_scenario::return_to_sender(&scenario, policy);
            transfer::public_transfer(ft2, ADMIN);
            transfer::public_transfer(asset_cap, ADMIN);
            transfer::public_transfer(asset_metadata, ADMIN);
        };
        test_scenario::end(scenario);
    }


    #[test]
    #[expected_failure(abort_code=tokenized_asset::EVecLengthMismatch)]
    fun test_invalid_vec_map() {
        let mut scenario = test_scenario::begin(ADMIN);
        let test = &mut scenario;
        let ctx = test_scenario::ctx(test);
        let witness = TESTS {};

        let (mut asset_cap, _asset_metadata) = tokenized_asset::new_asset(witness, 100, ascii::string(b"CORE "),
            string::utf8(b"asset_name"), string::utf8(b"description"), option::some(url::new_unsafe_from_bytes(b"icon_url")), false, ctx);

        let (keys, mut values) = create_vectors();
        vector::push_back(&mut values, string::utf8(b"No"));

        let _new_tokenized_asset = tokenized_asset::mint(&mut asset_cap, keys, values, 1, ctx);

        abort 1337
    }


    // Helper function: create key value vectors
    fun create_vectors(): (vector<String>, vector<String>) {

        let keys = vector[
            string::utf8(b"Piece"),
            string::utf8(b"Is it Amazing?"),
            string::utf8(b"In a scale from 1 to 10, how good?"),
        ];

        let values = vector[
            string::utf8(b"1/100"),
            string::utf8(b"Yes"),
            string::utf8(b"11"),
        ];

        (keys, values)
    }

    // Helper function initialize
    fun initialize(scenario: &mut Scenario, admin: address) {
        test_scenario::next_tx(scenario, admin);
        {
            proxy::test_init(test_scenario::ctx(scenario));
        };
        test_scenario::next_tx(scenario, admin);
        {
            tokenized_asset::test_init(test_scenario::ctx(scenario));
        };
    }
}