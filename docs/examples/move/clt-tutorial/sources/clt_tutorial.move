/// Module: clt_tutorial
module clt_tutorial::voucher {
    use iota::coin::{Self, TreasuryCap};
    use iota::token::{ActionRequest, Self, Token, TokenPolicy, TokenPolicyCap};

    use clt_tutorial::allowlist_rule;

    /// Token amount does not match the `LEDBULB_PRICE`.
    const EIncorrectAmount: u64 = 0;

    /// The price for the LED bulb.
    const LEDBULB_PRICE: u64 = 1;

    /// The OTW for the Token.
    public struct VOUCHER has drop {}

    /// The LedBulb object - can be purchased for 1 voucher.
    public  struct LedBulb has key, store {
        id: UID
    }

    // Create a new VOUCHER currency, create a `TokenPolicy` for it and allow
    // everyone to to spend their voucher for a led bulb.
    fun init(otw: VOUCHER, ctx: &mut TxContext) {
        let (treasury_cap, coin_metadata) = coin::create_currency(
            otw,
            0, // no decimals
            b"VCHR", // symbol
            b"Voucher Token", // name
            b"Token used for voucher clt tutorial", // description
            option::none(), // url
            ctx
        );

        let (mut policy, policy_cap) = token::new_policy(&treasury_cap, ctx);

        // Add allow_list rule for spend and transfer action
        policy.add_rule_for_action<VOUCHER, allowlist_rule::Allowlist>( &policy_cap, token::spend_action(), ctx);
        policy.add_rule_for_action<VOUCHER, allowlist_rule::Allowlist>( &policy_cap, token::transfer_action(), ctx);

        token::share_policy(policy);
        transfer::public_transfer(policy_cap, ctx.sender());
        transfer::public_freeze_object(coin_metadata);
        transfer::public_transfer(treasury_cap, ctx.sender());
    }

    /// Function to register shop addresses for rule validation
    public fun register_shop(
        policy: &mut TokenPolicy<VOUCHER>,
        cap: &TokenPolicyCap<VOUCHER>,
        addresses: vector<address>,
        ctx: &mut TxContext
    ) {
        allowlist_rule::add_addresses(policy, cap, addresses, ctx)
    }

    /// Function to gift voucher. Can be called by the application admin
    /// to gift vouchers to users
    ///
    /// `Mint` is available to the holder of the `TreasuryCap` by default and
    /// hence does not need to be confirmed; however, the `transfer` action
    /// does require a confirmation and can be confirmed with `TreasuryCap`.
    /// Keep in mind that confirming with `TreasuryCap` is ignoring the rules,
    /// hence why we are allowed to transfer to a non-shop address in this case.
    public fun gift_voucher(
        cap: &mut TreasuryCap<VOUCHER>,
        amount: u64,
        recipient: address,
        ctx: &mut TxContext
    ) {
        let token = token::mint(cap, amount, ctx);
        let req = token.transfer(recipient, ctx);

        token::confirm_with_treasury_cap(cap, req, ctx);
    }

    /// Buy a LED bulb using the voucher. The `LedBulb` is received, and the voucher is
    /// transferred to the shop address.
    public fun buy_led_bulb(
        token: Token<VOUCHER>,
        policy: &TokenPolicy<VOUCHER>,
        shop_address: address,
        ctx: &mut TxContext
    ): (LedBulb, ActionRequest<VOUCHER>) {
        assert!(token::value(&token) == LEDBULB_PRICE, EIncorrectAmount);

        let led_bulb = LedBulb { id: object::new(ctx) };
        let mut req = token.transfer( shop_address, ctx);

        allowlist_rule::verify(policy, &mut req, ctx);

        (led_bulb, req)
    }

    /// Function for shops to return the voucher by destroying it and adding it to the `policy` spent balance.
    /// Only shops can return the voucher.
    public fun return_voucher(
        token: Token<VOUCHER>,
        policy: &TokenPolicy<VOUCHER>,
        ctx: &mut TxContext
    ): ActionRequest<VOUCHER> {
        let mut action_request = token.spend(ctx);

        allowlist_rule::verify(policy, &mut action_request, ctx);

        action_request
    }

    // Only for testing purposes to run the init function in tests module
    #[test_only]
    public fun test_init(ctx: &mut TxContext) {
        init(VOUCHER {}, ctx)
    }
}
