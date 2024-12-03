
#[test_only]
module nft_marketplace::marketplace_extension_tests {

    use nft_marketplace::test_utils::{create_kiosk, create_transfer_policy};
    use kiosk::royalty_rule as royalty_rule;
    use nft_marketplace::marketplace_extension::Self;
    use iota::{
        iota::IOTA,
        coin::Coin,
        kiosk::Kiosk,
        kiosk_test_utils,
        package::Self,
        test_scenario::{Self as ts, Scenario},
        transfer_policy::{TransferPolicy, TransferPolicyCap}
    };

    const CREATOR: address = @0xCCCC;
    const SELLER: address = @0xAAAA;
    const BUYER: address = @0xBBBB;

    public struct T has key, store { id: UID }
    public struct WITNESS has drop {}


    // ==================== Happy path scenarios ====================

    #[test]
    fun test_buy_item_without_royalties() {
        let mut ts = ts::begin(SELLER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let seller_kiosk_id = create_kiosk(SELLER, ts.ctx());
        let item_price = 50000;

        create_transfer_policy<T>( CREATOR, &publisher, ts.ctx());
        install_ext(&mut ts, SELLER, seller_kiosk_id);
        setup_price<T>(&mut ts, SELLER, seller_kiosk_id, item, item_price);
        let payment = kiosk_test_utils::get_iota(item_price, ts.ctx());
        buy<T>(&mut ts, BUYER, seller_kiosk_id, item_id, payment);

        publisher.burn_publisher();
        ts.end();
    }

    #[test]
    fun test_buy_item_with_royalties() {
        let mut ts = ts::begin(SELLER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let seller_kiosk_id = create_kiosk(SELLER, ts.ctx());
        let item_price = 50000;
        let royalty_amount_bp = 5000;
        let royalty_min_amount = 2000;
        create_transfer_policy<T>( CREATOR, &publisher, ts.ctx());
        add_royalty_rule(&mut ts, CREATOR, royalty_amount_bp, royalty_min_amount);
        install_ext(&mut ts, SELLER, seller_kiosk_id);
        setup_price<T>(&mut ts, SELLER, seller_kiosk_id, item, item_price);
        let mut payment = kiosk_test_utils::get_iota(item_price, ts.ctx());
        let royalty_amount_to_pay = get_royalty_fee_amount(&ts, item_price);
        let royalties_coin = kiosk_test_utils::get_iota(royalty_amount_to_pay, ts.ctx());
        payment.join(royalties_coin);
        buy<T>(&mut ts, BUYER, seller_kiosk_id, item_id, payment);

        publisher.burn_publisher();
        ts.end();
    }

    #[test]
    fun test_get_item_price() {
        let mut ts = ts::begin(SELLER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let seller_kiosk_id = create_kiosk(SELLER, ts.ctx());
        let item_price = 50000;

        install_ext(&mut ts, SELLER, seller_kiosk_id);
        setup_price<T>(&mut ts, SELLER, seller_kiosk_id, item, item_price);
        ts.next_tx(SELLER);
        let kiosk: Kiosk = ts.take_shared_by_id(seller_kiosk_id);
        let storage_item_price = marketplace_extension::get_item_price<T>(&kiosk, item_id);
      
        assert!(storage_item_price == item_price);

        ts::return_shared(kiosk);
        ts.end();
    }

    // ==================== Negative scenarios ====================

    #[test]
    #[expected_failure(abort_code = marketplace_extension::EWrongPaymentRoyalties)]
    fun test_buy_item_with_royalties_wrong_royalties_amount() {
        let mut ts = ts::begin(SELLER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let seller_kiosk_id = create_kiosk(SELLER, ts.ctx());
        let item_price = 50000;
        let royalty_amount_bp = 5000;
        let royalty_min_amount = 2000;
        create_transfer_policy<T>( CREATOR, &publisher, ts.ctx());
        add_royalty_rule(&mut ts, CREATOR, royalty_amount_bp, royalty_min_amount);
        install_ext(&mut ts, SELLER, seller_kiosk_id);
        setup_price<T>(&mut ts, SELLER, seller_kiosk_id, item, item_price);
        let mut payment = kiosk_test_utils::get_iota(item_price, ts.ctx());
        let royalty_amount_to_pay = get_royalty_fee_amount(&ts, 1000);
        let royalties_coin = kiosk_test_utils::get_iota(royalty_amount_to_pay, ts.ctx());
        payment.join(royalties_coin);
        buy<T>(&mut ts, BUYER, seller_kiosk_id, item_id, payment);

        publisher.burn_publisher();
        ts.end();
    }

    #[test]
    #[expected_failure(abort_code = marketplace_extension::ENotEnoughPaymentAmount)]
    fun test_buy_item_without_royalties_wrong_price() {
        let mut ts = ts::begin(SELLER);

        let item = T { id: object::new(ts.ctx()) };
        let item_id = object::id(&item);

        let witness = WITNESS {};
        let publisher = package::test_claim(witness, ts.ctx());

        let seller_kiosk_id = create_kiosk(SELLER, ts.ctx());
        let item_price = 50000;

        create_transfer_policy<T>( CREATOR, &publisher, ts.ctx());
        install_ext(&mut ts, SELLER, seller_kiosk_id);
        setup_price<T>(&mut ts, SELLER, seller_kiosk_id, item, item_price);
        let payment = kiosk_test_utils::get_iota(40000, ts.ctx());
        buy<T>(&mut ts, BUYER, seller_kiosk_id, item_id, payment);

        publisher.burn_publisher();
        ts.end();
    }

    #[test]
    #[expected_failure(abort_code = marketplace_extension::EExtensionNotInstalled)]
    fun test_set_price_without_extension() {
        let mut ts = ts::begin(SELLER);

        let item = T { id: object::new(ts.ctx()) };

        let seller_kiosk_id = create_kiosk(SELLER, ts.ctx());
        let item_price = 50000;

        setup_price<T>(&mut ts, SELLER, seller_kiosk_id, item, item_price);

        ts.end();
    }
    // ==================== Helper methods ====================


    fun setup_price<T: key + store>(ts: &mut Scenario, sender: address, seller_kiosk_id: ID, item: T, price: u64) {
        ts.next_tx(sender);
        let mut kiosk: Kiosk = ts.take_shared_by_id(seller_kiosk_id);
        let kiosk_cap = ts.take_from_sender();

        marketplace_extension::set_price<T>(&mut kiosk, &kiosk_cap, item, price);

        ts::return_shared(kiosk);
        ts.return_to_sender(kiosk_cap);
    }

    fun install_ext(ts: &mut Scenario, sender: address, kiosk_id: ID) {
        ts.next_tx(sender);
        let mut kiosk: Kiosk = ts.take_shared_by_id(kiosk_id);
        let kiosk_cap = ts.take_from_sender();

        marketplace_extension::install(&mut kiosk, &kiosk_cap, ts.ctx());

        ts::return_shared(kiosk);
        ts.return_to_sender(kiosk_cap);
    }

    fun buy<T: key + store>(ts: &mut Scenario, buyer: address, seller_kiosk_id: ID, item_id: ID, payment: Coin<IOTA>) {
        ts.next_tx(buyer);
        let mut kiosk: Kiosk = ts.take_shared_by_id(seller_kiosk_id);
        let mut policy: TransferPolicy<T> = ts.take_shared();

        let item = marketplace_extension::buy_item<T>(&mut kiosk, &mut policy, item_id, payment, ts.ctx());
        transfer::public_transfer<T>(item, buyer);
        ts::return_shared(kiosk);
        ts::return_shared(policy);
     }

    fun add_royalty_rule(ts: &mut Scenario, sender: address, amount_bp: u16, min_amount: u64) {
        ts.next_tx(sender);
        let mut transfer_policy: TransferPolicy<T> = ts.take_shared();
        let policy_cap: TransferPolicyCap<T> = ts.take_from_sender();

        marketplace_extension::setup_royalties(&mut transfer_policy, &policy_cap, amount_bp, min_amount);

        ts::return_shared(transfer_policy);
        ts.return_to_sender(policy_cap);
    }

    fun get_royalty_fee_amount(ts: &Scenario, price: u64): u64 {
        let transfer_policy: TransferPolicy<T> = ts.take_shared();
        let royalty_fee = royalty_rule::fee_amount(&transfer_policy, price);
        ts::return_shared(transfer_policy);
        royalty_fee
    }
}