// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use iota_sdk_move_types::iota_framework::account::AuthenticatorFunctionRefV1Key;
use iota_sdk_types::Identifier;

pub const ACCOUNT_MODULE_NAME: Identifier = Identifier::from_static("account");
pub const AUTHENTICATOR_FUNCTION_REF_V1_KEY_STRUCT_NAME: Identifier =
    Identifier::from_static("AuthenticatorFunctionRefV1Key");
