// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::kiosk_extensions_tests;

use iota::kiosk_extension as ext;
use iota::kiosk_test_utils as test;

/// The `Ext` witness to use for testing.
public struct Extension has drop {}

// === Default Behavior ===

#[test]
fun test_default_behavior() {
    let ctx = &mut test::ctx();
    let (mut kiosk, owner_cap) = test::get_kiosk(ctx);

    ext::add(Extension {}, &mut kiosk, &owner_cap, 3, ctx);

    assert!(ext::is_installed<Extension>(&kiosk));
    assert!(ext::is_enabled<Extension>(&kiosk));
    assert!(ext::can_place<Extension>(&kiosk));
    assert!(ext::can_lock<Extension>(&kiosk));

    ext::disable<Extension>(&mut kiosk, &owner_cap);

    assert!(ext::is_installed<Extension>(&kiosk));
    assert!(!ext::is_enabled<Extension>(&kiosk));
    assert!(!ext::can_place<Extension>(&kiosk));
    assert!(!ext::can_lock<Extension>(&kiosk));

    ext::enable<Extension>(&mut kiosk, &owner_cap);

    assert!(ext::is_installed<Extension>(&kiosk));
    assert!(ext::is_enabled<Extension>(&kiosk));
    assert!(ext::can_place<Extension>(&kiosk));
    assert!(ext::can_lock<Extension>(&kiosk));

    test::return_kiosk(kiosk, owner_cap, ctx);
}

// === EExtensionNotAllowed ===

// methods tested:
// - `ext::place` (not allowed | only lock)
// - `ext::lock` (not allowed | only place)

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotAllowed)]
fun test_lock_not_allowed() {
    let ctx = &mut test::ctx();
    let (policy, _policy_cap) = test::get_policy(ctx);
    let (asset, _asset_id) = test::get_asset(ctx);
    let (mut kiosk, owner_cap) = test::get_kiosk(ctx);

    ext::add(Extension {}, &mut kiosk, &owner_cap, 0, ctx);
    ext::lock(Extension {}, &mut kiosk, asset, &policy);

    abort 1337
}

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotAllowed)]
fun test_lock_not_allowed_but_place() {
    let ctx = &mut test::ctx();
    let (policy, _policy_cap) = test::get_policy(ctx);
    let (asset, _asset_id) = test::get_asset(ctx);
    let (mut kiosk, owner_cap) = test::get_kiosk(ctx);

    ext::add(Extension {}, &mut kiosk, &owner_cap, 1, ctx);
    ext::lock(Extension {}, &mut kiosk, asset, &policy);

    abort 1337
}

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotAllowed)]
fun test_place_not_allowed() {
    let ctx = &mut test::ctx();
    let (policy, _policy_cap) = test::get_policy(ctx);
    let (asset, _asset_id) = test::get_asset(ctx);
    let (mut kiosk, owner_cap) = test::get_kiosk(ctx);

    ext::add(Extension {}, &mut kiosk, &owner_cap, 0, ctx);
    ext::place(Extension {}, &mut kiosk, asset, &policy);

    abort 1337
}

#[test]
fun test_place_allowed_with_lock() {
    let ctx = &mut test::ctx();
    let (policy, policy_cap) = test::get_policy(ctx);
    let (asset, asset_id) = test::get_asset(ctx);
    let (mut kiosk, owner_cap) = test::get_kiosk(ctx);

    ext::add(Extension {}, &mut kiosk, &owner_cap, 2, ctx);
    ext::place(Extension {}, &mut kiosk, asset, &policy);

    let asset = kiosk.take(&owner_cap, asset_id);

    test::return_kiosk(kiosk, owner_cap, ctx);
    test::return_policy(policy, policy_cap, ctx);
    test::return_assets(vector[asset]);
}

// === EExtensionNotInstalled ===

// methods tested:
// - `ext::remove`
// - `ext::storage`
// - `ext::storage_mut`
// - `ext::enable`
// - `ext::disable`
// - `ext::lock`
// - `ext::place`

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotInstalled)]
fun test_enable_not_installed() {
    let ctx = &mut test::ctx();
    let (mut kiosk, owner_cap) = test::get_kiosk(ctx);

    ext::enable<Extension>(&mut kiosk, &owner_cap);

    abort 1337
}

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotInstalled)]
fun test_disable_not_installed() {
    let ctx = &mut test::ctx();
    let (mut kiosk, owner_cap) = test::get_kiosk(ctx);

    ext::disable<Extension>(&mut kiosk, &owner_cap);

    abort 1337
}

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotInstalled)]
fun test_remove_not_installed() {
    let ctx = &mut test::ctx();
    let (mut kiosk, owner_cap) = test::get_kiosk(ctx);

    ext::remove<Extension>(&mut kiosk, &owner_cap);

    abort 1337
}

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotInstalled)]
fun test_storage_not_installed() {
    let ctx = &mut test::ctx();
    let (kiosk, _owner_cap) = test::get_kiosk(ctx);

    let _ = ext::storage(Extension {}, &kiosk);

    abort 1337
}

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotInstalled)]
fun test_storage_mut_not_installed() {
    let ctx = &mut test::ctx();
    let (mut kiosk, _owner_cap) = test::get_kiosk(ctx);

    let _ = ext::storage_mut(Extension {}, &mut kiosk);

    abort 1337
}

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotInstalled)]
fun test_lock_not_installed() {
    let ctx = &mut test::ctx();
    let (policy, _policy_cap) = test::get_policy(ctx);
    let (asset, _asset_id) = test::get_asset(ctx);
    let (mut kiosk, _owner_cap) = test::get_kiosk(ctx);

    ext::lock(Extension {}, &mut kiosk, asset, &policy);

    abort 1337
}

#[test, expected_failure(abort_code = iota::kiosk_extension::EExtensionNotInstalled)]
fun test_place_not_installed() {
    let ctx = &mut test::ctx();
    let (policy, _policy_cap) = test::get_policy(ctx);
    let (asset, _asset_id) = test::get_asset(ctx);
    let (mut kiosk, _owner_cap) = test::get_kiosk(ctx);

    ext::place(Extension {}, &mut kiosk, asset, &policy);

    abort 1337
}
