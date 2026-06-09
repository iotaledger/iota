// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// using a regulated coin in an authenticator

//# init --addresses test_coin=0x0 test_account=0x0 simple_abstract_account=0x0 --accounts A C

//# publish --sender C
module test_coin::regulated_coin {
    use iota::coin;

    public struct REGULATED_COIN has drop {}

    fun init(otw: REGULATED_COIN, ctx: &mut TxContext) {
        let (mut treasury_cap, deny_cap, metadata) = coin::create_regulated_currency_v1(
            otw,
            9,
            b"RC",
            b"REGULATED_COIN",
            b"A new regulated coin",
            option::none(),
            false,
            ctx,
        );
        let coin = coin::mint(&mut treasury_cap, 10000, ctx);
        transfer::public_share_object(coin);
        transfer::public_transfer(deny_cap, tx_context::sender(ctx));
        transfer::public_freeze_object(treasury_cap);
        transfer::public_freeze_object(metadata);
    }
}

// a `REGULATED_COIN` shared instance that will be used as an authenticator input
//# view-object 1,0

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies test_coin simple_abstract_account
module test_account::authenticate {
    use iota::coin::Coin;
    use simple_abstract_account::abstract_account::AbstractAccount;
    use test_coin::regulated_coin::REGULATED_COIN;

    #[authenticator]
    public fun authenticate(
        _account: &AbstractAccount,
        _denied: &Coin<REGULATED_COIN>,
        _auth_ctx: &AuthContext,
        _ctx: &TxContext,
    ) {}
}

//# init-abstract-account --sender A --package-metadata object(5,1) --inputs "authenticate" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# set-address account_addr object(6,2)

// use a `REGULATED_COIN` coin as an authenticator input, which is allowed
//# abstract --account immshared(6,2) --auth-inputs immshared(1,0) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

// deny `account_addr` from using `REGULATED_COIN` coins
//# run iota::coin::deny_list_v1_add --args object(0x403) object(1,2) @account_addr --type-args test_coin::regulated_coin::REGULATED_COIN --sender C

// attempt to use a `REGULATED_COIN` instance as an authenticator input, which is denied
//# abstract --account immshared(6,2) --auth-inputs immshared(1,0) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

// allow `account_addr` using `REGULATED_COIN` coins
//# run iota::coin::deny_list_v1_remove --args object(0x403) object(1,2) @account_addr --type-args test_coin::regulated_coin::REGULATED_COIN --sender C

// use a `REGULATED_COIN` coin as an authenticator input, which is allowed
//# abstract --account immshared(6,2) --auth-inputs immshared(1,0) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));
