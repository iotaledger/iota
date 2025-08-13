// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::{fmt, ops::Deref};

use crypto::{
    hashes::{Digest, blake2b::Blake2b256},
    signatures::ed25519::{PublicKey, PublicKeyBytes, Signature},
};
use packable::{
    Packable,
    error::{UnpackError, UnpackErrorExt},
    packer::Packer,
    unpacker::Unpacker,
};

use crate::types::block::{Error, address::Ed25519Address};

/// An Ed25519 signature.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Ed25519Signature {
    public_key: PublicKeyBytes,
    signature: Signature,
}

impl Ed25519Signature {
    /// The signature kind of an [`Ed25519Signature`].
    pub const KIND: u8 = 0;
    /// Length of an ED25519 public key.
    pub const PUBLIC_KEY_LENGTH: usize = PublicKey::LENGTH;
    /// Length of an ED25519 signature.
    pub const SIGNATURE_LENGTH: usize = Signature::LENGTH;

    /// Creates a new [`Ed25519Signature`] from a validated public key and
    /// signature.
    pub fn new(public_key: PublicKey, signature: Signature) -> Self {
        Self {
            public_key: public_key.to_bytes().into(),
            signature,
        }
    }

    /// Creates a new [`Ed25519Signature`] from public key bytes and signature.
    pub fn new_from_bytes(public_key: PublicKeyBytes, signature: Signature) -> Self {
        Self {
            public_key,
            signature,
        }
    }

    /// Creates a new [`Ed25519Signature`] from bytes.
    #[deprecated(since = "1.1.4", note = "use Ed25519Signature::from_bytes instead")]
    pub fn try_from_bytes(
        public_key: [u8; Self::PUBLIC_KEY_LENGTH],
        signature: [u8; Self::SIGNATURE_LENGTH],
    ) -> Result<Self, Error> {
        Ok(Self::from_bytes(public_key, signature))
    }

    /// Creates a new [`Ed25519Signature`] from bytes.
    pub fn from_bytes(
        public_key: [u8; Self::PUBLIC_KEY_LENGTH],
        signature: [u8; Self::SIGNATURE_LENGTH],
    ) -> Self {
        Self {
            public_key: PublicKeyBytes::from_bytes(public_key),
            signature: Signature::from_bytes(signature),
        }
    }

    /// Returns the public key of an [`Ed25519Signature`].
    #[deprecated(
        since = "1.1.4",
        note = "use Ed25519Signature::public_key_bytes instead"
    )]
    pub fn public_key(&self) -> &PublicKey {
        panic!("deprecated method: use Ed25519Signature::public_key_bytes instead")
    }

    /// Returns the unvalidated public key bytes of an [`Ed25519Signature`].
    pub fn public_key_bytes(&self) -> &PublicKeyBytes {
        &self.public_key
    }

    /// Return the actual signature of an [`Ed25519Signature`].
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Verify a message using the signature.
    #[deprecated(since = "1.1.4", note = "use Ed25519Signature::try_verify instead")]
    pub fn verify(&self, message: &[u8]) -> bool {
        self.public_key
            .verify(&self.signature, message)
            .unwrap_or_default()
    }

    /// Verify a message using the signature.
    pub fn try_verify(&self, message: &[u8]) -> Result<bool, crypto::Error> {
        self.public_key.verify(&self.signature, message)
    }

    /// Verifies the [`Ed25519Signature`] for a message against an
    /// [`Ed25519Address`].
    pub fn is_valid(&self, message: &[u8], address: &Ed25519Address) -> Result<(), Error> {
        let signature_address: [u8; Self::PUBLIC_KEY_LENGTH] =
            Blake2b256::digest(self.public_key).into();

        if address.deref() != &signature_address {
            return Err(Error::SignaturePublicKeyMismatch {
                expected: prefix_hex::encode(address.as_ref()),
                actual: prefix_hex::encode(signature_address),
            });
        }

        if !self.try_verify(message)? {
            return Err(Error::InvalidSignature);
        }

        Ok(())
    }
}

impl fmt::Debug for Ed25519Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[repr(transparent)]
        struct UnquotedStr<'a>(&'a str);

        impl fmt::Debug for UnquotedStr<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        f.debug_struct("Ed25519Signature")
            .field(
                "public_key",
                &UnquotedStr(&prefix_hex::encode(self.public_key.as_slice())),
            )
            .field(
                "signature",
                &UnquotedStr(&prefix_hex::encode(self.signature.to_bytes())),
            )
            .finish()
    }
}

impl Packable for Ed25519Signature {
    type UnpackError = Error;
    type UnpackVisitor = ();

    fn pack<P: Packer>(&self, packer: &mut P) -> Result<(), P::Error> {
        self.public_key.to_bytes().pack(packer)?;
        self.signature.to_bytes().pack(packer)?;

        Ok(())
    }

    fn unpack<U: Unpacker, const VERIFY: bool>(
        unpacker: &mut U,
        visitor: &Self::UnpackVisitor,
    ) -> Result<Self, UnpackError<Self::UnpackError, U::Error>> {
        let public_key =
            <[u8; Self::PUBLIC_KEY_LENGTH]>::unpack::<_, VERIFY>(unpacker, visitor).coerce()?;
        let signature =
            <[u8; Self::SIGNATURE_LENGTH]>::unpack::<_, VERIFY>(unpacker, visitor).coerce()?;
        Ok(Self::from_bytes(public_key, signature))
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use alloc::string::String;

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::types::block::Error;

    /// Defines an Ed25519 signature.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Ed25519SignatureDto {
        #[serde(rename = "type")]
        pub kind: u8,
        pub public_key: String,
        pub signature: String,
    }

    impl From<&Ed25519Signature> for Ed25519SignatureDto {
        fn from(value: &Ed25519Signature) -> Self {
            Self {
                kind: Ed25519Signature::KIND,
                public_key: prefix_hex::encode(value.public_key.as_slice()),
                signature: prefix_hex::encode(value.signature.to_bytes()),
            }
        }
    }

    impl TryFrom<Ed25519SignatureDto> for Ed25519Signature {
        type Error = Error;

        fn try_from(value: Ed25519SignatureDto) -> Result<Self, Self::Error> {
            Ok(Self::from_bytes(
                prefix_hex::decode(&value.public_key)
                    .map_err(|_| Error::InvalidField("publicKey"))?,
                prefix_hex::decode(&value.signature)
                    .map_err(|_| Error::InvalidField("signature"))?,
            ))
        }
    }
}
