// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use enum_dispatch::enum_dispatch;
use fastcrypto::{hash::HashFunction, traits::ToFromBytes};
use iota_sdk_crypto::multisig::MultisigVerifier;
pub use iota_sdk_types::crypto::{
    BitmapUnit, MultisigAggregatedSignature as MultiSig, MultisigCommittee as MultiSigPublicKey,
    MultisigMember, MultisigMemberSignature, ThresholdUnit, WeightUnit,
};
use iota_sdk_types::{SignatureScheme as SkdSignatureScheme, crypto::IntentMessage};
use serde::Serialize;

use crate::{
    base_types::IotaAddress,
    crypto::DefaultHash,
    error::IotaError,
    passkey_authenticator::PasskeyAuthenticator,
    signature::{AuthenticatorTrait, VerifyParams},
};

#[cfg(test)]
#[path = "unit_tests/multisig_tests.rs"]
mod multisig_tests;

pub const MAX_SIGNER_IN_MULTISIG: usize = 10;
pub const MAX_BITMAP_VALUE: BitmapUnit = 0b1111111111;

// /// The struct that contains signatures and public keys necessary for
// /// authenticating a MultiSig.
// #[serde_as]
// #[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
// pub struct MultiSig {
//     /// The plain signature encoded with signature scheme.
//     sigs: Vec<CompressedSignature>,
//     /// A bitmap that indicates the position of which public key the
// signature     /// should be authenticated with.
//     bitmap: BitmapUnit,
//     /// The public key encoded with each public key with its signature scheme
//     /// used along with the corresponding weight.
//     multisig_pk: MultiSigPublicKey,
//     /// A bytes representation of [struct MultiSig]. This helps with
//     /// implementing [trait AsRef<[u8]>].
//     #[serde(skip)]
//     bytes: OnceCell<Vec<u8>>,
// }

impl AuthenticatorTrait for MultiSig {
    fn verify_claims<T>(
        &self,
        value: &IntentMessage<T>,
        multisig_address: IotaAddress,
        verify_params: &VerifyParams,
    ) -> Result<(), IotaError>
    where
        T: Serialize,
    {
        if !self.committee().is_valid() {
            return Err(IotaError::InvalidSignature {
                error: "Invalid multisig pubkey".to_string(),
            });
        }

        if self.committee().derive_address() != multisig_address {
            return Err(IotaError::InvalidSignature {
                error: "Invalid address derived from pks".to_string(),
            });
        }

        if self.has_scheme_signatures(SkdSignatureScheme::PasskeyAuthenticator)
            && !verify_params.accept_passkey_in_multisig
        {
            return Err(IotaError::InvalidSignature {
                error: "Passkey sig not supported inside multisig".to_string(),
            });
        }

        let mut weight_sum: u16 = 0;
        let message = bcs::to_bytes(&value).expect("Message serialization should not fail");
        let mut hasher = DefaultHash::default();
        hasher.update(message);
        let digest = hasher.finalize().digest;
        let verifier = MultisigVerifier::new();

        // Verify each signature against its corresponding signature scheme and public
        // key. TODO: further optimization can be done because multiple Ed25519
        // signatures can be batch verified.
        // TODO unwrap
        for (signature, i) in self.signatures().iter().zip(self.get_indices().unwrap()) {
            // let (subsig_pubkey, weight) =
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
                    // TODO conversion
                    let authenticator = PasskeyAuthenticator::from_bytes(&auth.to_bytes())
                        .map_err(|_| IotaError::InvalidSignature {
                            error: "Invalid passkey authenticator bytes".to_string(),
                        })?;
                    authenticator
                        .verify_claims(value, IotaAddress::from(member.public_key()), verify_params)
                        .map_err(|e| {
                            fastcrypto::error::FastCryptoError::GeneralError(e.to_string())
                        })
                }
                _ => verifier
                    .verify_member_signature(&digest, member.public_key(), signature)
                    // TODO not sure about these map_err
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
                weight_sum += member.weight() as u16
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

// impl MultiSig {

// /// The struct that contains the public key used for authenticating a
// MultiSig. #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize,
// JsonSchema)] pub struct MultiSigPublicKey {
//     /// A list of public key and its corresponding weight.
//     pk_map: Vec<(PublicKey, WeightUnit)>,
//     /// If the total weight of the public keys corresponding to verified
//     /// signatures is larger than threshold, the MultiSig is verified.
//     threshold: ThresholdUnit,
// }
