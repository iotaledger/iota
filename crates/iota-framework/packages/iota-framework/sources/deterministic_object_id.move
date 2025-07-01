// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::deterministic_object_id;

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

