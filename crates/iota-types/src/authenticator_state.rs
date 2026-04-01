// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Deprecated: Authenticator state (JWK) is no longer supported.
//! Struct definitions are retained for BCS deserialization compatibility
//! with historical transactions and genesis objects.

#![allow(deprecated)]

pub(crate) use fastcrypto_zkp::bn254::zk_login::{JWK, JwkId};
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};
use serde::{Deserialize, Serialize};

use crate::{IOTA_FRAMEWORK_ADDRESS, id::UID};

pub const AUTHENTICATOR_STATE_MODULE_NAME: &IdentStr = ident_str!("authenticator_state");
pub const AUTHENTICATOR_STATE_STRUCT_NAME: &IdentStr = ident_str!("AuthenticatorState");
pub const RESOLVED_IOTA_AUTHENTICATOR_STATE: (&AccountAddress, &IdentStr, &IdentStr) = (
    &IOTA_FRAMEWORK_ADDRESS,
    AUTHENTICATOR_STATE_MODULE_NAME,
    AUTHENTICATOR_STATE_STRUCT_NAME,
);

/// Current latest version of the authenticator state object.
pub const AUTHENTICATOR_STATE_VERSION: u64 = 1;

#[deprecated(note = "JWK/authenticator state is no longer supported")]
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticatorState {
    pub id: UID,
    pub version: u64,
}

#[deprecated(note = "JWK/authenticator state is no longer supported")]
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticatorStateInner {
    pub version: u64,

    /// List of currently active JWKs.
    pub active_jwks: Vec<ActiveJwk>,
}

#[deprecated(note = "JWK/authenticator state is no longer supported")]
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct ActiveJwk {
    pub jwk_id: JwkId,
    pub jwk: JWK,
    // the most recent epoch in which the jwk was validated
    pub epoch: u64,
}
