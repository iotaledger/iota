// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[path = "generated/sui.rest.rs"]
mod generated;
pub use generated::*;
use tap::Pipe;

//
// Transaction
//

impl TryFrom<&sui_sdk_types::types::Transaction> for Transaction {
    type Error = bcs::Error;

    fn try_from(value: &sui_sdk_types::types::Transaction) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            transaction: bytes.into(),
        })
    }
}

impl TryFrom<&Transaction> for sui_sdk_types::types::Transaction {
    type Error = bcs::Error;

    fn try_from(value: &Transaction) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.transaction)
    }
}

impl TryFrom<&sui_types::transaction::TransactionData> for Transaction {
    type Error = bcs::Error;

    fn try_from(value: &sui_types::transaction::TransactionData) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            transaction: bytes.into(),
        })
    }
}

impl TryFrom<&Transaction> for sui_types::transaction::TransactionData {
    type Error = bcs::Error;

    fn try_from(value: &Transaction) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.transaction)
    }
}

//
// TransactionEffects
//

impl TryFrom<&sui_sdk_types::types::TransactionEffects> for TransactionEffects {
    type Error = bcs::Error;

    fn try_from(value: &sui_sdk_types::types::TransactionEffects) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            effects: bytes.into(),
        })
    }
}

impl TryFrom<&TransactionEffects> for sui_sdk_types::types::TransactionEffects {
    type Error = bcs::Error;

    fn try_from(value: &TransactionEffects) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.effects)
    }
}

impl TryFrom<&sui_types::effects::TransactionEffects> for TransactionEffects {
    type Error = bcs::Error;

    fn try_from(value: &sui_types::effects::TransactionEffects) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            effects: bytes.into(),
        })
    }
}

impl TryFrom<&TransactionEffects> for sui_types::effects::TransactionEffects {
    type Error = bcs::Error;

    fn try_from(value: &TransactionEffects) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.effects)
    }
}

//
// TransactionEvents
//

impl TryFrom<&sui_sdk_types::types::TransactionEvents> for TransactionEvents {
    type Error = bcs::Error;

    fn try_from(value: &sui_sdk_types::types::TransactionEvents) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            events: bytes.into(),
        })
    }
}

impl TryFrom<&TransactionEvents> for sui_sdk_types::types::TransactionEvents {
    type Error = bcs::Error;

    fn try_from(value: &TransactionEvents) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.events)
    }
}

impl TryFrom<&sui_types::effects::TransactionEvents> for TransactionEvents {
    type Error = bcs::Error;

    fn try_from(value: &sui_types::effects::TransactionEvents) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            events: bytes.into(),
        })
    }
}

impl TryFrom<&TransactionEvents> for sui_types::effects::TransactionEvents {
    type Error = bcs::Error;

    fn try_from(value: &TransactionEvents) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.events)
    }
}

//
// Object
//

impl TryFrom<&sui_sdk_types::types::Object> for Object {
    type Error = bcs::Error;

    fn try_from(value: &sui_sdk_types::types::Object) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            object: bytes.into(),
        })
    }
}

impl TryFrom<&Object> for sui_sdk_types::types::Object {
    type Error = bcs::Error;

    fn try_from(value: &Object) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.object)
    }
}

impl TryFrom<&sui_types::object::Object> for Object {
    type Error = bcs::Error;

    fn try_from(value: &sui_types::object::Object) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            object: bytes.into(),
        })
    }
}

impl TryFrom<&Object> for sui_types::object::Object {
    type Error = bcs::Error;

    fn try_from(value: &Object) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.object)
    }
}

//
// CheckpointSummary
//

impl TryFrom<&sui_sdk_types::types::CheckpointSummary> for CheckpointSummary {
    type Error = bcs::Error;

    fn try_from(value: &sui_sdk_types::types::CheckpointSummary) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            summary: bytes.into(),
        })
    }
}

impl TryFrom<&CheckpointSummary> for sui_sdk_types::types::CheckpointSummary {
    type Error = bcs::Error;

    fn try_from(value: &CheckpointSummary) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.summary)
    }
}

impl TryFrom<&sui_types::messages_checkpoint::CheckpointSummary> for CheckpointSummary {
    type Error = bcs::Error;

    fn try_from(
        value: &sui_types::messages_checkpoint::CheckpointSummary,
    ) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            summary: bytes.into(),
        })
    }
}

impl TryFrom<&CheckpointSummary> for sui_types::messages_checkpoint::CheckpointSummary {
    type Error = bcs::Error;

    fn try_from(value: &CheckpointSummary) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.summary)
    }
}

//
// CheckpointContents
//

impl TryFrom<&sui_sdk_types::types::CheckpointContents> for CheckpointContents {
    type Error = bcs::Error;

    fn try_from(value: &sui_sdk_types::types::CheckpointContents) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            contents: bytes.into(),
        })
    }
}

impl TryFrom<&CheckpointContents> for sui_sdk_types::types::CheckpointContents {
    type Error = bcs::Error;

    fn try_from(value: &CheckpointContents) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.contents)
    }
}

impl TryFrom<&sui_types::messages_checkpoint::CheckpointContents> for CheckpointContents {
    type Error = bcs::Error;

    fn try_from(
        value: &sui_types::messages_checkpoint::CheckpointContents,
    ) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            contents: bytes.into(),
        })
    }
}

impl TryFrom<&CheckpointContents> for sui_types::messages_checkpoint::CheckpointContents {
    type Error = bcs::Error;

    fn try_from(value: &CheckpointContents) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.contents)
    }
}

//
// ValidatorAggregatedSignature
//

impl TryFrom<&sui_sdk_types::types::ValidatorAggregatedSignature> for ValidatorAggregatedSignature {
    type Error = bcs::Error;

    fn try_from(
        value: &sui_sdk_types::types::ValidatorAggregatedSignature,
    ) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            signature: bytes.into(),
        })
    }
}

impl TryFrom<&ValidatorAggregatedSignature> for sui_sdk_types::types::ValidatorAggregatedSignature {
    type Error = bcs::Error;

    fn try_from(value: &ValidatorAggregatedSignature) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.signature)
    }
}

impl TryFrom<&sui_types::crypto::AuthorityStrongQuorumSignInfo> for ValidatorAggregatedSignature {
    type Error = bcs::Error;

    fn try_from(
        value: &sui_types::crypto::AuthorityStrongQuorumSignInfo,
    ) -> Result<Self, Self::Error> {
        bcs::to_bytes(&value).map(|bytes| Self {
            signature: bytes.into(),
        })
    }
}

impl TryFrom<&ValidatorAggregatedSignature> for sui_types::crypto::AuthorityStrongQuorumSignInfo {
    type Error = bcs::Error;

    fn try_from(value: &ValidatorAggregatedSignature) -> Result<Self, Self::Error> {
        bcs::from_bytes(&value.signature)
    }
}

//
// UserSignature
//

impl TryFrom<&sui_sdk_types::types::UserSignature> for UserSignature {
    type Error = bcs::Error;

    fn try_from(value: &sui_sdk_types::types::UserSignature) -> Result<Self, Self::Error> {
        Ok(Self {
            signature: value.to_bytes().into(),
        })
    }
}

impl TryFrom<&UserSignature> for sui_sdk_types::types::UserSignature {
    type Error = bcs::Error;

    fn try_from(value: &UserSignature) -> Result<Self, Self::Error> {
        Self::from_bytes(&value.signature).map_err(|e| bcs::Error::Custom(e.to_string()))
    }
}

impl TryFrom<&sui_types::signature::GenericSignature> for UserSignature {
    type Error = bcs::Error;

    fn try_from(value: &sui_types::signature::GenericSignature) -> Result<Self, Self::Error> {
        Ok(Self {
            signature: sui_types::crypto::ToFromBytes::as_bytes(value)
                .to_vec()
                .into(),
        })
    }
}

impl TryFrom<&UserSignature> for sui_types::signature::GenericSignature {
    type Error = bcs::Error;

    fn try_from(value: &UserSignature) -> Result<Self, Self::Error> {
        sui_types::crypto::ToFromBytes::from_bytes(&value.signature)
            .map_err(|e| bcs::Error::Custom(e.to_string()))
    }
}

//
// CheckpointTransaction
//

impl TryFrom<sui_types::full_checkpoint_content::CheckpointTransaction> for CheckpointTransaction {
    type Error = bcs::Error;

    fn try_from(
        transaction: sui_types::full_checkpoint_content::CheckpointTransaction,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            transaction: Some(Transaction::try_from(
                &transaction.transaction.intent_message().value,
            )?),
            signatures: transaction
                .transaction
                .tx_signatures()
                .iter()
                .map(UserSignature::try_from)
                .collect::<Result<_, _>>()?,
            effects: Some(TransactionEffects::try_from(&transaction.effects)?),
            events: transaction
                .events
                .as_ref()
                .map(TransactionEvents::try_from)
                .transpose()?,
            input_objects: transaction
                .input_objects
                .iter()
                .map(Object::try_from)
                .collect::<Result<_, _>>()?,
            output_objects: transaction
                .output_objects
                .iter()
                .map(Object::try_from)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<CheckpointTransaction> for sui_types::full_checkpoint_content::CheckpointTransaction {
    type Error = bcs::Error;

    fn try_from(transaction: CheckpointTransaction) -> Result<Self, Self::Error> {
        let transaction_data = transaction
            .transaction
            .ok_or_else(|| bcs::Error::Custom("missing transaction".into()))?
            .pipe_ref(TryInto::try_into)?;
        let user_signatures = transaction
            .signatures
            .iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;

        Ok(Self {
            transaction: sui_types::transaction::Transaction::new(
                sui_types::transaction::SenderSignedData::new(transaction_data, user_signatures),
            ),
            effects: transaction
                .effects
                .ok_or_else(|| bcs::Error::Custom("missing Effects".into()))?
                .pipe_ref(TryInto::try_into)?,
            events: transaction
                .events
                .as_ref()
                .map(TryInto::try_into)
                .transpose()?,
            input_objects: transaction
                .input_objects
                .iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            output_objects: transaction
                .output_objects
                .iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

//
// FullCheckpoint
//

impl TryFrom<sui_types::full_checkpoint_content::CheckpointData> for FullCheckpoint {
    type Error = bcs::Error;

    fn try_from(
        c: sui_types::full_checkpoint_content::CheckpointData,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            summary: Some(CheckpointSummary::try_from(c.checkpoint_summary.data())?),
            signature: Some(ValidatorAggregatedSignature::try_from(
                c.checkpoint_summary.auth_sig(),
            )?),
            contents: Some(CheckpointContents::try_from(&c.checkpoint_contents)?),
            transactions: c
                .transactions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<FullCheckpoint> for sui_types::full_checkpoint_content::CheckpointData {
    type Error = bcs::Error;

    fn try_from(checkpoint: FullCheckpoint) -> Result<Self, Self::Error> {
        let summary = checkpoint
            .summary
            .ok_or_else(|| bcs::Error::Custom("missing summary".into()))?
            .pipe_ref(TryInto::try_into)?;
        let signature = checkpoint
            .signature
            .ok_or_else(|| bcs::Error::Custom("missing signature".into()))?
            .pipe_ref(TryInto::try_into)?;
        let checkpoint_summary =
            sui_types::messages_checkpoint::CertifiedCheckpointSummary::new_from_data_and_sig(
                summary, signature,
            );

        let contents = checkpoint
            .contents
            .ok_or_else(|| bcs::Error::Custom("missing checkpoint contents".into()))?
            .pipe_ref(TryInto::try_into)?;

        let transactions = checkpoint
            .transactions
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;

        Ok(Self {
            checkpoint_summary,
            checkpoint_contents: contents,
            transactions,
        })
    }
}

//
// Address
//

impl From<&sui_sdk_types::types::Address> for Address {
    fn from(value: &sui_sdk_types::types::Address) -> Self {
        Self {
            address: value.as_bytes().to_vec().into(),
        }
    }
}

impl TryFrom<&Address> for sui_sdk_types::types::Address {
    type Error = bcs::Error;

    fn try_from(value: &Address) -> Result<Self, Self::Error> {
        Self::from_bytes(&value.address).map_err(|e| bcs::Error::Custom(e.to_string()))
    }
}

impl TryFrom<&Address> for sui_types::base_types::SuiAddress {
    type Error = bcs::Error;

    fn try_from(value: &Address) -> Result<Self, Self::Error> {
        Self::from_bytes(&value.address).map_err(|e| bcs::Error::Custom(e.to_string()))
    }
}

//
// TypeTag
//

impl From<&sui_sdk_types::types::TypeTag> for TypeTag {
    fn from(value: &sui_sdk_types::types::TypeTag) -> Self {
        Self {
            type_tag: value.to_string(),
        }
    }
}

impl TryFrom<&TypeTag> for sui_sdk_types::types::TypeTag {
    type Error = sui_sdk_types::types::TypeParseError;

    fn try_from(value: &TypeTag) -> Result<Self, Self::Error> {
        value.type_tag.parse()
    }
}

impl TryFrom<&TypeTag> for sui_types::TypeTag {
    type Error = bcs::Error;

    fn try_from(value: &TypeTag) -> Result<Self, Self::Error> {
        value
            .type_tag
            .parse::<sui_types::TypeTag>()
            .map_err(|e| bcs::Error::Custom(e.to_string()))
    }
}

//
// I128
//

impl From<i128> for I128 {
    fn from(value: i128) -> Self {
        Self {
            little_endian_bytes: value.to_le_bytes().to_vec().into(),
        }
    }
}

impl TryFrom<&I128> for i128 {
    type Error = std::array::TryFromSliceError;

    fn try_from(value: &I128) -> Result<Self, Self::Error> {
        Ok(i128::from_le_bytes(
            value.little_endian_bytes.as_ref().try_into()?,
        ))
    }
}

//
// ValidatorCommitteeMember
//

impl From<&sui_sdk_types::types::ValidatorCommitteeMember> for ValidatorCommitteeMember {
    fn from(value: &sui_sdk_types::types::ValidatorCommitteeMember) -> Self {
        Self {
            public_key: value.public_key.as_bytes().to_vec().into(),
            stake: value.stake,
        }
    }
}

impl TryFrom<ValidatorCommitteeMember> for sui_sdk_types::types::ValidatorCommitteeMember {
    type Error = bcs::Error;

    fn try_from(value: ValidatorCommitteeMember) -> Result<Self, Self::Error> {
        Ok(Self {
            public_key: sui_sdk_types::types::Bls12381PublicKey::from_bytes(&value.public_key)
                .map_err(|e| bcs::Error::Custom(e.to_string()))?,
            stake: value.stake,
        })
    }
}

//
// ValidatorCommittee
//

impl From<sui_sdk_types::types::ValidatorCommittee> for ValidatorCommittee {
    fn from(value: sui_sdk_types::types::ValidatorCommittee) -> Self {
        Self {
            epoch: value.epoch,
            members: value
                .members
                .iter()
                .map(ValidatorCommitteeMember::from)
                .collect(),
        }
    }
}

impl TryFrom<ValidatorCommittee> for sui_sdk_types::types::ValidatorCommittee {
    type Error = bcs::Error;

    fn try_from(value: ValidatorCommittee) -> Result<Self, Self::Error> {
        Ok(Self {
            epoch: value.epoch,
            members: value
                .members
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}
