// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::Arc,
};

use fastcrypto::{error::FastCryptoError, traits::ToFromBytes};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::crypto::IntentMessage;
use once_cell::sync::OnceCell;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    committee::EpochId,
    crypto::{SignatureScheme, default_hash},
    digests::{MoveAuthenticatorDigest, ObjectDigest, ZKLoginInputsDigest},
    error::{IotaError, IotaResult, UserInputError, UserInputResult},
    signature::{AuthenticatorTrait, VerifyParams},
    signature_verification::VerifiedDigestCache,
    transaction::{CallArg, InputObjectKind, ObjectArg, SharedInputObject},
    type_input::TypeInput,
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
    #[schemars(with = "Vec<String>")]
    type_arguments: Vec<TypeInput>,
    /// The object that is authenticated. Represents the account being the
    /// sender of the transaction.
    object_to_authenticate: CallArg,
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
    pub fn new(
        call_args: Vec<CallArg>,
        type_arguments: Vec<TypeInput>,
        object_to_authenticate: CallArg,
    ) -> Self {
        Self {
            call_args,
            type_arguments,
            object_to_authenticate,
            bytes: OnceCell::new(),
        }
    }

    pub fn address(&self) -> IotaResult<IotaAddress> {
        let (id, _, _) = self.object_to_authenticate_components()?;
        Ok(IotaAddress::from(id))
    }

    pub fn digest(&self) -> MoveAuthenticatorDigest {
        MoveAuthenticatorDigest::new(default_hash(self))
    }

    pub fn call_args(&self) -> &Vec<CallArg> {
        &self.call_args
    }

    pub fn type_arguments(&self) -> &Vec<TypeInput> {
        &self.type_arguments
    }

    pub fn object_to_authenticate(&self) -> &CallArg {
        &self.object_to_authenticate
    }

    pub fn object_to_authenticate_components(
        &self,
    ) -> UserInputResult<(ObjectID, Option<SequenceNumber>, Option<ObjectDigest>)> {
        Ok(match self.object_to_authenticate() {
            CallArg::Pure(_) => {
                return Err(UserInputError::Unsupported(
                    "MoveAuthenticator cannot authenticate pure inputs".to_string(),
                ));
            }
            CallArg::Object(object_arg) => match object_arg {
                ObjectArg::ImmOrOwnedObject((id, sequence_number, digest)) => {
                    (*id, Some(*sequence_number), Some(*digest))
                }
                ObjectArg::SharedObject { id, mutable, .. } => {
                    if *mutable {
                        return Err(UserInputError::Unsupported(
                            "MoveAuthenticator cannot authenticate mutable shared objects"
                                .to_string(),
                        ));
                    }

                    (*id, None, None)
                }
                ObjectArg::Receiving(_) => {
                    return Err(UserInputError::Unsupported(
                        "MoveAuthenticator cannot authenticate receiving objects".to_string(),
                    ));
                }
            },
        })
    }

    /// Returns all input objects used by the MoveAuthenticator,
    /// including those from the object to authenticate.
    pub fn input_objects(&self) -> Vec<InputObjectKind> {
        self.call_args
            .iter()
            .flat_map(|arg| arg.input_objects())
            .chain(self.object_to_authenticate().input_objects())
            .collect::<Vec<_>>()
    }

    pub fn receiving_objects(&self) -> Vec<ObjectRef> {
        self.call_args
            .iter()
            .flat_map(|arg| arg.receiving_objects())
            .collect()
    }

    /// Returns all shared input objects used by the MoveAuthenticator,
    /// including those from the object to authenticate.
    pub fn shared_objects(&self) -> Vec<SharedInputObject> {
        self.call_args
            .iter()
            .flat_map(|arg| arg.shared_objects())
            .chain(self.object_to_authenticate().shared_objects())
            .collect()
    }

    /// Validity check for MoveAuthenticator.
    pub fn validity_check(&self, config: &ProtocolConfig) -> UserInputResult {
        // Check that the object to authenticate is valid.
        self.object_to_authenticate_components()?;

        // Inputs validity check.
        //
        // `validity_check` is not called for `object_to_authenticate` because it is
        // already validated with a dedicated function.

        // `ProtocolConfig::max_function_parameters` is used to check the call arguments
        // because MoveAuthenticator is considered as a simple programmable call to a
        // Move function.
        //
        // The limit includes the object to authenticate, the auth context and the tx
        // context, so we subtract 3 here.
        let max_args = (config.max_function_parameters() - 3) as usize;
        fp_ensure!(
            self.call_args().len() < max_args,
            UserInputError::SizeLimitExceeded {
                limit: "maximum arguments in MoveAuthenticator".to_string(),
                value: max_args.to_string()
            }
        );

        fp_ensure!(
            self.receiving_objects().is_empty(),
            UserInputError::Unsupported(
                "MoveAuthenticator cannot have receiving objects as input".to_string(),
            )
        );

        let mut used = HashSet::new();
        fp_ensure!(
            self.input_objects()
                .iter()
                .all(|o| used.insert(o.object_id())),
            UserInputError::DuplicateObjectRefInput
        );

        self.call_args()
            .iter()
            .try_for_each(|obj| obj.validity_check(config))?;

        // Type arguments validity check.
        //
        // Each type argument is checked for validity in the same way as it is done for
        // `ProgrammableMoveCall`.
        let mut type_arguments_count = 0;
        self.type_arguments().iter().try_for_each(|type_arg| {
            crate::transaction::type_input_validity_check(
                type_arg,
                config,
                &mut type_arguments_count,
            )
        })?;

        Ok(())
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
        author: IotaAddress,
        _aux_verify_data: &VerifyParams,
        _zklogin_inputs_cache: Arc<VerifiedDigestCache<ZKLoginInputsDigest>>,
    ) -> IotaResult
    where
        T: Serialize,
    {
        if author != self.address()? {
            return Err(IotaError::InvalidSignature {
                error: "Invalid author".to_string(),
            });
        };

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
