
#[test_only]
module nft_marketplace::rental_extension_tests {

    use kiosk::kiosk_lock_rule as lock_rule;
    use nft_marketplace::test_utils::{create_kiosk, create_transfer_policy};
    use nft_marketplace::rental_extension::{Self, ProtectedTP, RentalPolicy};
    use iota::{
        clock::{Self, Clock},
        kiosk::{Kiosk, KioskOwnerCap},
        kiosk_test_utils,
        package::{Self, Publisher},
        test_scenario::{Self as ts, Scenario},
        transfer_policy::{TransferPolicy, TransferPolicyCap}
    };

    const CREATOR: address = @0xCCCC;
    const RENTER: address = @0xAAAA;
    const BORROWER: address = @0xBBBB;
    const THIEF: address = @0xDDDD;

    public struct T has key, store { id: UID }
    public struct WITNESS has drop {}


// ==================== Happy path scenarios ====================

    #[test]
    fun test_rent_with_extension() {
        let mut ts = ts::begin(BORROWER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let clock = clock::create_for_testing(ts.ctx());

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let renter_kiosk_id = create_kiosk(RENTER, ts.ctx());
        let borrower_kiosk_id = create_kiosk(BORROWER, ts.ctx());

        setup(&mut ts, RENTER, &publisher, 50);
        install_ext(&mut ts, RENTER, renter_kiosk_id);
        place_in_kiosk(&mut ts, RENTER, renter_kiosk_id, item);
        list_for_rent(&mut ts, RENTER, renter_kiosk_id, item_id, 10, 10);
        install_ext(&mut ts, BORROWER, borrower_kiosk_id);
        rent(&mut ts, BORROWER, renter_kiosk_id, borrower_kiosk_id, item_id, 100, &clock);

        clock.destroy_for_testing();
        publisher.burn_publisher();
        ts.end();
    }

    #[test]
    fun test_reclaim() {
        let mut ts = ts::begin(BORROWER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let mut clock = clock::create_for_testing(ts.ctx());

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let renter_kiosk_id = create_kiosk(RENTER, ts.ctx());
        let borrower_kiosk_id = create_kiosk(BORROWER, ts.ctx());

        create_transfer_policy<T>( CREATOR, &publisher, ts.ctx());
        setup(&mut ts, RENTER, &publisher, 50);
        place_in_kiosk(&mut ts, RENTER, renter_kiosk_id, item);
        install_ext(&mut ts, RENTER, renter_kiosk_id);
        list_for_rent(&mut ts, RENTER, renter_kiosk_id, item_id, 10, 10);
        install_ext(&mut ts, BORROWER, borrower_kiosk_id);
        rent(&mut ts, BORROWER, renter_kiosk_id, borrower_kiosk_id, item_id, 100, &clock);
        reclaim(&mut ts, RENTER, renter_kiosk_id, borrower_kiosk_id, item_id, 432000000, &mut clock);

        clock.destroy_for_testing();
        publisher.burn_publisher();
        ts.end();
    }

    // ==================== Negative scenarios ====================

    #[test]
    #[expected_failure(abort_code = rental_extension::EExtensionNotInstalled)]
    fun test_rent_without_extension() {
        let mut ts = ts::begin(BORROWER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let clock = clock::create_for_testing(ts.ctx());

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let renter_kiosk_id = create_kiosk(RENTER, ts.ctx());
        let borrower_kiosk_id = create_kiosk(BORROWER, ts.ctx());

        setup(&mut ts, RENTER, &publisher, 50);
        place_in_kiosk(&mut ts, RENTER, renter_kiosk_id, item);
        install_ext(&mut ts, RENTER, renter_kiosk_id);
        list_for_rent(&mut ts, RENTER, renter_kiosk_id, item_id, 10, 10);
        rent(&mut ts, BORROWER, renter_kiosk_id, borrower_kiosk_id, item_id, 100, &clock);
        abort 0xbad
    }

    #[test]
    #[expected_failure(abort_code = rental_extension::ENotEnoughCoins)]
    fun test_rent_with_not_enough_coins() {
        let mut ts = ts::begin(BORROWER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let clock = clock::create_for_testing(ts.ctx());

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let renter_kiosk_id = create_kiosk(RENTER, ts.ctx());
        let borrower_kiosk_id = create_kiosk(BORROWER, ts.ctx());

        setup(&mut ts, RENTER, &publisher, 50);
        place_in_kiosk(&mut ts, RENTER, renter_kiosk_id, item);
        install_ext(&mut ts, RENTER, renter_kiosk_id);
        list_for_rent(&mut ts, RENTER, renter_kiosk_id, item_id, 10, 10);
        install_ext(&mut ts, BORROWER, borrower_kiosk_id);
        rent(&mut ts, BORROWER, renter_kiosk_id, borrower_kiosk_id, item_id, 10, &clock);
        abort 0xbad
    }

    #[test]
    #[expected_failure(abort_code = rental_extension::ETotalPriceOverflow)]
    fun test_rent_with_overflow() {
        let mut ts = ts::begin(BORROWER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let clock = clock::create_for_testing(ts.ctx());

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let renter_kiosk_id = create_kiosk(RENTER, ts.ctx());
        let borrower_kiosk_id = create_kiosk(BORROWER, ts.ctx());

        setup(&mut ts, RENTER, &publisher, 50);
        place_in_kiosk(&mut ts, RENTER, renter_kiosk_id, item);
        install_ext(&mut ts, RENTER, renter_kiosk_id);
        list_for_rent(&mut ts, RENTER, renter_kiosk_id, item_id, 100, 1844674407370955160);
        install_ext(&mut ts, BORROWER, borrower_kiosk_id);
        rent(&mut ts, BORROWER, renter_kiosk_id, borrower_kiosk_id, item_id, 100, &clock);
        abort 0xbad
    }

    #[test]
    fun test_reclaim_locked() {
        let mut ts = ts::begin(RENTER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let mut clock = clock::create_for_testing(ts.ctx());

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let renter_kiosk_id = create_kiosk(RENTER, ts.ctx());
        let borrower_kiosk_id = create_kiosk(BORROWER, ts.ctx());

        create_transfer_policy<T>(CREATOR, &publisher, ts.ctx());
        add_lock_rule(&mut ts, CREATOR);
        setup(&mut ts, RENTER, &publisher, 50);
        lock_in_kiosk(&mut ts, RENTER, renter_kiosk_id, item);
        install_ext(&mut ts, RENTER, renter_kiosk_id);
        list_for_rent(&mut ts, RENTER, renter_kiosk_id, item_id, 10, 10);
        install_ext(&mut ts, BORROWER, borrower_kiosk_id);
        rent(&mut ts, BORROWER, renter_kiosk_id, borrower_kiosk_id, item_id, 100, &clock);
        reclaim(&mut ts, RENTER, renter_kiosk_id, borrower_kiosk_id, item_id, 432000000, &mut clock);

        clock.destroy_for_testing();
        publisher.burn_publisher();
        ts.end();
    }

    #[test]
    #[expected_failure(abort_code = rental_extension::EInvalidKiosk)]
    fun test_reclaim_wrong_kiosk() {
        let mut ts = ts::begin(BORROWER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let mut clock = clock::create_for_testing(ts.ctx());

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let renter_kiosk_id = create_kiosk(RENTER, ts.ctx());
        let borrower_kiosk_id = create_kiosk(BORROWER, ts.ctx());
        let thief_kiosk_id = create_kiosk(THIEF, ts.ctx());

        create_transfer_policy<T>(CREATOR, &publisher, ts.ctx());
        setup(&mut ts, RENTER, &publisher, 50);
        place_in_kiosk(&mut ts, RENTER, renter_kiosk_id, item);
        install_ext(&mut ts, RENTER, renter_kiosk_id);
        list_for_rent(&mut ts, RENTER, renter_kiosk_id, item_id, 10, 10);
        install_ext(&mut ts, BORROWER, borrower_kiosk_id);
        rent(&mut ts, BORROWER, renter_kiosk_id, borrower_kiosk_id, item_id, 100, &clock);
        install_ext(&mut ts, THIEF, thief_kiosk_id);
        reclaim(&mut ts, RENTER, thief_kiosk_id, borrower_kiosk_id, item_id, 432000000, &mut clock);
        abort 0xbad
    }

    // ==================== Helper methods ====================

    fun place_in_kiosk(ts: &mut Scenario, sender: address, kiosk_id: ID, item: T) {
        ts.next_tx(sender);
        let mut kiosk: Kiosk = ts.take_shared_by_id(kiosk_id);
        let kiosk_cap: KioskOwnerCap = ts.take_from_sender();

        kiosk.place(&kiosk_cap, item);

        ts::return_shared(kiosk);
        ts.return_to_sender(kiosk_cap);
    }


    fun list_for_rent(
        ts: &mut Scenario,
        sender: address,
        kiosk_id: ID,
        item_id: ID,
        duration: u64,
        price: u64,
    ) {
        ts.next_tx(sender);
        let mut kiosk: Kiosk = ts.take_shared_by_id(kiosk_id);
        let kiosk_cap: KioskOwnerCap = ts.take_from_sender();
        let protected_tp: ProtectedTP<T> = ts.take_shared();

        rental_extension::list(
            &mut kiosk,
            &kiosk_cap,
            &protected_tp,
            item_id,
            duration,
            price,
            ts.ctx(),
        );

        ts::return_shared(kiosk);
        ts.return_to_sender(kiosk_cap);
        ts::return_shared(protected_tp);
    }

    fun rent(
        ts: &mut Scenario,
        sender: address,
        renter_kiosk_id: ID,
        borrower_kiosk_id: ID,
        item_id: ID,
        coin_amount: u64,
        clock: &Clock,
    ) {
        ts.next_tx(sender);

        let mut borrower_kiosk: Kiosk = ts.take_shared_by_id(borrower_kiosk_id);
        let mut renter_kiosk: Kiosk = ts.take_shared_by_id(renter_kiosk_id);
        let mut rental_policy: RentalPolicy<T> = ts.take_shared();

        let coin = kiosk_test_utils::get_iota(coin_amount, ts.ctx());

        rental_extension::rent<T>(
            &mut renter_kiosk,
            &mut borrower_kiosk,
            &mut rental_policy,
            item_id,
            coin,
            clock,
            ts.ctx(),
        );

        ts::return_shared(borrower_kiosk);
        ts::return_shared(renter_kiosk);
        ts::return_shared(rental_policy);
    }

    fun setup(ts: &mut Scenario, sender: address, publisher: &Publisher, amount_bp: u64) {
        ts.next_tx(sender);
        rental_extension::setup_renting<T>(publisher, amount_bp, ts.ctx());
    }

    fun reclaim(
        ts: &mut Scenario,
        sender: address,
        renter_kiosk_id: ID,
        borrower_kiosk_id: ID,
        item_id: ID,
        tick: u64,
        clock: &mut Clock,
    ) {
        ts.next_tx(sender);
        let mut borrower_kiosk: Kiosk = ts.take_shared_by_id(borrower_kiosk_id);
        let mut renter_kiosk: Kiosk = ts.take_shared_by_id(renter_kiosk_id);
        let policy: TransferPolicy<T> = ts.take_shared();

        clock.increment_for_testing(tick);
        rental_extension::reclaim(
            &mut renter_kiosk,
            &mut borrower_kiosk,
            &policy,
            clock,
            item_id,
            ts.ctx(),
        );

        ts::return_shared(policy);
        ts::return_shared(borrower_kiosk);
        ts::return_shared(renter_kiosk);
    }

    fun add_lock_rule(ts: &mut Scenario, sender: address) {
        ts.next_tx(sender);
        let mut transfer_policy: TransferPolicy<T> = ts.take_shared();
        let policy_cap: TransferPolicyCap<T> = ts.take_from_sender();

        lock_rule::add(&mut transfer_policy, &policy_cap);

        ts::return_shared(transfer_policy);
        ts.return_to_sender(policy_cap);
    }

    fun lock_in_kiosk(ts: &mut Scenario, sender: address, kiosk_id: ID, item: T) {
        ts.next_tx(sender);

        let mut kiosk: Kiosk = ts.take_shared_by_id(kiosk_id);
        let kiosk_cap: KioskOwnerCap = ts.take_from_sender();
        let transfer_policy: TransferPolicy<T> = ts.take_shared();

        kiosk.lock(&kiosk_cap, &transfer_policy, item);

        ts::return_shared(kiosk);
        ts.return_to_sender(kiosk_cap);
        ts::return_shared(transfer_policy);
    }

    fun install_ext(ts: &mut Scenario, sender: address, kiosk_id: ID) {
        ts.next_tx(sender);
        let mut kiosk: Kiosk = ts.take_shared_by_id(kiosk_id);
        let kiosk_cap = ts.take_from_sender();

        rental_extension::install(&mut kiosk, &kiosk_cap, ts.ctx());

        ts::return_shared(kiosk);
        ts.return_to_sender(kiosk_cap);
}
}
