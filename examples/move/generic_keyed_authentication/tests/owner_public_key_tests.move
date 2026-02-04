// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module generic_keyed_authentication::owner_public_key_tests;

use generic_keyed_authentication::owner_public_key;
use iota::hex;
use iota::test_scenario;
use iota::test_utils;
use std::unit_test::assert_eq;
use iota::ecdsa_k1;

// ------------------------ Basic operations -----------------------------

#[test]
fun owner_public_key_handling() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    assert_eq!(owner_public_key::has(&id), false);
    owner_public_key::attach(&mut id, x"41");
    assert_eq!(owner_public_key::has(&id), true);
    assert_eq!(*owner_public_key::borrow(&id), x"41");

    owner_public_key::rotate(&mut id, x"43");
    assert_eq!(*owner_public_key::borrow(&id), x"43");

    owner_public_key::detach(&mut id);
    assert_eq!(owner_public_key::has(&id), false);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = owner_public_key::EPublicKeyAttached)]
fun duplicate_public_key_reported() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    owner_public_key::attach(&mut id, x"41");
    owner_public_key::attach(&mut id, x"41");

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = owner_public_key::EPublicKeyMissing)]
fun detach_public_key_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    owner_public_key::detach(&mut id);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = owner_public_key::EPublicKeyMissing)]
fun rotate_public_key_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    owner_public_key::rotate(&mut id, x"43");

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// ------------------------ Ed25519 -----------------------------

#[test]
#[expected_failure(abort_code = owner_public_key::EPublicKeyMissing)]
fun authenticate_ed25519_public_key_required() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    owner_public_key::authenticate_ed25519(&id, b"23", &b"33");

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = owner_public_key::EEd25519VerificationFailed)]
fun authenticate_ed25519_signature_mismatch() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    owner_public_key::attach(&mut id, b"33");
    owner_public_key::authenticate_ed25519(
        &id,
        hex::encode(b"42"),
        &b"invalid",
    );

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_ed25519_signature() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let signature = x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
    let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

    owner_public_key::attach(&mut id, public_key);
    owner_public_key::authenticate_ed25519(
        &id,
        hex::encode(signature),
        &digest,
    );

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// ------------------------ Secp256k1 -----------------------------

#[test]
#[expected_failure(abort_code = owner_public_key::EPublicKeyMissing)]
fun authenticate_secp256k1_public_key_required() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    owner_public_key::authenticate_secp256k1(&id, b"23", &b"33");

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = owner_public_key::ESecp256k1VerificationFailed)]
fun authenticate_secp256k1_signature_mismatch() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    owner_public_key::attach(&mut id, b"33");
    owner_public_key::authenticate_secp256k1(
        &id,
        hex::encode(b"42"),
        &b"invalid",
    );

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_secp256k1_signature() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    let public_key = x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
    let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
    let secret_key = x"42258dcda14cf111c602b8971b8cc843e91e46ca905151c02744a6b017e69316";
    let signature = ecdsa_k1::secp256k1_sign(&secret_key, &digest, 0, false);

    owner_public_key::attach(&mut id, public_key);
    owner_public_key::authenticate_secp256k1(
        &id,
        hex::encode(signature),
        &digest,
    );

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// ------------------------ Secp256r1 -----------------------------

#[test]
#[expected_failure(abort_code = owner_public_key::EPublicKeyMissing)]
fun authenticate_secp256r1_public_key_required() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    owner_public_key::authenticate_secp256r1(&id, b"23", &b"33");

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = owner_public_key::ESecp256r1VerificationFailed)]
fun authenticate_secp256r1_signature_mismatch() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    owner_public_key::attach(&mut id, b"33");
    owner_public_key::authenticate_secp256r1(
        &id,
        hex::encode(b"42"),
        &b"invalid",
    );

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_secp256r1_signature() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    let public_key = x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";
    let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
    let signature = x"310d0ab3a8870f6ab3d775f3cdf0a60059293e431f3ded9d1f6efe2c70f12da5628c7853ae18464b4d426d8ff6d31ae50fe31e47886b13733ba2aae508541bcd";

    owner_public_key::attach(&mut id, public_key);
    owner_public_key::authenticate_secp256r1(
        &id,
        hex::encode(signature),
        &digest,
    );

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// ------------------------------ Test utils -----------------------------
