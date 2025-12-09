// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// using a regulated coin in an authenticator

//# init --addresses test_coin=0x0 test_account=0x0 --accounts A C

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

//# publish --sender A --dependencies test_coin
module test_account::abstract_account {
    use iota::account::{Self, AuthenticatorInfoV1};
    use iota::auth_context::AuthContext;
    use iota::coin::Coin;
    use test_coin::regulated_coin::REGULATED_COIN;

    public struct AbstractAccount has key {
        id: UID,
    }

    public fun create(
        authenticator: AuthenticatorInfoV1<AbstractAccount>,
        ctx: &mut TxContext,
    ): address {
        let account = AbstractAccount { id: object::new(ctx) };

        let account_address = object::id_address(&account);

        account::create_account_v1(account, authenticator);

        account_address
    }

    #[authenticator]
    public fun authenticate(
        _account: &AbstractAccount,
        _denied: &Coin<REGULATED_COIN>,
        _auth_ctx: &AuthContext,
        _ctx: &TxContext,
    ) {}
}

//# programmable --sender A --inputs object(3,1) "abstract_account" "authenticate" 7000000000
//> 0: iota::account::create_auth_info_v1<test_account::abstract_account::AbstractAccount>(Input(0), Input(1), Input(2));
//> 1: test_account::abstract_account::create(Result(0));
//> 2: SplitCoins(Gas, [Input(3)]);
//> 3: TransferObjects([Result(2)], Result(1));

// account-owned coin used for gas payment
//# view-object 4,0

//# set-address account_addr object(4,2)

// use a `REGULATED_COIN` coin as an authenticator input, which is allowed
//# abstract --account immshared(4,2) --gas-payment 4,0 --auth-inputs immshared(1,0) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

// deny `account_addr` from using `REGULATED_COIN` coins
//# run iota::coin::deny_list_v1_add --args object(0x403) object(1,2) @account_addr --type-args test_coin::regulated_coin::REGULATED_COIN --sender C

// attempt to use a `REGULATED_COIN` instance as an authenticator input, which is denied
//# abstract --account immshared(4,2) --gas-payment 4,0 --auth-inputs immshared(1,0) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

// allow `account_addr` using `REGULATED_COIN` coins
//# run iota::coin::deny_list_v1_remove --args object(0x403) object(1,2) @account_addr --type-args test_coin::regulated_coin::REGULATED_COIN --sender C

// use a `REGULATED_COIN` coin as an authenticator input, which is allowed
//# abstract --account immshared(4,2) --gas-payment 4,0 --auth-inputs immshared(1,0) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));
