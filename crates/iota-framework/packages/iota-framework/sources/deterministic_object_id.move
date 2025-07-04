// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::deterministic_object_id;
use iota::account_registry::{AccountRegistry};
use iota::object::{new_uid_from_hash};

/// u64 flag used to produce precomputed object IDs.
const PRECOMPUTED_OBJECT_ID_FLAG: u64 = 123;

/// Native function for deriving an ID deterministically via hash(flag, iota_address, salt)
/// flag is a fixed byte that allows to avoid object id collisions
/// iota_address can be any IOTA address, not necessarily the sender address
/// salt is an arbitrary value provided by the sender 
public native fun derive_id_with_salt(
    flag: u64,
    iota_address: address,
    salt: vector<u8>,
): address;

#[test_only]
public fun dummy_address(): address {
    @0x0
}

/// Create a new object with a precomputed objectId.
/// Using a fixed 123 to avoid collisions
public fun new_precomputed(iota_address: address, salt: vector<u8>, account_registry: &mut AccountRegistry): UID {
    let id = derive_id_with_salt(PRECOMPUTED_OBJECT_ID_FLAG,iota_address, salt);
    account_registry.add(id.to_id());
    new_uid_from_hash(id)
}

// -------------------------------- Smart Account basic flow for test purpose (PTB) ---------------------------------------------------
//

public struct SmartAccount has key {
    id: UID,
    balance: u64,
}

// Initializion a mocked smart account
public fun init_smart_account(addr: address, account_registry: &mut AccountRegistry, _ctx: &mut TxContext) {
    let salt = vector[0x12, 0x34, 0xab, 0xcd]; // for prototype purpose salt is hardcoded here
    let smart_account = SmartAccount {
        id: new_precomputed(addr, salt,  account_registry),
        balance: 0,
    };
    transfer::share_object(smart_account);
}

// Deletion a mocked smart account
public fun delete_smart_account(
    smart_account: SmartAccount,
    account_registry: &mut AccountRegistry,
    _ctx: &mut TxContext,
) {
    let SmartAccount { id: sm_id, .. } = smart_account;
    account_registry.remove(*sm_id.as_inner());
    sm_id.delete();
}