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
    checkpoint::{
        CheckpointCommitment, CheckpointContents, CheckpointData, CheckpointSummary,
        CheckpointTransaction, CheckpointTransactionInfo, EndOfEpochData, SignedCheckpointSummary,
    },
    crypto::{Bls12381PublicKey, Bls12381Signature, UserSignature},
    digest::Digest,
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

impl TryFrom<crate::messages_checkpoint::CheckpointContents> for CheckpointContents {
    type Error = SdkTypeConversionError;

    fn try_from(
        value: crate::messages_checkpoint::CheckpointContents,
    ) -> Result<Self, Self::Error> {
        Self(
            value
                .into_iter_with_signatures()
                .map(|(digests, signatures)| {
                    let signatures_result = signatures
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<Vec<UserSignature>, _>>();

                    match signatures_result {
                        Ok(signatures) => Ok(CheckpointTransactionInfo {
                            transaction: digests.transaction,
                            effects: digests.effects,
                            signatures,
                        }),
                        Err(e) => Err(SdkTypeConversionError::from(e)),
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
        .pipe(Ok)
    }
}

impl TryFrom<CheckpointContents> for crate::messages_checkpoint::CheckpointContents {
    type Error = SdkTypeConversionError;

    fn try_from(value: CheckpointContents) -> Result<Self, Self::Error> {
        let (transactions, user_signatures) = value.0.into_iter().fold(
            (Vec::new(), Vec::new()),
            |(mut transactions, mut user_signatures), info| {
                transactions.push(crate::base_types::ExecutionDigests {
                    transaction: info.transaction,
                    effects: info.effects,
                });
                user_signatures.push(
                    info.signatures
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<_, _>>(),
                );
                (transactions, user_signatures)
            },
        );
        crate::messages_checkpoint::CheckpointContents::new_with_digests_and_signatures(
            transactions,
            user_signatures.into_iter().collect::<Result<Vec<_>, _>>()?,
        )
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
            checkpoint_contents: value.checkpoint_contents.try_into()?,
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
            checkpoint_contents: value.checkpoint_contents.try_into()?,
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

impl TryFrom<crate::signature::GenericSignature> for UserSignature {
    type Error = bcs::Error;

    fn try_from(value: crate::signature::GenericSignature) -> Result<Self, Self::Error> {
        bcs::from_bytes(&bcs::to_bytes(&value)?)
    }
}

impl TryFrom<UserSignature> for crate::signature::GenericSignature {
    type Error = bcs::Error;

    fn try_from(value: UserSignature) -> Result<Self, Self::Error> {
        bcs::from_bytes(&bcs::to_bytes(&value)?)
    }
}

impl From<crate::messages_checkpoint::EndOfEpochData> for EndOfEpochData {
    fn from(value: crate::messages_checkpoint::EndOfEpochData) -> Self {
        Self {
            next_epoch_committee: value
                .next_epoch_committee
                .into_iter()
                .map(|(public_key, stake)| ValidatorCommitteeMember {
                    public_key: Bls12381PublicKey::new(public_key.0),
                    stake,
                })
                .collect(),
            next_epoch_protocol_version: value.next_epoch_protocol_version.as_u64(),
            epoch_commitments: value
                .epoch_commitments
                .into_iter()
                .map(Into::into)
                .collect(),
            epoch_supply_change: value.epoch_supply_change,
        }
    }
}

impl From<EndOfEpochData> for crate::messages_checkpoint::EndOfEpochData {
    fn from(value: EndOfEpochData) -> Self {
        Self {
            next_epoch_committee: value
                .next_epoch_committee
                .into_iter()
                .map(|v| (v.public_key.into(), v.stake))
                .collect(),
            next_epoch_protocol_version: value.next_epoch_protocol_version.into(),
            epoch_commitments: value
                .epoch_commitments
                .into_iter()
                .map(Into::into)
                .collect(),
            epoch_supply_change: value.epoch_supply_change,
        }
    }
}

impl From<crate::messages_checkpoint::CheckpointCommitment> for CheckpointCommitment {
    fn from(value: crate::messages_checkpoint::CheckpointCommitment) -> Self {
        let crate::messages_checkpoint::CheckpointCommitment::ECMHLiveObjectSetDigest(digest) =
            value;
        Self::EcmhLiveObjectSet {
            digest: Digest::new(digest.digest.into_inner()),
        }
    }
}

impl From<CheckpointCommitment> for crate::messages_checkpoint::CheckpointCommitment {
    fn from(value: CheckpointCommitment) -> Self {
        match value {
            CheckpointCommitment::EcmhLiveObjectSet { digest } => {
                Self::ECMHLiveObjectSetDigest(crate::messages_checkpoint::ECMHLiveObjectSetDigest {
                    digest: crate::digests::Digest::new(digest.into_inner()),
                })
            }
            _ => unimplemented!(
                "a new CheckpointCommitment enum variant was added and needs to be handled"
            ),
        }
    }
}

impl TryFrom<crate::messages_checkpoint::CheckpointSummary> for CheckpointSummary {
    type Error = SdkTypeConversionError;

    fn try_from(value: crate::messages_checkpoint::CheckpointSummary) -> Result<Self, Self::Error> {
        Self {
            epoch: value.epoch,
            sequence_number: value.sequence_number,
            network_total_transactions: value.network_total_transactions,
            content_digest: value.content_digest,
            previous_digest: value.previous_digest,
            epoch_rolling_gas_cost_summary: value.epoch_rolling_gas_cost_summary,
            timestamp_ms: value.timestamp_ms,
            checkpoint_commitments: value
                .checkpoint_commitments
                .into_iter()
                .map(Into::into)
                .collect(),
            end_of_epoch_data: value.end_of_epoch_data.map(Into::into),
            version_specific_data: value.version_specific_data,
        }
        .pipe(Ok)
    }
}

impl TryFrom<CheckpointSummary> for crate::messages_checkpoint::CheckpointSummary {
    type Error = SdkTypeConversionError;

    fn try_from(value: CheckpointSummary) -> Result<Self, Self::Error> {
        Self {
            epoch: value.epoch,
            sequence_number: value.sequence_number,
            network_total_transactions: value.network_total_transactions,
            content_digest: value.content_digest,
            previous_digest: value.previous_digest,
            epoch_rolling_gas_cost_summary: value.epoch_rolling_gas_cost_summary,
            timestamp_ms: value.timestamp_ms,
            checkpoint_commitments: value
                .checkpoint_commitments
                .into_iter()
                .map(Into::into)
                .collect(),
            end_of_epoch_data: value.end_of_epoch_data.map(Into::into),
            version_specific_data: value.version_specific_data,
        }
        .pipe(Ok)
    }
}

impl TryFrom<crate::messages_checkpoint::CertifiedCheckpointSummary> for SignedCheckpointSummary {
    type Error = SdkTypeConversionError;

    fn try_from(
        value: crate::messages_checkpoint::CertifiedCheckpointSummary,
    ) -> Result<Self, Self::Error> {
        let (data, sig) = value.into_data_and_sig();
        Self {
            checkpoint: data.try_into()?,
            signature: sig.into(),
        }
        .pipe(Ok)
    }
}

impl TryFrom<SignedCheckpointSummary> for crate::messages_checkpoint::CertifiedCheckpointSummary {
    type Error = SdkTypeConversionError;

    fn try_from(value: SignedCheckpointSummary) -> Result<Self, Self::Error> {
        Self::new_from_data_and_sig(
            crate::messages_checkpoint::CheckpointSummary::try_from(value.checkpoint)?,
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
            signatures: tx_signatures
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
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

        Self::new(
            transaction,
            signatures
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        )
        .pipe(Ok)
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

pub fn struct_tag_core_to_sdk(value: &move_core_types::language_storage::StructTag) -> StructTag {
    let move_core_types::language_storage::StructTag {
        address,
        module,
        name,
        type_params,
    } = value;

    let address = Address::new(address.into_bytes());
    let module = Identifier::new_unchecked(module.as_str());
    let name = Identifier::new_unchecked(name.as_str());
    let type_params = type_params.iter().map(type_tag_core_to_sdk).collect();
    StructTag::new(address, module, name, type_params)
}

pub fn struct_tag_sdk_to_core(value: &StructTag) -> move_core_types::language_storage::StructTag {
    let address =
        move_core_types::account_address::AccountAddress::new(value.address().into_bytes());
    let module = move_core_types::identifier::Identifier::new(value.module().as_str()).unwrap();
    let name = move_core_types::identifier::Identifier::new(value.name().as_str()).unwrap();
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
