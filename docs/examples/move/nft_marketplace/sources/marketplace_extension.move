module nft_marketplace::marketplace_extension {
    // iota imports
    use iota::{
        kiosk::{Kiosk, KioskOwnerCap, purchase},
        kiosk_extension,
        bag,
        transfer_policy::{Self, TransferPolicy, TransferPolicyCap, has_rule},
        coin::Coin,
        iota::IOTA,
    };

    // rules imports
    use kiosk::royalty_rule::Rule as RoyaltyRule;
    use kiosk::royalty_rule;


    // === Errors ===
    const EExtensionNotInstalled: u64 = 0;
    const EWrongPaymentRoyalties: u64 = 1;
    const ENotEnoughPaymentAmount: u64 = 2;

    // === Constants ===
    const ALLOW_PLACE_AND_LOCK: u128 = 11;

    /// Extension Key for Kiosk Marketplace extension.
    public struct Marketplace has drop {}

    /// Used as a key for the item that has been up for sale that's placed in the Extension's Bag.
    public struct Listed has store, copy, drop { id: ID }
    
    public struct ItemPrice<phantom T: key + store> has store {
        /// Total amount of time offered for renting in days.
        price: u64,
    }

    // === Public Functions ===
    
    /// Enables someone to install the Marketplace extension in their Kiosk.
    public fun install(
        kiosk: &mut Kiosk,
        cap: &KioskOwnerCap,
        ctx: &mut TxContext,
    ) {
        kiosk_extension::add(Marketplace {}, kiosk, cap, ALLOW_PLACE_AND_LOCK, ctx);
    }

    /// Remove the extension from the Kiosk. Can only be performed by the owner,
    /// The extension storage must be empty for the transaction to succeed.
    public fun remove(kiosk: &mut Kiosk, cap: &KioskOwnerCap, _ctx: &mut TxContext) {
        kiosk_extension::remove<Marketplace>(kiosk, cap);
    }

    /// Setup item royalty percentage
    /// - amount_bp - the percentage of the purchase price to be paid as a
    /// fee, denominated in basis points (100_00 = 100%, 1 = 0.01%).
    /// - min_amount - the minimum amount to be paid as a fee if the relative
    /// amount is lower than this setting.
    public fun setup_royalties<T: key + store>(policy: &mut TransferPolicy<T>, cap: &TransferPolicyCap<T>, amount_bp: u16, min_amount: u64) {
        royalty_rule::add<T>(policy, cap, amount_bp, min_amount);
    }

    /// Buy listed item with the indicated price and pay royalties if needed
    public fun buy_item<T: key + store>(kiosk: &mut Kiosk, policy: &mut TransferPolicy<T>, item_id: object::ID, mut payment: Coin<IOTA>, ctx: &mut TxContext): T {
        assert!(kiosk_extension::is_installed<Marketplace>(kiosk), EExtensionNotInstalled);

        // Get item price
        let ItemPrice { price } = take_from_bag<T, Listed>(kiosk,  Listed { id: item_id });

        // Compute the value of the coin in input
        let payment_amount_value = payment.value();

        // If the payment_amount_value is less than the item price, the request is invalid.
        assert!(payment_amount_value >= price, ENotEnoughPaymentAmount);

        // Prepare the payment coin for the purchase (if no royalties are present then the
        // remaining balance will be 0 after this operation)
        let coin_price = payment.split(price, ctx);

        // Purchase and create the transfer request
        let (item, mut transfer_request) = purchase(kiosk, item_id, coin_price);

        // If the royalty is present, then update the request with a royalty payment
        if (policy.has_rule<T, RoyaltyRule>()) { 
            let royalties_value = royalty_rule::fee_amount(policy, price);
            assert!(payment_amount_value == price + royalties_value, EWrongPaymentRoyalties);
            royalty_rule::pay(policy, &mut transfer_request, payment);
        } else {
            // Else clean the input coin (if the input payment amount is not exact, this will fail)
            payment.destroy_zero();
        };

        // Confirm the request
        transfer_policy::confirm_request(policy, transfer_request);
        item
    }


    public fun set_price<T: key + store>(
        kiosk: &mut Kiosk,
        cap: &KioskOwnerCap,
        item: T,
        price: u64) {
        assert!(kiosk_extension::is_installed<Marketplace>(kiosk), EExtensionNotInstalled);

        let id = object::id(&item);
        kiosk.place_and_list<T>(cap, item, price);

        let item_price = ItemPrice {
            price,
        };

        place_in_bag<T, Listed>(kiosk, Listed { id }, item_price);
    }


    public fun get_item_price<T: key + store>(
        kiosk: &Kiosk,
        item_id: ID,
    ) : u64 {
        let storage_ref = kiosk_extension::storage(Marketplace {}, kiosk);
        let ItemPrice { price } = bag::borrow<Listed, ItemPrice<T>>(
            storage_ref,
            Listed { id: item_id },
        );

        *price
    }


    // === Private Functions ===

    fun take_from_bag<T: key + store, Key: store + copy + drop>(
        kiosk: &mut Kiosk,
        item_key: Key,
    ) : ItemPrice<T> {
        let ext_storage_mut = kiosk_extension::storage_mut(Marketplace {}, kiosk);
        bag::remove<Key, ItemPrice<T>>(
            ext_storage_mut,
            item_key,
        )
    }

    fun place_in_bag<T: key + store, Key: store + copy + drop>(
        kiosk: &mut Kiosk,
        item_key: Key,
        item_price: ItemPrice<T>,
    ) {
        let ext_storage_mut = kiosk_extension::storage_mut(Marketplace {}, kiosk);
        bag::add(ext_storage_mut, item_key, item_price);
    }
}