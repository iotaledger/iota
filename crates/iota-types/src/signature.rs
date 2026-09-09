// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_crypto::{Verifier, simple::SimpleVerifier};
use iota_sdk_types::{
    Address, UserSignature,
    crypto::{IntentMessage, SimpleSignature},
};
use serde::Serialize;

use crate::error::{IotaError, IotaResult};

#[derive(Default, Debug, Clone)]
pub struct VerifyParams {
    pub accept_passkey_in_multisig: bool,
    pub additional_multisig_checks: bool,
}

impl VerifyParams {
    pub fn new(accept_passkey_in_multisig: bool, additional_multisig_checks: bool) -> Self {
        Self {
            accept_passkey_in_multisig,
            additional_multisig_checks,
        }
    }
}

/// A lightweight trait that all members of [`UserSignature`] implement.
pub trait AuthenticatorTrait {
    fn verify_claims<T>(
        &self,
        value: &IntentMessage<T>,
        author: Address,
        aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize;
}

impl AuthenticatorTrait for UserSignature {
    fn verify_claims<T>(
        &self,
        value: &IntentMessage<T>,
        author: Address,
        aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize,
    {
        match self {
            UserSignature::Simple(s) => s.verify_claims(value, author, aux_verify_data),
            UserSignature::Multisig(s) => s.verify_claims(value, author, aux_verify_data),
            UserSignature::PasskeyAuthenticator(s) => {
                s.verify_claims(value, author, aux_verify_data)
            }
            UserSignature::MoveAuthenticator(s) => s.verify_claims(value, author, aux_verify_data),
            _ => unimplemented!("a new UserSignature variant was added and needs to be handled"),
        }
    }
}

impl AuthenticatorTrait for SimpleSignature {
    #[tracing::instrument(level = "trace", skip_all)]
    fn verify_claims<T>(
        &self,
        value: &IntentMessage<T>,
        author: Address,
        _aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize,
    {
        // `SimpleVerifier` only checks the signature against its embedded public
        // key, so the signer/author binding is enforced here.
        let address: Address = self.to_public_key().into();
        if author != address {
            return Err(IotaError::IncorrectSigner {
                error: format!("Incorrect signer, expected {author}, got {address}"),
            });
        }

        SimpleVerifier
            .verify(&value.signing_digest(), self)
            .map_err(|e| IotaError::InvalidSignature {
                error: format!("Fail to verify user sig {e}"),
            })
    }
}
