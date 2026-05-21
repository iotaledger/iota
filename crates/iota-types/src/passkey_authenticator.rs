// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::hash::{Hash, Hasher};

use fastcrypto::{
    error::FastCryptoError,
    hash::HashFunction,
    rsa::{Base64UrlUnpadded, Encoding},
    secp256r1::{Secp256r1PublicKey, Secp256r1Signature},
    traits::ToFromBytes,
};
use iota_sdk_crypto::{Verifier, passkey::PasskeyVerifier};
use iota_sdk_types::crypto::IntentMessage;
pub use iota_sdk_types::crypto::PasskeyAuthenticator;
use once_cell::sync::OnceCell;
use passkey_types::webauthn::{ClientDataType, CollectedClientData};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    base_types::IotaAddress,
    crypto::{
        DefaultHash, IotaSignature, IotaSignatureInner, PublicKey, Secp256r1IotaSignature,
        Signature, SignatureScheme,
    },
    error::{IotaError, IotaResult},
    signature::{AuthenticatorTrait, VerifyParams},
};

#[cfg(test)]
#[path = "unit_tests/passkey_authenticator_test.rs"]
mod passkey_authenticator_test;

pub trait PasskeyAuthenticatorExt {
    fn new_for_testing(
        authenticator_data: Vec<u8>,
        client_data_json: String,
        user_signature: Signature,
    ) -> Result<PasskeyAuthenticator, IotaError>;
}

impl PasskeyAuthenticatorExt for PasskeyAuthenticator {
    /// A constructor for [struct PasskeyAuthenticator] with custom defined
    /// fields. Used for testing.
    fn new_for_testing(
        authenticator_data: Vec<u8>,
        client_data_json: String,
        user_signature: Signature,
    ) -> Result<Self, IotaError> {
        let raw = RawPasskeyAuthenticator {
            authenticator_data,
            client_data_json,
            user_signature,
        };
        raw.try_into()
    }
}

// /// Convert [struct RawPasskeyAuthenticator] to [struct PasskeyAuthenticator]
// /// with validations.
// impl TryFrom<RawPasskeyAuthenticator> for PasskeyAuthenticator {
//     type Error = IotaError;

//     fn try_from(raw: RawPasskeyAuthenticator) -> Result<Self, Self::Error> {
//         let client_data_json_parsed: CollectedClientData =
//             serde_json::from_str(&raw.client_data_json).map_err(|_| {
//                 IotaError::InvalidSignature {
//                     error: "Invalid client data json".to_string(),
//                 }
//             })?;

//         if client_data_json_parsed.ty != ClientDataType::Get {
//             return Err(IotaError::InvalidSignature {
//                 error: "Invalid client data type".to_string(),
//             });
//         };

//         let challenge =
// Base64UrlUnpadded::decode_vec(&client_data_json_parsed.challenge)
// .map_err(|_| IotaError::InvalidSignature {                 error: "Invalid
// encoded challenge".to_string(),             })?
//             .try_into()
//             .map_err(|_| IotaError::InvalidSignature {
//                 error: "Invalid size for challenge".to_string(),
//             })?;

//         if raw.user_signature.scheme() != SignatureScheme::Secp256r1 {
//             return Err(IotaError::InvalidSignature {
//                 error: "Invalid signature scheme".to_string(),
//             });
//         };

//         let pk =
// Secp256r1PublicKey::from_bytes(raw.user_signature.public_key_bytes()).
// map_err(             |_| IotaError::InvalidSignature {
//                 error: "Invalid r1 pk".to_string(),
//             },
//         )?;

//         let signature =
// Secp256r1Signature::from_bytes(raw.user_signature.signature_bytes())
//             .map_err(|_| IotaError::InvalidSignature {
//                 error: "Invalid r1 sig".to_string(),
//             })?;

//         Ok(PasskeyAuthenticator {
//             authenticator_data: raw.authenticator_data,
//             client_data_json: raw.client_data_json,
//             signature,
//             pk,
//             challenge,
//             bytes: OnceCell::new(),
//         })
//     }
// }

// impl<'de> Deserialize<'de> for PasskeyAuthenticator {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         use serde::de::Error;

//         let serializable =
// RawPasskeyAuthenticator::deserialize(deserializer)?;         serializable
//             .try_into()
//             .map_err(|e: IotaError| Error::custom(e.to_string()))
//     }
// }

// impl Serialize for PasskeyAuthenticator {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::ser::Serializer,
//     {
//         let mut bytes = Vec::with_capacity(Secp256r1IotaSignature::LENGTH);
//         bytes.push(SignatureScheme::Secp256r1.flag());
//         bytes.extend_from_slice(self.signature.as_ref());
//         bytes.extend_from_slice(self.pk.as_ref());

//         let raw = RawPasskeyAuthenticator {
//             authenticator_data: self.authenticator_data.clone(),
//             client_data_json: self.client_data_json.clone(),
//             user_signature: Signature::Secp256r1IotaSignature(
//                 Secp256r1IotaSignature::from_bytes(&bytes).unwrap(), /* ok to
// unwrap since the
//                                                                       * bytes
//                                                                         are constructed
//                                                                         as
//                                                                       * valid
//                                                                         above.
//                                                                         */
//             ),
//         };
//         raw.serialize(serializer)
//     }
// }

// impl PasskeyAuthenticator {

//     /// Returns the public key of the passkey authenticator.
//     pub fn get_pk(&self) -> IotaResult<PublicKey> {
//         Ok(PublicKey::Passkey((&self.pk).into()))
//     }

//     pub fn authenticator_data(&self) -> &[u8] {
//         &self.authenticator_data
//     }

//     pub fn client_data_json(&self) -> &str {
//         &self.client_data_json
//     }

//     pub fn signature(&self) -> Signature {
//         let mut bytes = Vec::with_capacity(Secp256r1IotaSignature::LENGTH);
//         bytes.push(SignatureScheme::Secp256r1.flag());
//         bytes.extend_from_slice(self.signature.as_ref());
//         bytes.extend_from_slice(self.pk.as_ref());

//         // Safe to unwrap because signature and pk are serialized from valid
// struct.
//         Signature::Secp256r1IotaSignature(Secp256r1IotaSignature::from_bytes(&bytes).unwrap())
//     }
// }

// /// Necessary trait for [struct SenderSignedData].
// impl PartialEq for PasskeyAuthenticator {
//     fn eq(&self, other: &Self) -> bool {
//         self.as_ref() == other.as_ref()
//     }
// }

// /// Necessary trait for [struct SenderSignedData].
// impl Eq for PasskeyAuthenticator {}

// /// Necessary trait for [struct SenderSignedData].
// impl Hash for PasskeyAuthenticator {
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         self.as_ref().hash(state);
//     }
// }

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

// impl ToFromBytes for PasskeyAuthenticator {
//     fn from_bytes(bytes: &[u8]) -> Result<Self, FastCryptoError> {
//         // The first byte matches the flag of PasskeyAuthenticator.
//         if bytes.first().ok_or(FastCryptoError::InvalidInput)?
//             != &SignatureScheme::PasskeyAuthenticator.flag()
//         {
//             return Err(FastCryptoError::InvalidInput);
//         }
//         let passkey: PasskeyAuthenticator =
//             bcs::from_bytes(&bytes[1..]).map_err(|_|
// FastCryptoError::InvalidSignature)?;         Ok(passkey)
//     }
// }

// impl AsRef<[u8]> for PasskeyAuthenticator {
//     fn as_ref(&self) -> &[u8] {
//         self.bytes
//             .get_or_try_init::<_, eyre::Report>(|| {
//                 let as_bytes = bcs::to_bytes(self).expect("BCS serialization
// should not fail");                 let mut bytes = Vec::with_capacity(1 +
// as_bytes.len());
// bytes.push(SignatureScheme::PasskeyAuthenticator.flag());
// bytes.extend_from_slice(as_bytes.as_slice());                 Ok(bytes)
//             })
//             .expect("OnceCell invariant violated")
//     }
// }
