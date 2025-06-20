// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module account_abstraction::account_abstraction;

use iota::balance::{Self, Balance};
use iota::coin::{Self, Coin};
use iota::iota::IOTA;

const EInvalidAccountOwner: u64 = 0;
const EInvalidOwnerAddr: u64 = 1;
const ENotEnoughBalance: u64 = 2;

public struct OwnerCap has key, store { id: UID, smart_acc_id: ID, owner_addr: address }

public struct SmartAccount has key {
    id: UID,
    balance: Balance<IOTA>,
    //owner_cap: OwnerCap,
}

public(package) fun id_mut(smart_account: &mut SmartAccount): &mut UID {
    &mut smart_account.id
}

// public fun create_abstraction_account(owners: vector<address>, ctx: &mut TxContext) {
//     let uid = object::new(ctx);
//     let id = *uid.as_inner();
//     let smart_account = SmartAccount { id: uid, balance: balance::zero<IOTA>() };

//     let mut signer_count = 0;
//     while (signer_count != owners.length()) {
//         let owner_cap = OwnerCap { id: object::new(ctx), smart_acc_id: id, owner_addr: owners[signer_count]  };
//         transfer::public_transfer(owner_cap, owners[signer_count]);
//         signer_count = signer_count + 1;
//     };
//     transfer::share_object(smart_account);
// }

// Initializes a shared account and issues OwnerCap to multisig address
public fun init_multisig_aa(multisig_owner: address, ctx: &mut TxContext) {
    let uid = object::new(ctx);
    let id = *uid.as_inner();
    let smart_account = SmartAccount { id: uid, balance: balance::zero<IOTA>() };

    let owner_cap = OwnerCap { id: object::new(ctx), smart_acc_id: id, owner_addr: multisig_owner };
    transfer::public_transfer(owner_cap, multisig_owner);
    transfer::share_object(smart_account);
}

public fun deposit(smart_account: &mut SmartAccount, coin: Coin<IOTA>) {
    coin::put(&mut smart_account.balance, coin);
}

public fun withdraw(
    smart_account: &mut SmartAccount,
    owner_cap: &OwnerCap,
    amount: u64,
    recipient: address,
    ctx: &mut TxContext,
) {
    assert!(owner_cap.smart_acc_id == smart_account.id.as_inner(), EInvalidAccountOwner);
    assert!(ctx.sender() == owner_cap.owner_addr, EInvalidOwnerAddr);
    assert!(amount <= smart_account.balance.value(), ENotEnoughBalance);
    let coin = coin::take(&mut smart_account.balance, amount, ctx);
    transfer::public_transfer(coin, recipient);
}
