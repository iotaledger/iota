module nft_marketplace::nft_marketplace {
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
    // use kiosk::floor_price_rule::Rule as FloorPriceRule;
    // use kiosk::personal_kiosk_rule::Rule as PersonalRule;
    use kiosk::royalty_rule::Rule as RoyaltyRule;
    use kiosk::royalty_rule;
    // use kiosk::witness_rule::Rule as WitnessRule;


    // === Errors ===
    const EExtensionNotInstalled: u64 = 0;
    const EObjectNotExist: u64 = 1;

    // === Constants ===
    const PERMISSIONS: u128 = 11;

    /// Extension Key for Kiosk Marketplace extension.
    public struct Marketplace has drop {}

    /// Used as a key for the item that has been up for sale that's placed in the Extension's Bag.
    public struct Listed has store, copy, drop { id: ID }
    
    public struct ItemPrice<T: key + store> has store {
        /// Total amount of time offered for renting in days.
        price: u64,
    }

    /// Enables someone to install the Marketplace extension in their Kiosk.
    public fun install(
        kiosk: &mut Kiosk,
        cap: &KioskOwnerCap,
        ctx: &mut TxContext,
    ) {
        kiosk_extension::add(Marketplace {}, kiosk, cap, PERMISSIONS, ctx);
    }

    /// Remove the extension from the Kiosk. Can only be performed by the owner,
    /// The extension storage must be empty for the transaction to succeed.
    public fun remove(kiosk: &mut Kiosk, cap: &KioskOwnerCap, _ctx: &mut TxContext) {
        kiosk_extension::remove<Marketplace>(kiosk, cap);
    }

    public fun setup_royalties<T: key + store>(policy: &mut TransferPolicy<T>, cap: &TransferPolicyCap<T>, amount_bp: u16, min_amount: u64, ctx: &mut TxContext) {
        royalty_rule::add<T>(policy, cap, amount_bp, min_amount);
    }

    /// Buy listed item and pay royalties if needed
    public fun buy_item<T: key + store>(kiosk: &mut Kiosk, policy: &mut TransferPolicy<T>, item_id: object::ID, mut payment: Coin<IOTA>, ctx: &mut TxContext) {
        assert!(kiosk_extension::is_installed<Marketplace>(kiosk), EExtensionNotInstalled);
        let item_price = take_from_bag<T, Listed>(kiosk,  Listed { id: item_id });
        let ItemPrice { price } = item_price;
        let payment_amount = payment.split(price, ctx);
        let payment_amount_value = payment_amount.value();
        let (item, mut transfer_request) = purchase(kiosk, item_id, payment_amount);
        if (policy.has_rule<T, RoyaltyRule>()) { 
            let royalties_value = royalty_rule::fee_amount(policy, payment_amount_value);
            let royalties_coin = payment.split(royalties_value, ctx);
            royalty_rule::pay(policy, &mut transfer_request, royalties_coin);
        };
        transfer_policy::confirm_request(policy, transfer_request);
        transfer::public_transfer<T>(item, ctx.sender());
        // Send a leftover back to buyer
        transfer::public_transfer(payment, ctx.sender());
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


    // === Private Functions ===

    fun take_from_bag<T: key + store, Key: store + copy + drop>(
        kiosk: &mut Kiosk,
        item_key: Key,
    ) : ItemPrice<T> {
        let ext_storage_mut = kiosk_extension::storage_mut(Marketplace {}, kiosk);
        assert!(bag::contains(ext_storage_mut, item_key), EObjectNotExist);
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