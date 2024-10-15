// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// This test verifies double adding an address to the deny list does not panic and still
// ensures the correct behavior when removing

//# init --accounts A B --addresses test=0x0

//# publish --sender A
module test::regulated_coin {
    use iota::coin;
    use iota::deny_list::DenyList;

    public struct REGULATED_COIN has drop {}

    fun init(otw: REGULATED_COIN, ctx: &mut TxContext) {
        let (mut treasury_cap, deny_cap, metadata) = coin::create_regulated_currency_v2(
            otw,
            9,
            b"RC",
            b"REGULATED_COIN",
            b"A new regulated coin",
            option::none(),
            false,
            ctx
        );
        let coin = coin::mint(&mut treasury_cap, 10000, ctx);
        transfer::public_transfer(coin, tx_context::sender(ctx));
        transfer::public_transfer(deny_cap, tx_context::sender(ctx));
        transfer::public_freeze_object(treasury_cap);
        transfer::public_freeze_object(metadata);
    }

    entry fun assert_address_deny_status(
        deny_list: &DenyList,
        addr: address,
        expected: bool,
    ) {
        let status = coin::deny_list_with_config_key_v1_contains_next_epoch<REGULATED_COIN>(deny_list, addr);
        assert!(status == expected, 0);
    }
}

// Transfer away the newly minted coin works normally.
//# run iota::pay::split_and_transfer --args object(1,0) 1 @B --type-args test::regulated_coin::REGULATED_COIN --sender A

// Deny account B.
//# run iota::coin::deny_list_with_config_key_v1_add --args object(0x403) object(1,2) @B --type-args test::regulated_coin::REGULATED_COIN --sender A

// Deny account B a second time. This should not change anything.
//# run iota::coin::deny_list_with_config_key_v1_add --args object(0x403) object(1,2) @B --type-args test::regulated_coin::REGULATED_COIN --sender A

// Assert that the address is denied.
//# run test::regulated_coin::assert_address_deny_status --args immshared(0x403) @B true --sender A

// Try transfer the coin from B. This should now be denied.
//# transfer-object 2,0 --sender B --recipient A

// Try using the coin in a Move call. This should also be denied.
//# run iota::pay::split_and_transfer --args object(2,0) 1 @A --type-args test::regulated_coin::REGULATED_COIN --sender B

// Undeny account B.
//# run iota::coin::deny_list_with_config_key_v1_remove --args object(0x403) object(1,2) @B --type-args test::regulated_coin::REGULATED_COIN --sender A

// Assert that the address is no longer denied.
//# run test::regulated_coin::assert_address_deny_status --args immshared(0x403) @B false --sender A

// This time the transfer should work.
//# transfer-object 2,0 --sender B --recipient A
