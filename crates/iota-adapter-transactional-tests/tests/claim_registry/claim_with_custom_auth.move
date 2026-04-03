// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// claim_with_auth: publish a package with a custom authenticator and attach it
// to a freshly-claimed IotaDefaultAccount via claim_registry::claim_with_auth.
// This verifies that a custom Move-based authenticator can be attached at claim
// time, and that the resulting IotaDefaultAccount is shared with the custom
// auth ref stored.
// Account A's deterministic Ed25519 public key in the transactional test runner:
//   7f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928
// Derived address (Blake2b256(pk)):
//   0x8cca4e1ce0ba5904cea61df9242da2f7d29e3ef328fb7ec07c086b3bf47ca61a

//# init --accounts A --addresses test=0x0

//# publish --sender A
module test::custom_auth;

use iota::iota_default_account::IotaDefaultAccount;

/// Always-allow custom authenticator for IotaDefaultAccount (testing only).
#[authenticator]
public fun authenticate(
    _account: &IotaDefaultAccount,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

//# programmable --sender A --inputs object(0x11) 0u8 x"7f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928" object(1,1) "custom_auth" "authenticate"
//> 0: iota::authenticator_function::create_auth_function_ref_v1<iota::iota_default_account::IotaDefaultAccount>(Input(3), Input(4), Input(5));
//> 1: iota::claim_registry::claim_with_auth(Input(0), Input(1), Input(2), Result(0));

//# view-object 2,2