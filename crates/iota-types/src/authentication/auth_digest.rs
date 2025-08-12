// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use anyhow::{anyhow, bail};
use fastcrypto::encoding::{Base58, Encoding};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::digests::Digest;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
pub struct AuthenticationDigest(Digest);

impl Default for AuthenticationDigest {
    fn default() -> Self {
        Self::ZERO
    }
}

impl AuthenticationDigest {
    pub const ZERO: Self = Self(Digest::ZERO);

    pub const fn new(digest: [u8; 32]) -> Self {
        Self(Digest::new(digest))
    }

    pub const fn genesis_marker() -> Self {
        Self::ZERO
    }

    pub fn generate<R: rand::RngCore + rand::CryptoRng>(rng: R) -> Self {
        Self(Digest::generate(rng))
    }

    pub fn random() -> Self {
        Self(Digest::random())
    }

    pub fn inner(&self) -> &[u8; 32] {
        self.0.inner()
    }

    pub fn into_inner(self) -> [u8; 32] {
        self.0.into_inner()
    }

    pub fn base58_encode(&self) -> String {
        Base58::encode(self.0)
    }

    pub fn next_lexicographical(&self) -> Option<Self> {
        self.0.next_lexicographical().map(Self)
    }
}

impl AsRef<[u8]> for AuthenticationDigest {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsRef<[u8; 32]> for AuthenticationDigest {
    fn as_ref(&self) -> &[u8; 32] {
        self.0.as_ref()
    }
}

impl From<AuthenticationDigest> for [u8; 32] {
    fn from(digest: AuthenticationDigest) -> Self {
        digest.into_inner()
    }
}

impl From<[u8; 32]> for AuthenticationDigest {
    fn from(digest: [u8; 32]) -> Self {
        Self::new(digest)
    }
}

impl fmt::Display for AuthenticationDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for AuthenticationDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuthenticationDigest")
            .field(&self.0)
            .finish()
    }
}

impl fmt::LowerHex for AuthenticationDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for AuthenticationDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl TryFrom<&[u8]> for AuthenticationDigest {
    type Error = crate::error::IotaError;

    fn try_from(bytes: &[u8]) -> Result<Self, crate::error::IotaError> {
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| crate::error::IotaError::InvalidAuthenticationDigest)?;
        Ok(Self::new(arr))
    }
}

impl std::str::FromStr for AuthenticationDigest {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut result = [0; 32];
        let buffer = Base58::decode(s).map_err(|e| anyhow!(e))?;
        if buffer.len() != 32 {
            bail!("Invalid digest length. Expected 32 bytes");
        }
        result.copy_from_slice(&buffer);
        Ok(AuthenticationDigest::new(result))
    }
}
