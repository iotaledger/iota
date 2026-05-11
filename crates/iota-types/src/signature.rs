// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::hash::Hash;

pub use enum_dispatch::enum_dispatch;
use fastcrypto::{
    ed25519::{Ed25519PublicKey, Ed25519Signature},
    error::FastCryptoError,
    secp256k1::{Secp256k1PublicKey, Secp256k1Signature},
    secp256r1::{Secp256r1PublicKey, Secp256r1Signature},
    traits::{EncodeDecodeBase64, ToFromBytes},
};
use iota_sdk_types::crypto::IntentMessage;
use serde::Serialize;
use tracing::instrument;

use crate::{
    base_types::IotaAddress,
    crypto::{
        CompressedSignature, IotaSignature, PasskeyAuthenticatorAsBytes, PublicKey, Signature,
        SignatureScheme,
    },
    error::{IotaError, IotaResult},
    move_authenticator::{MoveAuthenticator, MoveAuthenticatorInner, MoveAuthenticatorV1},
    multisig::MultiSig,
    passkey_authenticator::PasskeyAuthenticator,
};
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

/// A lightweight trait that all members of [enum GenericSignature] implement.
#[enum_dispatch]
pub trait AuthenticatorTrait {
    fn verify_claims<T>(
        &self,
        value: &IntentMessage<T>,
        author: IotaAddress,
        aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize;
}

/// Deprecated zkLogin authenticator — empty stub retained only so the
/// [`GenericSignature::ZkLoginAuthenticatorDeprecated`] enum variant compiles.
/// Instances are never constructed; deserialization rejects the flag byte.
#[iota_proc_macros::allow_deprecated_for_derives]
#[deprecated(note = "zkLogin is deprecated and was never enabled on IOTA")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZkLoginAuthenticatorDeprecated;

#[allow(deprecated)]
impl AuthenticatorTrait for ZkLoginAuthenticatorDeprecated {
    fn verify_claims<T>(
        &self,
        _value: &IntentMessage<T>,
        _author: IotaAddress,
        _aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize,
    {
        Err(IotaError::UnsupportedFeature {
            error: "zkLogin is not supported".to_string(),
        })
    }
}

#[allow(deprecated)]
impl AsRef<[u8]> for ZkLoginAuthenticatorDeprecated {
    fn as_ref(&self) -> &[u8] {
        &[]
    }
}

/// Due to the incompatibility of [enum Signature] (which dispatches a trait
/// that assumes signature and pubkey bytes for verification), here we add a
/// wrapper enum where member can just implement a lightweight [trait
/// AuthenticatorTrait]. This way MultiSig (and future Authenticators) can
/// implement its own `verify`.
#[iota_proc_macros::allow_deprecated_for_derives]
#[enum_dispatch(AuthenticatorTrait)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(clippy::large_enum_variant)]
pub enum GenericSignature {
    MultiSig,
    Signature,
    #[deprecated(note = "zkLogin is deprecated and was never enabled on IOTA")]
    ZkLoginAuthenticatorDeprecated,
    PasskeyAuthenticator,
    MoveAuthenticator,
}

impl GenericSignature {
    pub fn is_passkey(&self) -> bool {
        matches!(self, GenericSignature::PasskeyAuthenticator(_))
    }

    pub fn is_upgraded_multisig(&self) -> bool {
        matches!(self, GenericSignature::MultiSig(_))
    }

    pub fn is_move_authenticator(&self) -> bool {
        matches!(self, GenericSignature::MoveAuthenticator(_))
    }

    pub fn verify_authenticator<T>(
        &self,
        value: &IntentMessage<T>,
        author: IotaAddress,
        verify_params: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize,
    {
        self.verify_claims(value, author, verify_params)
    }

    /// Parse [enum CompressedSignature] from trait IotaSignature `flag || sig
    /// || pk`. This is useful for the MultiSig to combine partial signature
    /// into a MultiSig public key.
    pub fn to_compressed(&self) -> Result<CompressedSignature, IotaError> {
        match self {
            GenericSignature::Signature(s) => {
                let bytes = s.signature_bytes();
                match s.scheme() {
                    SignatureScheme::ED25519 => Ok(CompressedSignature::Ed25519(
                        (&Ed25519Signature::from_bytes(bytes).map_err(|_| {
                            IotaError::InvalidSignature {
                                error: "Cannot parse ed25519 sig".to_string(),
                            }
                        })?)
                            .into(),
                    )),
                    SignatureScheme::Secp256k1 => Ok(CompressedSignature::Secp256k1(
                        (&Secp256k1Signature::from_bytes(bytes).map_err(|_| {
                            IotaError::InvalidSignature {
                                error: "Cannot parse secp256k1 sig".to_string(),
                            }
                        })?)
                            .into(),
                    )),
                    SignatureScheme::Secp256r1 | SignatureScheme::PasskeyAuthenticator => {
                        Ok(CompressedSignature::Secp256r1(
                            (&Secp256r1Signature::from_bytes(bytes).map_err(|_| {
                                IotaError::InvalidSignature {
                                    error: "Cannot parse secp256r1 sig".to_string(),
                                }
                            })?)
                                .into(),
                        ))
                    }
                    _ => Err(IotaError::UnsupportedFeature {
                        error: "Unsupported signature scheme".to_string(),
                    }),
                }
            }
            #[allow(deprecated)]
            GenericSignature::ZkLoginAuthenticatorDeprecated(_) => {
                Err(IotaError::UnsupportedFeature {
                    error: "zkLogin is not supported".to_string(),
                })
            }
            GenericSignature::PasskeyAuthenticator(s) => Ok(CompressedSignature::Passkey(
                PasskeyAuthenticatorAsBytes(s.as_ref().to_vec()),
            )),
            _ => Err(IotaError::UnsupportedFeature {
                error: "Unsupported signature scheme".to_string(),
            }),
        }
    }

    /// Parse [struct PublicKey] from trait IotaSignature `flag || sig || pk`.
    /// This is useful for the MultiSig to construct the bitmap in [struct
    /// MultiPublicKey].
    pub fn to_public_key(&self) -> Result<PublicKey, IotaError> {
        match self {
            GenericSignature::Signature(s) => {
                let bytes = s.public_key_bytes();
                match s.scheme() {
                    SignatureScheme::ED25519 => Ok(PublicKey::Ed25519(
                        (&Ed25519PublicKey::from_bytes(bytes).map_err(|_| {
                            IotaError::KeyConversion("Cannot parse ed25519 pk".to_string())
                        })?)
                            .into(),
                    )),
                    SignatureScheme::Secp256k1 => Ok(PublicKey::Secp256k1(
                        (&Secp256k1PublicKey::from_bytes(bytes).map_err(|_| {
                            IotaError::KeyConversion("Cannot parse secp256k1 pk".to_string())
                        })?)
                            .into(),
                    )),
                    SignatureScheme::Secp256r1 => Ok(PublicKey::Secp256r1(
                        (&Secp256r1PublicKey::from_bytes(bytes).map_err(|_| {
                            IotaError::KeyConversion("Cannot parse secp256r1 pk".to_string())
                        })?)
                            .into(),
                    )),
                    _ => Err(IotaError::UnsupportedFeature {
                        error: "Unsupported signature scheme in MultiSig".to_string(),
                    }),
                }
            }
            #[allow(deprecated)]
            GenericSignature::ZkLoginAuthenticatorDeprecated(_) => {
                Err(IotaError::UnsupportedFeature {
                    error: "zkLogin is not supported".to_string(),
                })
            }
            GenericSignature::PasskeyAuthenticator(s) => s.get_pk(),
            GenericSignature::MoveAuthenticator(_) => Err(IotaError::UnsupportedFeature {
                error: "Unsupported in MoveAuthenticator".to_string(),
            }),
            _ => Err(IotaError::UnsupportedFeature {
                error: "Unsupported signature scheme".to_string(),
            }),
        }
    }
}

/// GenericSignature encodes a single signature [enum Signature] as is `flag ||
/// signature || pubkey`. [struct Multisig] is encoded as
/// the MultiSig flag (0x03) concat with the bcs serialized bytes of [struct
/// Multisig] i.e. `flag || bcs_bytes(Multisig)`.
impl ToFromBytes for GenericSignature {
    fn from_bytes(bytes: &[u8]) -> Result<Self, FastCryptoError> {
        match SignatureScheme::from_flag_byte(
            bytes.first().ok_or(FastCryptoError::InputTooShort(0))?,
        ) {
            Ok(x) => match x {
                SignatureScheme::ED25519
                | SignatureScheme::Secp256k1
                | SignatureScheme::Secp256r1 => Ok(GenericSignature::Signature(
                    Signature::from_bytes(bytes).map_err(|_| FastCryptoError::InvalidSignature)?,
                )),
                SignatureScheme::MultiSig => {
                    Ok(GenericSignature::MultiSig(MultiSig::from_bytes(bytes)?))
                }
                #[allow(deprecated)]
                SignatureScheme::ZkLoginAuthenticatorDeprecated => {
                    // zkLogin is deprecated and was never enabled on IOTA — reject at
                    // deserialization.
                    Err(FastCryptoError::GeneralError(
                        "zkLogin is not supported".to_string(),
                    ))
                }
                SignatureScheme::PasskeyAuthenticator => {
                    let passkey = PasskeyAuthenticator::from_bytes(bytes)?;
                    Ok(GenericSignature::PasskeyAuthenticator(passkey))
                }
                SignatureScheme::MoveAuthenticator => {
                    let move_auth = MoveAuthenticator::from_bytes(bytes)?;
                    Ok(GenericSignature::MoveAuthenticator(move_auth))
                }
                _ => Err(FastCryptoError::InvalidInput),
            },
            Err(_) => Err(FastCryptoError::InvalidInput),
        }
    }
}

/// Trait useful to get the bytes reference for [enum GenericSignature].
impl AsRef<[u8]> for GenericSignature {
    fn as_ref(&self) -> &[u8] {
        match self {
            GenericSignature::MultiSig(s) => s.as_ref(),
            GenericSignature::Signature(s) => s.as_ref(),
            #[allow(deprecated)]
            GenericSignature::ZkLoginAuthenticatorDeprecated(s) => s.as_ref(),
            GenericSignature::PasskeyAuthenticator(s) => s.as_ref(),
            GenericSignature::MoveAuthenticator(s) => s.as_ref(),
        }
    }
}

impl ::serde::Serialize for GenericSignature {
    fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            #[derive(serde::Serialize)]
            struct GenericSignature(String);
            GenericSignature(self.encode_base64()).serialize(serializer)
        } else {
            #[derive(serde::Serialize)]
            struct GenericSignature<'a>(&'a [u8]);
            GenericSignature(self.as_ref()).serialize(serializer)
        }
    }
}

impl<'de> ::serde::Deserialize<'de> for GenericSignature {
    fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;

        if deserializer.is_human_readable() {
            #[derive(serde::Deserialize)]
            struct GenericSignature(String);
            let s = GenericSignature::deserialize(deserializer)?;
            Self::decode_base64(&s.0).map_err(::serde::de::Error::custom)
        } else {
            #[derive(serde::Deserialize)]
            struct GenericSignature(Vec<u8>);

            let data = GenericSignature::deserialize(deserializer)?;
            Self::from_bytes(&data.0).map_err(|e| Error::custom(e.to_string()))
        }
    }
}

/// This ports the wrapper trait to the verify_secure defined on [enum
/// Signature].
impl AuthenticatorTrait for Signature {
    #[instrument(level = "trace", skip_all)]
    fn verify_claims<T>(
        &self,
        value: &IntentMessage<T>,
        author: IotaAddress,
        _aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize,
    {
        self.verify_secure(value, author, self.scheme())
    }
}
