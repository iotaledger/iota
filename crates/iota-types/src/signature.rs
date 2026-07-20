// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use iota_sdk_types::UserSignature;
use iota_sdk_types::{Address, crypto::IntentMessage};
use serde::Serialize;

use crate::error::IotaResult;

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

/// This ports the wrapper trait to the verify_secure defined on
/// [`crate::crypto::Signature`].
impl AuthenticatorTrait for crate::crypto::Signature {
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
        use crate::crypto::IotaSignature;
        self.verify_secure(value, author, self.signature_scheme())
    }
}
