// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_ext::types::{Address, ObjectId, TypeTag, crypto::IntentMessage};
pub use iota_sdk_ext::types::{MoveAuthenticator, MoveAuthenticatorV1};
use serde::Serialize;

use crate::{
    base_types::{ObjectRef, SequenceNumber},
    digests::ObjectDigest,
    error::{IotaError, IotaResult, UserInputError, UserInputResult},
    signature::{AuthenticatorTrait, VerifyParams},
    transaction::{CallArg, CallArgExt, InputObjectKind, SharedObjectRef},
};

mod move_authenticator_ext {
    pub trait Sealed {}
    impl Sealed for super::MoveAuthenticator {}
    impl Sealed for super::MoveAuthenticatorV1 {}
}

pub trait MoveAuthenticatorExt: Sized + move_authenticator_ext::Sealed {
    /// Returns the address of the object being authenticated, which acts as the
    /// sender of the transaction.
    fn address(&self) -> Address;

    /// Returns the input objects or primitive values passed to the authenticate
    /// function.
    fn call_args(&self) -> &[CallArg];

    /// Returns the type arguments for the Move authenticate function.
    fn type_args(&self) -> &[TypeTag];

    /// Returns the object that is being authenticated (the account/sender).
    fn object_to_authenticate(&self) -> &CallArg;

    fn object_to_authenticate_components(
        &self,
    ) -> UserInputResult<(ObjectId, Option<SequenceNumber>, Option<ObjectDigest>)>;

    fn input_objects(&self) -> Vec<InputObjectKind>;

    fn receiving_objects(&self) -> Vec<ObjectRef>;

    fn shared_objects(&self) -> Vec<SharedObjectRef>;

    fn validity_check(&self, config: &ProtocolConfig) -> UserInputResult;
}

impl MoveAuthenticatorExt for MoveAuthenticator {
    fn address(&self) -> Address {
        match self {
            Self::V1(v1) => v1.address(),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }

    fn call_args(&self) -> &[CallArg] {
        match self {
            Self::V1(v1) => v1.call_args(),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }

    fn type_args(&self) -> &[TypeTag] {
        match self {
            Self::V1(v1) => v1.type_args(),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }

    fn object_to_authenticate(&self) -> &CallArg {
        match self {
            Self::V1(v1) => v1.object_to_authenticate(),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }

    fn object_to_authenticate_components(
        &self,
    ) -> UserInputResult<(ObjectId, Option<SequenceNumber>, Option<ObjectDigest>)> {
        match self {
            Self::V1(v1) => v1.object_to_authenticate_components(),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }

    fn input_objects(&self) -> Vec<InputObjectKind> {
        match self {
            Self::V1(v1) => v1.input_objects(),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }

    fn receiving_objects(&self) -> Vec<ObjectRef> {
        match self {
            Self::V1(v1) => v1.receiving_objects(),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }

    fn shared_objects(&self) -> Vec<SharedObjectRef> {
        match self {
            Self::V1(v1) => v1.shared_objects(),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }

    fn validity_check(&self, config: &ProtocolConfig) -> UserInputResult {
        match self {
            Self::V1(v1) => v1.validity_check(config),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }
}

impl MoveAuthenticatorExt for MoveAuthenticatorV1 {
    // Forward to the inherent accessors of the same name; inherent methods take
    // priority in resolution, so these are not recursive.
    fn address(&self) -> Address {
        self.address()
    }

    fn call_args(&self) -> &[CallArg] {
        self.call_args()
    }

    fn type_args(&self) -> &[TypeTag] {
        self.type_args()
    }

    fn object_to_authenticate(&self) -> &CallArg {
        self.object_to_authenticate()
    }

    fn object_to_authenticate_components(
        &self,
    ) -> UserInputResult<(ObjectId, Option<SequenceNumber>, Option<ObjectDigest>)> {
        Ok(match self.object_to_authenticate() {
            CallArg::Pure(_) => {
                return Err(UserInputError::Unsupported(
                    "MoveAuthenticatorV1 cannot authenticate pure inputs".to_string(),
                ));
            }
            CallArg::ImmutableOrOwned(object_ref) => (
                object_ref.object_id,
                Some(object_ref.version),
                Some(object_ref.digest),
            ),
            CallArg::Shared(SharedObjectRef {
                object_id, mutable, ..
            }) => {
                if *mutable {
                    return Err(UserInputError::Unsupported(
                        "MoveAuthenticatorV1 cannot authenticate mutable shared objects"
                            .to_string(),
                    ));
                }

                (*object_id, None, None)
            }
            CallArg::Receiving(_) => {
                return Err(UserInputError::Unsupported(
                    "MoveAuthenticator cannot authenticate receiving objects".to_string(),
                ));
            }
            _ => unimplemented!("a new CallArg enum variant was added and needs to be handled"),
        })
    }

    /// Returns all input objects used by the MoveAuthenticatorV1,
    /// including those from the object to authenticate.
    fn input_objects(&self) -> Vec<InputObjectKind> {
        self.call_args()
            .iter()
            .filter_map(|arg| arg.input_object_kind())
            .chain(self.object_to_authenticate().input_object_kind())
            .collect::<Vec<_>>()
    }

    fn receiving_objects(&self) -> Vec<ObjectRef> {
        self.call_args()
            .iter()
            .filter_map(|arg| arg.as_opt_receiving().copied())
            .collect()
    }

    /// Returns all shared input objects used by the MoveAuthenticatorV1,
    /// including those from the object to authenticate.
    fn shared_objects(&self) -> Vec<SharedObjectRef> {
        self.call_args()
            .iter()
            .filter_map(|arg| arg.as_opt_shared())
            .chain(self.object_to_authenticate().as_opt_shared())
            .cloned()
            .collect()
    }

    /// Validity check for MoveAuthenticatorV1.
    fn validity_check(&self, config: &ProtocolConfig) -> UserInputResult {
        // Check that the object to authenticate is valid.
        self.object_to_authenticate_components()?;

        // Inputs validity check.
        //
        // `validity_check` is not called for `object_to_authenticate` because it is
        // already validated with a dedicated function.

        // `ProtocolConfig::max_function_parameters` is used to check the call arguments
        // because MoveAuthenticatorV1 is considered as a simple programmable call to a
        // Move function.
        //
        // The limit includes the object to authenticate, the auth context and the tx
        // context, so we subtract 3 here.
        let max_args = (config.max_function_parameters() - 3) as usize;
        fp_ensure!(
            self.call_args().len() < max_args,
            UserInputError::SizeLimitExceeded {
                limit: "maximum arguments in MoveAuthenticatorV1".to_string(),
                value: max_args.to_string()
            }
        );

        fp_ensure!(
            self.receiving_objects().is_empty(),
            UserInputError::Unsupported(
                "MoveAuthenticatorV1 cannot have receiving objects as input".to_string(),
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
        // `MoveCall`.
        let mut type_arguments_count = 0;
        self.type_args().iter().try_for_each(|type_arg| {
            crate::transaction::type_tag_validity_check(type_arg, config, &mut type_arguments_count)
        })?;

        Ok(())
    }
}

impl AuthenticatorTrait for MoveAuthenticator {
    // This function accepts all inputs, as signature verification is performed
    // later on the Move side.
    fn verify_claims<T>(
        &self,
        value: &IntentMessage<T>,
        author: Address,
        aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize,
    {
        match self {
            Self::V1(v1) => v1.verify_claims(value, author, aux_verify_data),
            _ => unimplemented!(
                "a new MoveAuthenticator enum variant was added and needs to be handled"
            ),
        }
    }
}

impl AuthenticatorTrait for MoveAuthenticatorV1 {
    // This function accepts all inputs, as signature verification is performed
    // later on the Move side.
    fn verify_claims<T>(
        &self,
        _value: &IntentMessage<T>,
        author: Address,
        _aux_verify_data: &VerifyParams,
    ) -> IotaResult
    where
        T: Serialize,
    {
        if author != self.address() {
            return Err(IotaError::InvalidSignature {
                error: "Invalid author".to_string(),
            });
        };

        Ok(())
    }
}
