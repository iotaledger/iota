// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_sdk_crypto::{Verifier, simple::SimpleVerifier};
use iota_sdk_types::{
    Address, SigningDigest, UserSignature,
    crypto::{IntentMessage, SimpleSignature},
};
use serde::Serialize;

use crate::error::{IotaError, IotaResult};

#[derive(Debug, Clone)]
pub struct VerifyParams {
    pub accept_passkey_in_multisig: bool,
    pub additional_multisig_checks: bool,
    pub reject_partially_signed_mixed_ed25519_multisig: bool,
}

impl VerifyParams {
    pub fn new(
        accept_passkey_in_multisig: bool,
        additional_multisig_checks: bool,
        reject_partially_signed_mixed_ed25519_multisig: bool,
    ) -> Self {
        Self {
            accept_passkey_in_multisig,
            additional_multisig_checks,
            reject_partially_signed_mixed_ed25519_multisig,
        }
    }

    /// The parameters the network applies at `config`'s protocol version.
    pub fn from_protocol_config(config: &ProtocolConfig) -> Self {
        Self::new(
            config.accept_passkey_in_multisig(),
            config.additional_multisig_checks(),
            config.reject_partially_signed_mixed_ed25519_multisig(),
        )
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

    /// Like [`Self::verify_claims`], for the message whose signing digest is
    /// `digest`.
    ///
    /// Returns an error unless the authenticator checks a signature of the
    /// signing digest.
    fn verify_signing_digest(
        &self,
        _digest: &SigningDigest,
        _author: Address,
        _aux_verify_data: &VerifyParams,
    ) -> IotaResult {
        Err(IotaError::InvalidSignature {
            error: "The authenticator does not check a signature of a signing digest".into(),
        })
    }
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

    fn verify_signing_digest(
        &self,
        digest: &SigningDigest,
        author: Address,
        aux_verify_data: &VerifyParams,
    ) -> IotaResult {
        match self {
            UserSignature::Simple(s) => s.verify_signing_digest(digest, author, aux_verify_data),
            UserSignature::Multisig(s) => s.verify_signing_digest(digest, author, aux_verify_data),
            UserSignature::PasskeyAuthenticator(s) => {
                s.verify_signing_digest(digest, author, aux_verify_data)
            }
            UserSignature::MoveAuthenticator(s) => {
                s.verify_signing_digest(digest, author, aux_verify_data)
            }
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
        aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize,
    {
        self.verify_signing_digest(&value.signing_digest(), author, aux_verify_data)
    }

    fn verify_signing_digest(
        &self,
        digest: &SigningDigest,
        author: Address,
        _aux_verify_data: &VerifyParams,
    ) -> IotaResult {
        // `SimpleVerifier` only checks the signature against its embedded public
        // key, so the signer/author binding is enforced here.
        let address: Address = self.to_public_key().into();
        if author != address {
            return Err(IotaError::IncorrectSigner {
                error: format!("Incorrect signer, expected {author}, got {address}"),
            });
        }

        SimpleVerifier
            .verify(digest, self)
            .map_err(|e| IotaError::InvalidSignature {
                error: format!("Fail to verify user sig {e}"),
            })
    }
}
