// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use enum_dispatch::enum_dispatch;
use iota_sdk_crypto::{Verifier, multisig::MultisigVerifier};
use iota_sdk_types::crypto::IntentMessage;
pub use iota_sdk_types::crypto::{
    BitmapUnit, MultisigAggregatedSignature as MultiSig, MultisigCommittee as MultiSigPublicKey,
    MultisigMember, MultisigMemberSignature, ThresholdUnit, WeightUnit,
};
use serde::Serialize;

use crate::{
    base_types::IotaAddress,
    error::IotaError,
    signature::{AuthenticatorTrait, VerifyParams},
};

#[cfg(test)]
#[path = "unit_tests/multisig_tests.rs"]
mod multisig_tests;

impl AuthenticatorTrait for MultiSig {
    fn verify_claims<T>(
        &self,
        intent_message: &IntentMessage<T>,
        multisig_address: IotaAddress,
        verify_params: &VerifyParams,
    ) -> Result<(), IotaError>
    where
        T: Serialize,
    {
        let digest = intent_message.signing_digest();
        let verifier = MultisigVerifier::new()
            .with_address(multisig_address)
            .with_accept_passkey_in_multisig(verify_params.accept_passkey_in_multisig)
            .with_additional_multisig_checks(verify_params.additional_multisig_checks);

        verifier
            .verify(&*digest, self)
            .map_err(|e| IotaError::InvalidSignature {
                error: format!("Invalid multisig: {e}"),
            })
    }
}
