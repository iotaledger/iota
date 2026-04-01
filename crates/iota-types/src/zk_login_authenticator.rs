// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(deprecated)]

//! Deprecated: zkLogin is no longer supported.
//! This module is retained solely for BCS/serde deserialization compatibility
//! in [`GenericSignature`](crate::signature::GenericSignature).

use std::hash::{Hash, Hasher};

use fastcrypto::{error::FastCryptoError, traits::ToFromBytes};
use fastcrypto_zkp::bn254::zk_login::ZkLoginInputs;
use iota_sdk_types::crypto::IntentMessage;
use once_cell::sync::OnceCell;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    base_types::EpochId,
    crypto::{Signature, SignatureScheme},
    error::{IotaError, IotaResult},
    signature::{AuthenticatorTrait, VerifyParams},
};

/// Deprecated zkLogin authenticator.
/// Kept only for BCS/serde deserialization compatibility. All operations return
/// an `UnsupportedFeature` error.
#[deprecated(note = "zkLogin is no longer supported")]
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZkLoginAuthenticator {
    inputs: ZkLoginInputs,
    max_epoch: EpochId,
    user_signature: Signature,
    #[serde(skip)]
    pub bytes: OnceCell<Vec<u8>>,
}

impl PartialEq for ZkLoginAuthenticator {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

/// Necessary trait for [struct SenderSignedData].
impl Eq for ZkLoginAuthenticator {}

/// Necessary trait for [struct SenderSignedData].
impl Hash for ZkLoginAuthenticator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl AuthenticatorTrait for ZkLoginAuthenticator {
    fn verify_user_authenticator_epoch(
        &self,
        _epoch: EpochId,
        _max_epoch_upper_bound_delta: Option<u64>,
    ) -> IotaResult {
        Err(IotaError::UnsupportedFeature {
            error: "zkLogin is not supported".to_string(),
        })
    }

    fn verify_claims<T>(
        &self,
        _value: &IntentMessage<T>,
        _author: crate::base_types::IotaAddress,
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

impl ToFromBytes for ZkLoginAuthenticator {
    fn from_bytes(bytes: &[u8]) -> Result<Self, FastCryptoError> {
        // The first byte matches the flag of MultiSig.
        if bytes.first().ok_or(FastCryptoError::InvalidInput)?
            != &SignatureScheme::ZkLoginAuthenticator.flag()
        {
            return Err(FastCryptoError::InvalidInput);
        }
        let mut zk_login: ZkLoginAuthenticator =
            bcs::from_bytes(&bytes[1..]).map_err(|_| FastCryptoError::InvalidSignature)?;
        zk_login.inputs.init()?;
        Ok(zk_login)
    }
}

impl AsRef<[u8]> for ZkLoginAuthenticator {
    fn as_ref(&self) -> &[u8] {
        self.bytes
            .get_or_try_init::<_, eyre::Report>(|| {
                let as_bytes = bcs::to_bytes(self).expect("BCS serialization should not fail");
                let mut bytes = Vec::with_capacity(1 + as_bytes.len());
                bytes.push(SignatureScheme::ZkLoginAuthenticator.flag());
                bytes.extend_from_slice(as_bytes.as_slice());
                Ok(bytes)
            })
            .expect("OnceCell invariant violated")
    }
}
