/// An allowlist/denylist rule for the VOUCHER system
module clt_tutorial::allowlist_rule {
    use iota::bag::{Self, Bag};
    use iota::token::{
        Self,
        TokenPolicy,
        TokenPolicyCap,
        ActionRequest
    };

    /// There is no conofiguration for the rule.
    const ENoConfig: u64 = 0;

    /// The sender is not a shop for returning/spending the voucher.
    const ESenderNotAShop: u64 = 1;

    /// The sender is a shop trying to transfer the voucher.
    const ESenderIsAShop: u64 = 2;

    /// The recipient is not a shop for receiving the voucher.
    const ERecipientNotAShop: u64 = 3;

    /// The Rule witness.
    public struct Allowlist has drop {}

    /// Verifes that only shops can receive and return vouchers.
    ///
    /// Aborts if:
    ///     - The policy does not have a configuration.
    ///     - A shop tries to use a voucher
    ///     - A non-shop tries to return the voucher
    public fun verify<T>(
        policy: &TokenPolicy<T>,
        request: &mut ActionRequest<T>,
        ctx: &mut TxContext
    ) {
        assert!(has_config(policy), ENoConfig);

        let config = config(policy);
        let sender = token::sender(request);
        let recipient = token::recipient(request);

        if (request.action()==token::spend_action()) {
            // Sender needs to be a shop
            assert!(bag::contains(config, sender), ESenderNotAShop);
        } else if (request.action()==token::transfer_action()) {
            // The sender can't be a shop
            assert!(!bag::contains(config, sender), ESenderIsAShop);

            // The recipient has to be a shop
            let recipient = *option::borrow(&recipient);
            assert!(bag::contains(config, recipient), ERecipientNotAShop);
        };

        token::add_approval(Allowlist {}, request, ctx);
    }

    // === Protected: List Management ===

    /// Adds addresses to the `allowlist_rule` for a given action.
    public fun add_addresses<T>(
        policy: &mut TokenPolicy<T>,
        cap: &TokenPolicyCap<T>,
        mut addresses: vector<address>,
        ctx: &mut TxContext,
    ) {
        if (!has_config(policy)) {
            token::add_rule_config(Allowlist {}, policy, cap, bag::new(ctx), ctx);
        };

        let config_mut = config_mut(policy, cap);
        while (vector::length(&addresses) > 0) {
            bag::add(config_mut, vector::pop_back(&mut addresses), true)
        }
    }

    /// Removes addresses from the `allowlist_rule` for a given action.
    public fun remove_addresses<T>(
        policy: &mut TokenPolicy<T>,
        cap: &TokenPolicyCap<T>,
        mut addresses: vector<address>,
    ) {
        let config_mut = config_mut(policy, cap);

        while (vector::length(&addresses) > 0) {
            let record = vector::pop_back(&mut addresses);
            let _: bool = bag::remove(config_mut, record);
        };
    }

    // === Internal ===

    fun has_config<T>(self: &TokenPolicy<T>): bool {
        token::has_rule_config_with_type<T, Allowlist, Bag>(self)
    }

    fun config<T>(self: &TokenPolicy<T>): &Bag {
        token::rule_config<T, Allowlist, Bag>(Allowlist {}, self)
    }

    fun config_mut<T>(self: &mut TokenPolicy<T>, cap: &TokenPolicyCap<T>): &mut Bag {
        token::rule_config_mut(Allowlist {}, self, cap)
    }
}