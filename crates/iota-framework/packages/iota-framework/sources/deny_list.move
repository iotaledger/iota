// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Defines the `DenyList` type. The `DenyList` shared object is used to restrict access to
/// instances of certain core types from being used as inputs by specified addresses in the deny
/// list.
module iota::deny_list;

use iota::bag::{Self, Bag};
use iota::config::{Self, Config};
use iota::dynamic_object_field as ofield;

/// Trying to create a deny list object when not called by the system address.
const ENotSystemAddress: u64 = 0;

/// A shared object that stores the addresses that are blocked for a given core type.
public struct DenyList has key {
    id: UID,
    /// The individual deny lists.
    lists: Bag,
}

/// The capability used to write to the deny list config. Ensures that the Configs for the
/// DenyList are modified only by this module.
public struct ConfigWriteCap() has drop;

/// The dynamic object field key used to store the `Config` for a given type, essentially a
/// `(per_type_index, per_type_key)` pair.
public struct ConfigKey has copy, drop, store {
    per_type_index: u64,
    per_type_key: vector<u8>,
}

/// The setting key used to store the deny list for a given address in the `Config`.
public struct AddressKey(address) has copy, drop, store;

/// The setting key used to store the global pause setting in the `Config`.
public struct GlobalPauseKey() has copy, drop, store;

/// The event emitted when a new `Config` is created for a given type. This can be useful for
/// tracking the `ID` of a type's `Config` object.
public struct PerTypeConfigCreated has copy, drop, store {
    key: ConfigKey,
    config_id: ID,
}

public(package) fun add(
    deny_list: &mut DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
    addr: address,
    ctx: &mut TxContext,
) {
    let per_type_config = deny_list.per_type_config_entry!(per_type_index, per_type_key, ctx);
    let setting_name = AddressKey(addr);
    let next_epoch_entry = per_type_config.entry!<_, AddressKey, bool>(
        &mut ConfigWriteCap(),
        setting_name,
        |_deny_list, _cap, _ctx| true,
        ctx,
    );
    *next_epoch_entry = true;
}

public(package) fun remove(
    deny_list: &mut DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
    addr: address,
    ctx: &mut TxContext,
) {
    let per_type_config = deny_list.per_type_config_entry!(per_type_index, per_type_key, ctx);
    let setting_name = AddressKey(addr);
    per_type_config.remove_for_next_epoch<_, AddressKey, bool>(
        &mut ConfigWriteCap(),
        setting_name,
        ctx,
    );
}

public(package) fun contains_current_epoch(
    deny_list: &DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
    addr: address,
    ctx: &TxContext,
): bool {
    if (!deny_list.per_type_exists(per_type_index, per_type_key)) return false;
    let per_type_config = deny_list.borrow_per_type_config(per_type_index, per_type_key);
    let setting_name = AddressKey(addr);
    config::read_setting(object::id(per_type_config), setting_name, ctx).destroy_or!(false)
}

public(package) fun contains_next_epoch(
    deny_list: &DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
    addr: address,
): bool {
    if (!deny_list.per_type_exists(per_type_index, per_type_key)) return false;
    let per_type_config = deny_list.borrow_per_type_config(per_type_index, per_type_key);
    let setting_name = AddressKey(addr);
    per_type_config.read_setting_for_next_epoch(setting_name).destroy_or!(false)
}

public(package) fun enable_global_pause(
    deny_list: &mut DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
    ctx: &mut TxContext,
) {
    let per_type_config = deny_list.per_type_config_entry!(per_type_index, per_type_key, ctx);
    let setting_name = GlobalPauseKey();
    let next_epoch_entry = per_type_config.entry!<_, GlobalPauseKey, bool>(
        &mut ConfigWriteCap(),
        setting_name,
        |_deny_list, _cap, _ctx| true,
        ctx,
    );
    *next_epoch_entry = true;
}

public(package) fun disable_global_pause(
    deny_list: &mut DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
    ctx: &mut TxContext,
) {
    let per_type_config = deny_list.per_type_config_entry!(per_type_index, per_type_key, ctx);
    let setting_name = GlobalPauseKey();
    per_type_config.remove_for_next_epoch<_, GlobalPauseKey, bool>(
        &mut ConfigWriteCap(),
        setting_name,
        ctx,
    );
}

public(package) fun is_global_pause_enabled_current_epoch(
    deny_list: &DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
    ctx: &TxContext,
): bool {
    if (!deny_list.per_type_exists(per_type_index, per_type_key)) return false;
    let per_type_config = deny_list.borrow_per_type_config(per_type_index, per_type_key);
    let setting_name = GlobalPauseKey();
    config::read_setting(object::id(per_type_config), setting_name, ctx).destroy_or!(false)
}

public(package) fun is_global_pause_enabled_next_epoch(
    deny_list: &DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
): bool {
    if (!deny_list.per_type_exists(per_type_index, per_type_key)) return false;
    let per_type_config = deny_list.borrow_per_type_config(per_type_index, per_type_key);
    let setting_name = GlobalPauseKey();
    per_type_config.read_setting_for_next_epoch(setting_name).destroy_or!(false)
}

fun add_per_type_config(
    deny_list: &mut DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
    ctx: &mut TxContext,
) {
    let key = ConfigKey { per_type_index, per_type_key };
    let config = config::new(&mut ConfigWriteCap(), ctx);
    let config_id = object::id(&config);
    ofield::internal_add(&mut deny_list.id, key, config);
    iota::event::emit(PerTypeConfigCreated { key, config_id });
}

fun borrow_per_type_config_mut(
    deny_list: &mut DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
): &mut Config<ConfigWriteCap> {
    let key = ConfigKey { per_type_index, per_type_key };
    ofield::internal_borrow_mut(&mut deny_list.id, key)
}

fun borrow_per_type_config(
    deny_list: &DenyList,
    per_type_index: u64,
    per_type_key: vector<u8>,
): &Config<ConfigWriteCap> {
    let key = ConfigKey { per_type_index, per_type_key };
    ofield::internal_borrow(&deny_list.id, key)
}

fun per_type_exists(deny_list: &DenyList, per_type_index: u64, per_type_key: vector<u8>): bool {
    let key = ConfigKey { per_type_index, per_type_key };
    ofield::exists_(&deny_list.id, key)
}

macro fun per_type_config_entry(
    $deny_list: &mut DenyList,
    $per_type_index: u64,
    $per_type_key: vector<u8>,
    $ctx: &mut TxContext,
): &mut Config<ConfigWriteCap> {
    let deny_list = $deny_list;
    let per_type_index = $per_type_index;
    let per_type_key = $per_type_key;
    let ctx = $ctx;
    if (!deny_list.per_type_exists(per_type_index, per_type_key)) {
        deny_list.add_per_type_config(per_type_index, per_type_key, ctx);
    };
    deny_list.borrow_per_type_config_mut(per_type_index, per_type_key)
}

#[allow(unused_function)]
/// Creation of the deny list object is restricted to the system address
/// via a system transaction.
fun create(ctx: &mut TxContext) {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);

    let deny_list_object = DenyList {
        id: object::iota_deny_list_object_id(),
        lists: bag::new(ctx),
    };

    transfer::share_object(deny_list_object);
}

#[test_only]
public fun create_for_test(ctx: &mut TxContext) {
    create(ctx);
}
