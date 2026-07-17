// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Module for conversions between iota-core types and iota-sdk types
//!
//! For now this module makes heavy use of the `bcs_convert_impl` macro to
//! implement the `From` trait for converting between core and external sdk
//! types, relying on the fact that the BCS format of these types are strictly
//! identical. As time goes on we'll slowly hand implement these impls
//! directly to avoid going through the BCS machinery.

use fastcrypto::traits::ToFromBytes;
use iota_sdk_types::{
    address::Address,
    checkpoint::{CheckpointData, CheckpointTransaction, SignedCheckpointSummary},
    crypto::{Bls12381PublicKey, Bls12381Signature},
    move_core::{Identifier, StructTag, TypeParseError, TypeTag},
    object::Object,
    transaction::SignedTransaction,
    validator::{ValidatorAggregatedSignature, ValidatorCommittee, ValidatorCommitteeMember},
};
use tap::Pipe;

#[derive(Debug)]
pub struct SdkTypeConversionError(pub String);

impl std::fmt::Display for SdkTypeConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SdkTypeConversionError {}

impl From<TypeParseError> for SdkTypeConversionError {
    fn from(value: TypeParseError) -> Self {
        Self(value.to_string())
    }
}

impl From<anyhow::Error> for SdkTypeConversionError {
    fn from(value: anyhow::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<bcs::Error> for SdkTypeConversionError {
    fn from(value: bcs::Error) -> Self {
        Self(value.to_string())
    }
}

impl TryFrom<crate::object::Object> for Object {
    type Error = SdkTypeConversionError;

    fn try_from(value: crate::object::Object) -> Result<Self, Self::Error> {
        Self {
            data: value.data.clone(),
            owner: value.owner,
            previous_transaction: value.previous_transaction,
            storage_rebate: value.storage_rebate,
        }
        .pipe(Ok)
    }
}

impl TryFrom<crate::full_checkpoint_content::CheckpointData> for CheckpointData {
    type Error = SdkTypeConversionError;

    fn try_from(
        value: crate::full_checkpoint_content::CheckpointData,
    ) -> Result<Self, Self::Error> {
        Self {
            checkpoint_summary: value.checkpoint_summary.try_into()?,
            checkpoint_contents: value.checkpoint_contents,
            transactions: value
                .transactions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        }
        .pipe(Ok)
    }
}

impl TryFrom<CheckpointData> for crate::full_checkpoint_content::CheckpointData {
    type Error = SdkTypeConversionError;

    fn try_from(value: CheckpointData) -> Result<Self, Self::Error> {
        Self {
            checkpoint_summary: value.checkpoint_summary.try_into()?,
            checkpoint_contents: value.checkpoint_contents,
            transactions: value
                .transactions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        }
        .pipe(Ok)
    }
}

impl TryFrom<crate::full_checkpoint_content::CheckpointTransaction> for CheckpointTransaction {
    type Error = SdkTypeConversionError;

    fn try_from(
        value: crate::full_checkpoint_content::CheckpointTransaction,
    ) -> Result<Self, Self::Error> {
        let input_objects = value
            .input_objects
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>();
        let output_objects = value
            .output_objects
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>();
        match (input_objects, output_objects) {
            (Ok(input_objects), Ok(output_objects)) => Ok(Self {
                transaction: value.transaction.try_into()?,
                effects: value.effects,
                events: value.events,
                input_objects,
                output_objects,
            }),
            (Err(e), _) | (_, Err(e)) => Err(e),
        }
    }
}

impl TryFrom<CheckpointTransaction> for crate::full_checkpoint_content::CheckpointTransaction {
    type Error = SdkTypeConversionError;

    fn try_from(value: CheckpointTransaction) -> Result<Self, Self::Error> {
        let input_objects = value
            .input_objects
            .into_iter()
            .map(crate::object::Object::from)
            .collect();
        let output_objects = value
            .output_objects
            .into_iter()
            .map(crate::object::Object::from)
            .collect();

        Ok(Self {
            transaction: value.transaction.try_into()?,
            effects: value.effects,
            events: value.events,
            input_objects,
            output_objects,
        })
    }
}

impl TryFrom<crate::messages_checkpoint::CertifiedCheckpointSummary> for SignedCheckpointSummary {
    type Error = SdkTypeConversionError;

    fn try_from(
        value: crate::messages_checkpoint::CertifiedCheckpointSummary,
    ) -> Result<Self, Self::Error> {
        let (data, sig) = value.into_data_and_sig();
        Self {
            checkpoint: data,
            signature: sig.into(),
        }
        .pipe(Ok)
    }
}

impl TryFrom<SignedCheckpointSummary> for crate::messages_checkpoint::CertifiedCheckpointSummary {
    type Error = SdkTypeConversionError;

    fn try_from(value: SignedCheckpointSummary) -> Result<Self, Self::Error> {
        Self::new_from_data_and_sig(
            value.checkpoint,
            crate::crypto::AuthorityQuorumSignInfo::<true>::from(value.signature),
        )
        .pipe(Ok)
    }
}

impl<const T: bool> From<crate::crypto::AuthorityQuorumSignInfo<T>>
    for ValidatorAggregatedSignature
{
    fn from(value: crate::crypto::AuthorityQuorumSignInfo<T>) -> Self {
        let crate::crypto::AuthorityQuorumSignInfo {
            epoch,
            signature,
            signers_map,
        } = value;

        Self {
            epoch,
            signature: Bls12381Signature::from_bytes(signature.as_ref()).unwrap(),
            bitmap: signers_map,
        }
    }
}

impl<const T: bool> From<ValidatorAggregatedSignature>
    for crate::crypto::AuthorityQuorumSignInfo<T>
{
    fn from(value: ValidatorAggregatedSignature) -> Self {
        let ValidatorAggregatedSignature {
            epoch,
            signature,
            bitmap,
        } = value;

        Self {
            epoch,
            signature: crate::crypto::AggregateAuthoritySignature::from_bytes(signature.as_bytes())
                .unwrap(),
            signers_map: bitmap,
        }
    }
}

impl TryFrom<crate::transaction::SenderSignedData> for SignedTransaction {
    type Error = SdkTypeConversionError;

    fn try_from(value: crate::transaction::SenderSignedData) -> Result<Self, Self::Error> {
        let crate::transaction::SenderSignedTransaction {
            intent_message,
            tx_signatures,
        } = value.into_inner();

        Self {
            transaction: intent_message.value,
            signatures: tx_signatures,
        }
        .pipe(Ok)
    }
}

impl TryFrom<SignedTransaction> for crate::transaction::SenderSignedData {
    type Error = SdkTypeConversionError;

    fn try_from(value: SignedTransaction) -> Result<Self, Self::Error> {
        let SignedTransaction {
            transaction,
            signatures,
        } = value;

        Self::new(transaction, signatures).pipe(Ok)
    }
}

impl TryFrom<crate::transaction::Transaction> for SignedTransaction {
    type Error = SdkTypeConversionError;

    fn try_from(value: crate::transaction::Transaction) -> Result<Self, Self::Error> {
        value.into_data().try_into()
    }
}

impl TryFrom<SignedTransaction> for crate::transaction::Transaction {
    type Error = SdkTypeConversionError;

    fn try_from(value: SignedTransaction) -> Result<Self, Self::Error> {
        Ok(Self::new(value.try_into()?))
    }
}

pub fn type_tag_core_to_sdk(value: &move_core_types::language_storage::TypeTag) -> TypeTag {
    match value {
        move_core_types::language_storage::TypeTag::Bool => TypeTag::Bool,
        move_core_types::language_storage::TypeTag::U8 => TypeTag::U8,
        move_core_types::language_storage::TypeTag::U64 => TypeTag::U64,
        move_core_types::language_storage::TypeTag::U128 => TypeTag::U128,
        move_core_types::language_storage::TypeTag::Address => TypeTag::Address,
        move_core_types::language_storage::TypeTag::Signer => TypeTag::Signer,
        move_core_types::language_storage::TypeTag::Vector(type_tag) => {
            TypeTag::Vector(Box::new(type_tag_core_to_sdk(type_tag)))
        }
        move_core_types::language_storage::TypeTag::Struct(struct_tag) => {
            TypeTag::Struct(Box::new(struct_tag_core_to_sdk(struct_tag)))
        }
        move_core_types::language_storage::TypeTag::U16 => TypeTag::U16,
        move_core_types::language_storage::TypeTag::U32 => TypeTag::U32,
        move_core_types::language_storage::TypeTag::U256 => TypeTag::U256,
    }
}

pub fn type_tag_sdk_to_core(value: &TypeTag) -> move_core_types::language_storage::TypeTag {
    match value {
        TypeTag::Bool => move_core_types::language_storage::TypeTag::Bool,
        TypeTag::U8 => move_core_types::language_storage::TypeTag::U8,
        TypeTag::U64 => move_core_types::language_storage::TypeTag::U64,
        TypeTag::U128 => move_core_types::language_storage::TypeTag::U128,
        TypeTag::Address => move_core_types::language_storage::TypeTag::Address,
        TypeTag::Signer => move_core_types::language_storage::TypeTag::Signer,
        TypeTag::Vector(type_tag) => move_core_types::language_storage::TypeTag::Vector(Box::new(
            type_tag_sdk_to_core(type_tag),
        )),
        TypeTag::Struct(struct_tag) => move_core_types::language_storage::TypeTag::Struct(
            Box::new(struct_tag_sdk_to_core(struct_tag)),
        ),
        TypeTag::U16 => move_core_types::language_storage::TypeTag::U16,
        TypeTag::U32 => move_core_types::language_storage::TypeTag::U32,
        TypeTag::U256 => move_core_types::language_storage::TypeTag::U256,
    }
}

pub fn identifier_core_to_sdk(value: &move_core_types::identifier::IdentStr) -> Identifier {
    Identifier::new_unchecked(value.as_str())
}

pub fn identifier_sdk_to_core(value: &Identifier) -> move_core_types::identifier::Identifier {
    // SAFETY: an SDK `Identifier` is an already-validated Move identifier; preserve
    // it verbatim without re-validation.
    unsafe { move_core_types::identifier::Identifier::new_unchecked(value.as_str()) }
}

pub fn struct_tag_core_to_sdk(value: &move_core_types::language_storage::StructTag) -> StructTag {
    let move_core_types::language_storage::StructTag {
        address,
        module,
        name,
        type_params,
    } = value;

    let address = Address::new(address.into_bytes());
    let module = identifier_core_to_sdk(module);
    let name = identifier_core_to_sdk(name);
    let type_params = type_params.iter().map(type_tag_core_to_sdk).collect();
    StructTag::new(address, module, name, type_params)
}

pub fn struct_tag_sdk_to_core(value: &StructTag) -> move_core_types::language_storage::StructTag {
    let address =
        move_core_types::account_address::AccountAddress::new(value.address().into_bytes());
    let module = identifier_sdk_to_core(value.module());
    let name = identifier_sdk_to_core(value.name());
    let type_params = value
        .type_params()
        .iter()
        .map(type_tag_sdk_to_core)
        .collect();
    move_core_types::language_storage::StructTag {
        address,
        module,
        name,
        type_params,
    }
}

impl From<crate::committee::Committee> for ValidatorCommittee {
    fn from(value: crate::committee::Committee) -> Self {
        Self {
            epoch: value.epoch(),
            members: value
                .voting_rights
                .into_iter()
                .map(|(name, stake)| ValidatorCommitteeMember {
                    public_key: name.into(),
                    stake,
                })
                .collect(),
        }
    }
}

impl From<ValidatorCommittee> for crate::committee::Committee {
    fn from(value: ValidatorCommittee) -> Self {
        let ValidatorCommittee { epoch, members } = value;

        Self::new(
            epoch,
            members
                .into_iter()
                .map(|member| (member.public_key.into(), member.stake))
                .collect(),
        )
    }
}

impl From<crate::crypto::AuthorityPublicKeyBytes> for Bls12381PublicKey {
    fn from(value: crate::crypto::AuthorityPublicKeyBytes) -> Self {
        Self::new(value.0)
    }
}

impl From<Bls12381PublicKey> for crate::crypto::AuthorityPublicKeyBytes {
    fn from(value: Bls12381PublicKey) -> Self {
        Self::new(value.into_inner())
    }
}
