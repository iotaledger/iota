// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_crypto::{Verifier, passkey::PasskeyVerifier};
use iota_sdk_types::crypto::IntentMessage;
pub use iota_sdk_types::crypto::PasskeyAuthenticator;
use serde::Serialize;

use crate::{
    base_types::IotaAddress,
    error::{IotaError, IotaResult},
    signature::{AuthenticatorTrait, VerifyParams},
};

#[cfg(test)]
#[path = "unit_tests/passkey_authenticator_test.rs"]
mod passkey_authenticator_test;

impl AuthenticatorTrait for PasskeyAuthenticator {
    /// Verify an intent message of a transaction with an passkey authenticator.
    fn verify_claims<T>(
        &self,
        intent_msg: &IntentMessage<T>,
        author: IotaAddress,
        _aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize,
    {
        let digest = intent_msg.signing_digest();

        PasskeyVerifier::new()
            .with_address(author)
            .verify(&*digest, self)
            .map_err(|e| IotaError::InvalidSignature {
                error: format!("Invalid passkey authentication: {e}"),
            })
    }
}
