// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use enum_dispatch::enum_dispatch;
use fastcrypto::traits::ToFromBytes;
use iota_sdk_crypto::multisig::MultisigVerifier;
pub use iota_sdk_types::crypto::{
    BitmapUnit, MultisigAggregatedSignature as MultiSig, MultisigCommittee as MultiSigPublicKey,
    MultisigMember, MultisigMemberSignature, ThresholdUnit, WeightUnit,
};
use iota_sdk_types::{SignatureScheme, crypto::IntentMessage};
use serde::Serialize;

use crate::{
    base_types::IotaAddress,
    error::IotaError,
    passkey_authenticator::PasskeyAuthenticator,
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
        self.validate().map_err(|e| IotaError::InvalidSignature {
            error: format!("Invalid multisig: {e}"),
        })?;

        if self.committee().derive_address() != multisig_address {
            return Err(IotaError::InvalidSignature {
                error: "Invalid address derived from pks".to_string(),
            });
        }

        if self.has_scheme_signatures(SignatureScheme::PasskeyAuthenticator)
            && !verify_params.accept_passkey_in_multisig
        {
            return Err(IotaError::InvalidSignature {
                error: "Passkey sig not supported inside multisig".to_string(),
            });
        }

        let mut weight_sum: ThresholdUnit = 0;
        let digest = intent_message.signing_digest();
        let verifier = MultisigVerifier::new();

        // Verify each signature against its corresponding signature scheme and public
        // key. TODO: further optimization can be done because multiple Ed25519
        // signatures can be batch verified.
        // TODO we can unwrap depending if we validate the whole multisig or not
        let indices = self.get_indices().unwrap();
        for (signature, i) in self.signatures().iter().zip(indices) {
            let member =
                self.committee()
                    .members()
                    .get(i as usize)
                    .ok_or(IotaError::InvalidSignature {
                        error: "Invalid public keys index".to_string(),
                    })?;

            if verify_params.additional_multisig_checks
                && member.public_key().scheme() != signature.scheme()
            {
                return Err(IotaError::InvalidSignature {
                    error: format!(
                        "Invalid sig for pk={} address={:?} error=signature/pubkey type mismatch",
                        member.public_key().to_base64(),
                        member.public_key().derive_address(),
                    ),
                });
            }

            let res = match signature {
                MultisigMemberSignature::Passkey(auth) => {
                    // TODO https://github.com/iotaledger/iota/issues/11607
                    let authenticator = PasskeyAuthenticator::from_bytes(&auth.to_bytes())
                        .map_err(|_| IotaError::InvalidSignature {
                            error: "Invalid passkey authenticator bytes".to_string(),
                        })?;
                    authenticator
                        .verify_claims(
                            intent_message,
                            IotaAddress::from(member.public_key()),
                            verify_params,
                        )
                        .map_err(|e| {
                            fastcrypto::error::FastCryptoError::GeneralError(e.to_string())
                        })
                }
                _ => verifier
                    .verify_member_signature(&*digest, member.public_key(), signature)
                    .map_err(|e| fastcrypto::error::FastCryptoError::GeneralError(e.to_string())),
            };

            if let Err(e) = res {
                return Err(IotaError::InvalidSignature {
                    error: format!(
                        "Invalid sig for pk={} address={:?} error={e:?}",
                        member.public_key().to_base64(),
                        member.public_key().derive_address(),
                    ),
                });
            } else {
                weight_sum = weight_sum
                    .checked_add(member.weight() as ThresholdUnit)
                    .ok_or(IotaError::InvalidSignature {
                        error: "Weight overflow".to_string(),
                    })?;
            }
        }

        if weight_sum >= self.committee().threshold() {
            Ok(())
        } else {
            Err(IotaError::InvalidSignature {
                error: format!(
                    "Insufficient weight={weight_sum:?} threshold={:?}",
                    self.committee().threshold()
                ),
            })
        }
    }
}
