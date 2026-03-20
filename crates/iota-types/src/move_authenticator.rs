// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::Arc,
};

use enum_dispatch::enum_dispatch;
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
    #[serde(flatten)]
    pub(crate) inner: MoveAuthenticatorInner,
    /// A bytes representation of [struct MoveAuthenticator]. This helps with
    /// implementing trait [AsRef](core::convert::AsRef).
    #[serde(skip)]
    bytes: OnceCell<Vec<u8>>,
}

impl MoveAuthenticator {
    /// Creates a new MoveAuthenticator of version 1.
    pub fn new_v1(
        call_args: Vec<CallArg>,
        type_arguments: Vec<TypeInput>,
        object_to_authenticate: CallArg,
    ) -> Self {
        Self {
            inner: MoveAuthenticatorInner::new_v1(
                call_args,
                type_arguments,
                object_to_authenticate,
            ),
            bytes: OnceCell::new(),
        }
    }

    /// Constructs a `MoveAuthenticator` from a deserialized
    /// [`MoveAuthenticatorInner`].
    pub(crate) fn from_inner(inner: MoveAuthenticatorInner) -> Self {
        Self {
            inner,
            bytes: OnceCell::new(),
        }
    }

    /// Computes the digest of the MoveAuthenticator.
    pub fn digest(&self) -> MoveAuthenticatorDigest {
        MoveAuthenticatorDigest::new(default_hash(self))
    }

    /// Returns the version of the MoveAuthenticator.
    pub fn version(&self) -> u64 {
        self.inner.version()
    }

    /// Returns the address of the MoveAuthenticator.
    pub fn address(&self) -> IotaResult<IotaAddress> {
        self.inner.address()
    }

    /// Returns the call arguments of the MoveAuthenticator.
    pub fn call_args(&self) -> &Vec<CallArg> {
        self.inner.call_args()
    }

    /// Returns the type arguments of the MoveAuthenticator.
    pub fn type_arguments(&self) -> &Vec<TypeInput> {
        self.inner.type_arguments()
    }

    /// Returns the object to authenticate of the MoveAuthenticator.
    pub fn object_to_authenticate(&self) -> &CallArg {
        self.inner.object_to_authenticate()
    }

    /// Returns the components of the object to authenticate.
    pub fn object_to_authenticate_components(
        &self,
    ) -> UserInputResult<(ObjectID, Option<SequenceNumber>, Option<ObjectDigest>)> {
        self.inner.object_to_authenticate_components()
    }

    /// Returns all input objects used by the MoveAuthenticator,
    /// including those from the object to authenticate.
    pub fn input_objects(&self) -> Vec<InputObjectKind> {
        self.inner.input_objects()
    }

    /// Returns all receiving objects used by the MoveAuthenticator.
    pub fn receiving_objects(&self) -> Vec<ObjectRef> {
        self.inner.receiving_objects()
    }

    /// Returns all shared input objects used by the MoveAuthenticator,
    /// including those from the object to authenticate.
    pub fn shared_objects(&self) -> Vec<SharedInputObject> {
        self.inner.shared_objects()
    }

    /// Validity check for MoveAuthenticator.
    pub fn validity_check(&self, config: &ProtocolConfig) -> UserInputResult {
        self.inner.validity_check(config)
    }
}

impl AuthenticatorTrait for MoveAuthenticator {
    fn verify_user_authenticator_epoch(
        &self,
        epoch: EpochId,
        max_epoch_upper_bound_delta: Option<u64>,
    ) -> IotaResult {
        self.inner
            .verify_user_authenticator_epoch(epoch, max_epoch_upper_bound_delta)
    }
    // This function accepts all inputs, as signature verification is performed
    // later on the Move side.
    fn verify_claims<T>(
        &self,
        value: &IntentMessage<T>,
        author: IotaAddress,
        aux_verify_data: &VerifyParams,
        zklogin_inputs_cache: Arc<VerifiedDigestCache<ZKLoginInputsDigest>>,
    ) -> IotaResult
    where
        T: Serialize,
    {
        self.inner
            .verify_claims(value, author, aux_verify_data, zklogin_inputs_cache)
    }
}

/// Necessary trait for
/// [SenderSignerData](crate::transaction::SenderSignedData). This trait is
/// implemented only for MoveAuthenticator and not for specific versions of
/// MoveAuthenticator (e.g., MoveAuthenticatorV1) because the custom
/// serialization/deserialization signature logic is defined on the
/// MoveAuthenticator level.
impl Hash for MoveAuthenticator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

/// Necessary trait for
/// [GenericSignature](crate::signature::GenericSignature). This trait is
/// implemented only for MoveAuthenticator and not for specific versions of
/// MoveAuthenticator (e.g., MoveAuthenticatorV1) because the custom
/// serialization/deserialization signature logic is defined on the
/// MoveAuthenticator level.
impl ToFromBytes for MoveAuthenticator {
    fn from_bytes(bytes: &[u8]) -> Result<Self, FastCryptoError> {
        // The first byte matches the flag of MoveAuthenticator.
        if bytes.first().ok_or(FastCryptoError::InvalidInput)?
            != &SignatureScheme::MoveAuthenticator.flag()
        {
            return Err(FastCryptoError::InvalidInput);
        }

        let inner: MoveAuthenticatorInner =
            bcs::from_bytes(&bytes[1..]).map_err(|_| FastCryptoError::InvalidSignature)?;
        Ok(Self {
            inner,
            bytes: OnceCell::new(),
        })
    }
}

/// Necessary trait for
/// [GenericSignature](crate::signature::GenericSignature). This trait is
/// implemented only for MoveAuthenticator and not for specific versions of
/// MoveAuthenticator (e.g., MoveAuthenticatorV1) because the custom
/// serialization/deserialization signature logic is defined on the
/// MoveAuthenticator level.
impl AsRef<[u8]> for MoveAuthenticator {
    fn as_ref(&self) -> &[u8] {
        self.bytes.get_or_init(|| {
            let as_bytes = bcs::to_bytes(&self.inner).expect("BCS serialization should not fail");
            let mut bytes = Vec::with_capacity(1 + as_bytes.len());
            bytes.push(SignatureScheme::MoveAuthenticator.flag());
            bytes.extend_from_slice(as_bytes.as_slice());
            bytes
        })
    }
}

/// Necessary trait for
/// [SenderSignerData](crate::transaction::SenderSignedData). This trait is
/// implemented only for MoveAuthenticator and not for specific versions of
/// MoveAuthenticator (e.g., MoveAuthenticatorV1) because the custom
/// serialization/deserialization signature logic is defined on the
/// MoveAuthenticator level.
impl PartialEq for MoveAuthenticator {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

/// Necessary trait for
/// [SenderSignerData](crate::transaction::SenderSignedData). This trait is
/// implemented only for MoveAuthenticator and not for specific versions of
/// MoveAuthenticator (e.g., MoveAuthenticatorV1) because the custom
/// serialization/deserialization signature logic is defined at the
/// MoveAuthenticator level.
impl Eq for MoveAuthenticator {}

/// MoveAuthenticatorInner is an enum that represents the different versions
/// of MoveAuthenticator.
#[enum_dispatch(AuthenticatorTrait)]
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub enum MoveAuthenticatorInner {
    V1(MoveAuthenticatorV1),
}

impl MoveAuthenticatorInner {
    pub fn new_v1(
        call_args: Vec<CallArg>,
        type_arguments: Vec<TypeInput>,
        object_to_authenticate: CallArg,
    ) -> Self {
        MoveAuthenticatorInner::V1(MoveAuthenticatorV1::new(
            call_args,
            type_arguments,
            object_to_authenticate,
        ))
    }

    pub fn version(&self) -> u64 {
        match self {
            MoveAuthenticatorInner::V1(_) => 1,
        }
    }

    pub fn address(&self) -> IotaResult<IotaAddress> {
        match self {
            MoveAuthenticatorInner::V1(v1) => v1.address(),
        }
    }

    pub fn call_args(&self) -> &Vec<CallArg> {
        match self {
            MoveAuthenticatorInner::V1(v1) => v1.call_args(),
        }
    }

    pub fn type_arguments(&self) -> &Vec<TypeInput> {
        match self {
            MoveAuthenticatorInner::V1(v1) => v1.type_arguments(),
        }
    }

    pub fn object_to_authenticate(&self) -> &CallArg {
        match self {
            MoveAuthenticatorInner::V1(v1) => v1.object_to_authenticate(),
        }
    }

    pub fn object_to_authenticate_components(
        &self,
    ) -> UserInputResult<(ObjectID, Option<SequenceNumber>, Option<ObjectDigest>)> {
        match self {
            MoveAuthenticatorInner::V1(v1) => v1.object_to_authenticate_components(),
        }
    }

    pub fn input_objects(&self) -> Vec<InputObjectKind> {
        match self {
            MoveAuthenticatorInner::V1(v1) => v1.input_objects(),
        }
    }

    pub fn receiving_objects(&self) -> Vec<ObjectRef> {
        match self {
            MoveAuthenticatorInner::V1(v1) => v1.receiving_objects(),
        }
    }

    pub fn shared_objects(&self) -> Vec<SharedInputObject> {
        match self {
            MoveAuthenticatorInner::V1(v1) => v1.shared_objects(),
        }
    }

    pub fn validity_check(&self, config: &ProtocolConfig) -> UserInputResult {
        match self {
            MoveAuthenticatorInner::V1(v1) => v1.validity_check(config),
        }
    }
}

/// MoveAuthenticatorV1 is the first version of MoveAuthenticator.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct MoveAuthenticatorV1 {
    /// Input objects or primitive values
    call_args: Vec<CallArg>,
    /// Type arguments for the Move authenticate function
    #[schemars(with = "Vec<String>")]
    type_arguments: Vec<TypeInput>,
    /// The object that is authenticated. Represents the account being the
    /// sender of the transaction.
    object_to_authenticate: CallArg,
}

impl MoveAuthenticatorV1 {
    pub fn new(
        call_args: Vec<CallArg>,
        type_arguments: Vec<TypeInput>,
        object_to_authenticate: CallArg,
    ) -> Self {
        Self {
            call_args,
            type_arguments,
            object_to_authenticate,
        }
    }

    /// Returns the address of the MoveAuthenticatorV1, which is the object ID
    /// of the object to authenticate.
    pub fn address(&self) -> IotaResult<IotaAddress> {
        let (id, _, _) = self.object_to_authenticate_components()?;
        Ok(IotaAddress::from(id))
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
                    "MoveAuthenticatorV1 cannot authenticate pure inputs".to_string(),
                ));
            }
            CallArg::Object(object_arg) => match object_arg {
                ObjectArg::ImmOrOwnedObject((id, sequence_number, digest)) => {
                    (*id, Some(*sequence_number), Some(*digest))
                }
                ObjectArg::SharedObject { id, mutable, .. } => {
                    if *mutable {
                        return Err(UserInputError::Unsupported(
                            "MoveAuthenticatorV1 cannot authenticate mutable shared objects"
                                .to_string(),
                        ));
                    }

                    (*id, None, None)
                }
                ObjectArg::Receiving(_) => {
                    return Err(UserInputError::Unsupported(
                        "MoveAuthenticatorV1 cannot authenticate receiving objects".to_string(),
                    ));
                }
            },
        })
    }

    /// Returns all input objects used by the MoveAuthenticatorV1,
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

    /// Returns all shared input objects used by the MoveAuthenticatorV1,
    /// including those from the object to authenticate.
    pub fn shared_objects(&self) -> Vec<SharedInputObject> {
        self.call_args
            .iter()
            .flat_map(|arg| arg.shared_objects())
            .chain(self.object_to_authenticate().shared_objects())
            .collect()
    }

    /// Validity check for MoveAuthenticatorV1.
    pub fn validity_check(&self, config: &ProtocolConfig) -> UserInputResult {
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

impl AuthenticatorTrait for MoveAuthenticatorV1 {
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

#[cfg(test)]
mod tests {
    use fastcrypto::traits::ToFromBytes;

    use super::*;
    use crate::{
        base_types::{ObjectID, SequenceNumber},
        digests::ObjectDigest,
        transaction::{CallArg, ObjectArg},
    };

    fn make_simple_authenticator() -> MoveAuthenticator {
        let object_to_authenticate = CallArg::Object(ObjectArg::ImmOrOwnedObject((
            ObjectID::ZERO,
            SequenceNumber::default(),
            ObjectDigest::MIN,
        )));
        MoveAuthenticator::new_v1(vec![], vec![], object_to_authenticate)
    }

    #[test]
    fn round_trip() {
        let auth = make_simple_authenticator();
        let bytes = auth.as_ref().to_vec();
        let decoded = MoveAuthenticator::from_bytes(&bytes).expect("round-trip should succeed");
        assert_eq!(auth, decoded);
    }

    #[test]
    fn as_ref_starts_with_flag_byte() {
        let auth = make_simple_authenticator();
        let bytes = auth.as_ref();
        assert_eq!(bytes[0], SignatureScheme::MoveAuthenticator.flag());
    }

    #[test]
    fn as_ref_is_cached() {
        let auth = make_simple_authenticator();
        let bytes1 = auth.as_ref();
        let bytes2 = auth.as_ref();
        assert!(std::ptr::eq(bytes1.as_ptr(), bytes2.as_ptr()));
    }

    #[test]
    fn from_bytes_rejects_wrong_flag() {
        let auth = make_simple_authenticator();
        let mut bytes = auth.as_ref().to_vec();
        bytes[0] = SignatureScheme::ED25519.flag();
        assert!(MoveAuthenticator::from_bytes(&bytes).is_err());
    }

    #[test]
    fn from_bytes_rejects_empty_input() {
        assert!(MoveAuthenticator::from_bytes(&[]).is_err());
    }

    #[test]
    fn from_bytes_rejects_flag_only() {
        let flag = SignatureScheme::MoveAuthenticator.flag();
        assert!(MoveAuthenticator::from_bytes(&[flag]).is_err());
    }

    // ---- Signable / SignableBytes round-trip tests ----

    use crate::crypto::{Signable, SignableBytes};

    /// Helper: produce the signable bytes for a MoveAuthenticator (the
    /// `"MoveAuthenticator::" ++ BCS(inner)` format).
    fn signable_bytes(auth: &MoveAuthenticator) -> Vec<u8> {
        let mut buf = Vec::new();
        auth.write(&mut buf);
        buf
    }

    #[test]
    fn signable_round_trip() {
        let auth = make_simple_authenticator();
        let bytes = signable_bytes(&auth);
        let decoded = MoveAuthenticator::from_signable_bytes(&bytes)
            .expect("round-trip via signable bytes should succeed");
        assert_eq!(auth, decoded);
    }

    #[test]
    fn signable_bytes_start_with_name_tag() {
        let auth = make_simple_authenticator();
        let bytes = signable_bytes(&auth);
        let tag = b"MoveAuthenticator::";
        assert!(
            bytes.starts_with(tag),
            "signable bytes must start with the hardcoded name tag"
        );
    }

    #[test]
    fn signable_bytes_payload_is_bcs_of_inner() {
        let auth = make_simple_authenticator();
        let bytes = signable_bytes(&auth);
        let tag_len = "MoveAuthenticator::".len();
        let payload = &bytes[tag_len..];
        let expected_bcs = bcs::to_bytes(&auth.inner).expect("BCS serialization should not fail");
        assert_eq!(payload, expected_bcs.as_slice());
    }

    #[test]
    fn from_signable_bytes_rejects_empty() {
        assert!(MoveAuthenticator::from_signable_bytes(&[]).is_err());
    }

    #[test]
    fn from_signable_bytes_rejects_short_input() {
        // Shorter than the name tag — should fail, not panic.
        assert!(MoveAuthenticator::from_signable_bytes(b"Move").is_err());
    }

    #[test]
    fn from_signable_bytes_rejects_tag_only() {
        // Exact tag with no BCS payload.
        assert!(MoveAuthenticator::from_signable_bytes(b"MoveAuthenticator::").is_err());
    }

    #[test]
    fn from_signable_bytes_rejects_corrupt_payload() {
        let auth = make_simple_authenticator();
        let mut bytes = signable_bytes(&auth);
        // Truncate the BCS payload so it is incomplete.
        let tag_len = "MoveAuthenticator::".len();
        bytes.truncate(tag_len + 1);
        assert!(MoveAuthenticator::from_signable_bytes(&bytes).is_err());
    }

    #[test]
    fn digest_is_stable() {
        let auth = make_simple_authenticator();
        let d1 = auth.digest();
        let d2 = auth.digest();
        assert_eq!(d1, d2, "digest must be deterministic");
    }
}
