// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

use fastcrypto::{error::FastCryptoError, traits::ToFromBytes};
use move_core_types::language_storage::TypeTag;
use once_cell::sync::OnceCell;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use shared_crypto::intent::IntentMessage;

use crate::{
    base_types::{IotaAddress, ObjectRef},
    committee::EpochId,
    crypto::{SignatureScheme, default_hash},
    digests::{MoveAuthenticatorDigest, ZKLoginInputsDigest},
    error::IotaResult,
    signature::{AuthenticatorTrait, VerifyParams},
    signature_verification::VerifiedDigestCache,
    transaction::{CallArg, InputObjectKind, ObjectArg, SharedInputObject},
};

/// MoveAuthenticator is a GenericSignature variant that enables a new
/// method of authentication through Move code.
/// This function represents the data received by the Move authenticate function
/// during the Account Abstraction authentication flow.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct MoveAuthenticator {
    /// Input objects or primitive values
    call_args: Vec<CallArg>,
    /// Type arguments for the Move authenticate function
    #[schemars(with = "String")]
    type_arguments: Vec<TypeTag>,
    /// The reference to the object that this authenticates. This object
    /// represents the account being the sender of the transaction.
    object_to_authenticate: ObjectRef,
    /// A bytes representation of [struct MoveAuthenticator]. This helps with
    /// implementing trait [AsRef](core::convert::AsRef).
    #[serde(skip)]
    bytes: OnceCell<Vec<u8>>,
}

/// Necessary trait for
/// [SenderSignerData](crate::transaction::SenderSignedData).
impl Hash for MoveAuthenticator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl MoveAuthenticator {
    pub fn address(&self) -> IotaAddress {
        self.object_to_authenticate.0.into()
    }

    pub fn digest(&self) -> MoveAuthenticatorDigest {
        MoveAuthenticatorDigest::new(default_hash(self))
    }

    pub fn call_args(&self) -> &Vec<CallArg> {
        &self.call_args
    }

    pub fn type_arguments(&self) -> &Vec<TypeTag> {
        &self.type_arguments
    }

    pub fn object_to_authenticate(&self) -> &ObjectRef {
        &self.object_to_authenticate
    }

    pub fn input_objects(&self) -> Vec<InputObjectKind> {
        self.call_args
            .iter()
            .flat_map(|arg| arg.input_objects())
            .collect::<Vec<_>>()
    }

    pub fn receiving_objects(&self) -> Vec<ObjectRef> {
        self.call_args
            .iter()
            .flat_map(|arg| arg.receiving_objects())
            .collect()
    }

    pub fn shared_objects(&self) -> Vec<SharedInputObject> {
        self.call_args
            .iter()
            .filter_map(|arg| match arg {
                CallArg::Pure(_)
                | CallArg::Object(ObjectArg::Receiving(_))
                | CallArg::Object(ObjectArg::ImmOrOwnedObject(_)) => None,
                CallArg::Object(ObjectArg::SharedObject {
                    id,
                    initial_shared_version,
                    mutable,
                }) => Some(SharedInputObject {
                    id: *id,
                    initial_shared_version: *initial_shared_version,
                    mutable: *mutable,
                }),
            })
            .collect()
    }
}

impl AuthenticatorTrait for MoveAuthenticator {
    fn verify_user_authenticator_epoch(
        &self,
        _epoch: EpochId,
        _max_epoch_upper_bound_delta: Option<u64>,
    ) -> IotaResult {
        Ok(())
    }
    // This function accepts all inputs, as signature verification is performed
    // later on the Move side.
    fn verify_claims<T>(
        &self,
        _value: &IntentMessage<T>,
        _author: IotaAddress,
        _aux_verify_data: &VerifyParams,
        _zklogin_inputs_cache: Arc<VerifiedDigestCache<ZKLoginInputsDigest>>,
    ) -> IotaResult
    where
        T: Serialize,
    {
        Ok(())
    }
}

/// Necessary trait for
/// [SenderSignerData](crate::transaction::SenderSignedData).
impl PartialEq for MoveAuthenticator {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl ToFromBytes for MoveAuthenticator {
    fn from_bytes(bytes: &[u8]) -> Result<Self, FastCryptoError> {
        // The first byte matches the flag of MultiSig.
        if bytes.first().ok_or(FastCryptoError::InvalidInput)?
            != &SignatureScheme::MoveAuthenticator.flag()
        {
            return Err(FastCryptoError::InvalidInput);
        }
        let move_auth: MoveAuthenticator =
            bcs::from_bytes(&bytes[1..]).map_err(|_| FastCryptoError::InvalidSignature)?;
        Ok(move_auth)
    }
}

/// Necessary trait for
/// [SenderSignerData](crate::transaction::SenderSignedData).
impl Eq for MoveAuthenticator {}

impl AsRef<[u8]> for MoveAuthenticator {
    fn as_ref(&self) -> &[u8] {
        self.bytes
            .get_or_try_init::<_, eyre::Report>(|| {
                let as_bytes = bcs::to_bytes(self).expect("BCS serialization should not fail");
                let mut bytes = Vec::with_capacity(1 + as_bytes.len());
                bytes.push(SignatureScheme::MoveAuthenticator.flag());
                bytes.extend_from_slice(as_bytes.as_slice());
                Ok(bytes)
            })
            .expect("OnceCell invariant violated")
    }
}
