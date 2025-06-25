// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module account_abstraction::account_abstraction;

use iota::balance::{Self, Balance};
use iota::coin::{Self, Coin};
use iota::iota::IOTA;
use iota::transfer::Receiving;

const EInvalidAccountOwner: u64 = 0;
const EInvalidOwnerAddr: u64 = 1;
const ENotEnoughBalance: u64 = 2;

public struct OwnerCap has key, store { id: UID, owner_addr: address }

public struct SmartAccount has key {
    id: UID,
    balance: Balance<IOTA>,
    owner_cap_id: ID,
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

// Initializes a smart account and issues OwnerCap to multisig address
public fun init_multisig_smart_account(multisig_owner: address, ctx: &mut TxContext) {
    let uid = object::new(ctx);
    let id = *uid.as_inner();
    let owner_cap = OwnerCap { id: uid, owner_addr: multisig_owner };

    let smart_account = SmartAccount {
        id: object::new(ctx),
        balance: balance::zero<IOTA>(),
        owner_cap_id: id,
    };

    transfer::public_transfer(owner_cap, multisig_owner);
    transfer::share_object(smart_account);
}

// Delete a smart account and realted OwnerCap and returns remained balance as coin.
public fun delete_multisig_smart_account(
    smart_account: SmartAccount,
    owner_cap: OwnerCap,
    ctx: &mut TxContext,
): Coin<IOTA> {
    assert!(owner_cap.id.as_inner() == smart_account.owner_cap_id, EInvalidAccountOwner);
    let SmartAccount { id: sm_id, balance, owner_cap_id: _ } = smart_account;
    let OwnerCap { id: owner_cap_id, owner_addr: _ } = owner_cap;
    sm_id.delete();
    owner_cap_id.delete();
    balance.into_coin(ctx)
}

// Receive the tokens that were sent to the smart account.
public fun receive_deposit(smart_account: &mut SmartAccount, coin: Receiving<Coin<IOTA>>) {
    let coin = transfer::public_receive(&mut smart_account.id, coin);
    coin::put(&mut smart_account.balance, coin);
}

public fun withdraw(
    smart_account: &mut SmartAccount,
    owner_cap: &OwnerCap,
    amount: u64,
    ctx: &mut TxContext,
): Coin<IOTA> {
    assert!(owner_cap.id.as_inner() == smart_account.owner_cap_id, EInvalidAccountOwner);
    assert!(ctx.sender() == owner_cap.owner_addr, EInvalidOwnerAddr);
    assert!(amount <= smart_account.balance.value(), ENotEnoughBalance);
    coin::take(&mut smart_account.balance, amount, ctx)
}
