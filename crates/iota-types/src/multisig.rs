// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    hash::{Hash, Hasher},
    str::FromStr,
    sync::Arc,
};
use std::sync::Arc;

pub use enum_dispatch::enum_dispatch;
use fastcrypto::{
    // ed25519::Ed25519PublicKey,
    error::FastCryptoError,
    hash::HashFunction,
    // secp256k1::Secp256k1PublicKey,
    // secp256r1::Secp256r1PublicKey,
    traits::{EncodeDecodeBase64, ToFromBytes, VerifyingKey},
};
use iota_sdk_crypto::{Verifier, ed25519::Ed25519VerifyingKey, multisig::MultisigVerifier};
pub use iota_sdk_types::crypto::{
    BitmapUnit, MultisigAggregatedSignature as MultiSig, MultisigCommittee as MultiSigPublicKey,
    MultisigMember, MultisigMemberSignature, ThresholdUnit, WeightUnit,
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    base_types::IotaAddress,
    crypto::{CompressedSignature, DefaultHash, PublicKey, SignatureScheme},
    error::IotaError,
    passkey_authenticator::PasskeyAuthenticator,
    signature::{AuthenticatorTrait, GenericSignature, VerifyParams},
use iota_sdk_types::{SignatureScheme as SkdSignatureScheme, crypto::IntentMessage};
use serde::Serialize;

use crate::{
    base_types::{EpochId, IotaAddress},
    crypto::{CompressedSignature, DefaultHash, SignatureScheme},
    digests::ZKLoginInputsDigest,
    error::IotaError,
    signature::{AuthenticatorTrait, VerifyParams},
    signature_verification::VerifiedDigestCache,
    zk_login_authenticator::ZkLoginAuthenticator,
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

        if IotaAddress::from(self.committee()) != multisig_address {
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
        for (signature, i) in self.signatures().iter().zip(as_indices(self.bitmap())?) {
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
                        member.public_key().encode_base64(),
                        IotaAddress::from(member.public_key())
                    ),
                });
            }

            let res = match signature {
                 // MultisigMemberSignature::Passkey(bytes) => {
                //     let authenticator =
                //         PasskeyAuthenticator::from_bytes(&bytes.0).map_err(|_| {
                //             IotaError::InvalidSignature {
                //                 error: "Invalid passkey authenticator bytes".to_string(),
                //             }
                //         })?;
                //     authenticator
                //         .verify_claims(
                //             value,
                //             IotaAddress::from(subsig_pubkey),
                //             verify_params,
                //             zklogin_inputs_cache.clone(),
                //         )
                //         .map_err(|e| FastCryptoError::GeneralError(e.to_string()))
                // }
                _ => verifier
                    .verify_member_signature(&digest, member.public_key(), signature)
                    .unwrap(),
            };

            if res.is_ok() {
                weight_sum += member.weight() as u16;
            } else {
                return res.map_err(|e| IotaError::InvalidSignature {
                    error: format!(
                        "Invalid sig for pk={} address={:?} error={:?}",
                        subsig_pubkey.encode_base64(),
                        IotaAddress::from(subsig_pubkey),
                        e.to_string()
                    ),
                });
            }
        }

        if weight_sum >= self.committee().threshold() {
            Ok(())
        } else {
            Err(IotaError::InvalidSignature {
                error: format!(
                    "Insufficient weight={:?} threshold={:?}",
                    weight_sum,
                    self.committee().threshold()
                ),
            })
        }
    }
}

/// Interpret a bitmap of 01s as a list of indices that is set to 1s.
/// e.g. 22 = 0b10110, then the result is [1, 2, 4].
pub fn as_indices(bitmap: u16) -> Result<Vec<u8>, IotaError> {
    if bitmap > MAX_BITMAP_VALUE {
        return Err(IotaError::InvalidSignature {
            error: "Invalid bitmap".to_string(),
        });
    }
    let mut res = Vec::new();
    for i in 0..10 {
        if bitmap & (1 << i) != 0 {
            res.push(i as u8);
        }
    }
    Ok(res)
}

// pub fn get_indices(&self) -> Result<Vec<u8>, IotaError> {
//     as_indices(self.bitmap)
// }
// }

// impl FromStr for MultiSig {
//     type Err = IotaError;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         let bytes = Base64::decode(s).map_err(|_| IotaError::InvalidSignature
// {             error: "Invalid base64 string".to_string(),
//         })?;
//         let sig = MultiSig::from_bytes(&bytes).map_err(|_|
// IotaError::InvalidSignature {             error: "Invalid multisig
// bytes".to_string(),         })?;
//         Ok(sig)
//     }
// }

// /// The struct that contains the public key used for authenticating a
// MultiSig. #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize,
// JsonSchema)] pub struct MultiSigPublicKey {
//     /// A list of public key and its corresponding weight.
//     pk_map: Vec<(PublicKey, WeightUnit)>,
//     /// If the total weight of the public keys corresponding to verified
//     /// signatures is larger than threshold, the MultiSig is verified.
//     threshold: ThresholdUnit,
// }
