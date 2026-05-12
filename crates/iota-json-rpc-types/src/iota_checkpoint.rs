// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolVersion;
use iota_types::{
    base_types::{AuthorityName, TransactionDigest},
    committee::{EpochId, StakeUnit},
    crypto::AggregateAuthoritySignature,
    digests::{CheckpointDigest, Digest},
    gas::GasCostSummary,
    iota_serde::BigInt,
    message_envelope::Message,
    messages_checkpoint::{
        CheckpointCommitment, CheckpointContents, CheckpointSequenceNumber, CheckpointSummary,
        CheckpointTimestamp, ECMHLiveObjectSetDigest, EndOfEpochData,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeAs, DisplayFromStr, SerializeAs, serde_as};

use crate::{
    IotaAuthorityPublicKeyBytes, Page,
    iota_gas_cost_summary::IotaGasCostSummary,
    iota_primitives::{
        Base58 as Base58Schema, Base64 as Base64Schema, ProtocolVersion as ProtocolVersionSchema,
    },
};
pub type CheckpointPage = Page<Checkpoint, BigInt<u64>>;

#[serde_as]
#[derive(Clone, Debug, JsonSchema, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    /// Checkpoint's epoch ID
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub epoch: EpochId,
    /// Checkpoint sequence number
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub sequence_number: CheckpointSequenceNumber,
    /// Checkpoint digest
    #[schemars(with = "Base58Schema")]
    pub digest: CheckpointDigest,
    /// Total number of transactions committed since genesis, including those in
    /// this checkpoint.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub network_total_transactions: u64,
    /// Digest of the previous checkpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Base58Schema>")]
    pub previous_digest: Option<CheckpointDigest>,
    /// The running total gas costs of all transactions included in the current
    /// epoch so far until this checkpoint.
    #[schemars(with = "IotaGasCostSummary")]
    #[serde_as(as = "IotaGasCostSummary")]
    pub epoch_rolling_gas_cost_summary: GasCostSummary,
    /// Timestamp of the checkpoint - number of milliseconds from the Unix epoch
    /// Checkpoint timestamps are monotonic, but not strongly monotonic -
    /// subsequent checkpoints can have same timestamp if they originate
    /// from the same underlining consensus commit
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp_ms: CheckpointTimestamp,
    /// Present only on the final checkpoint of the epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<EndOfEpochDataSchema>")]
    #[serde_as(as = "Option<EndOfEpochDataSchema>")]
    pub end_of_epoch_data: Option<EndOfEpochData>,
    /// Transaction digests
    #[schemars(with = "Vec<Base58Schema>")]
    pub transactions: Vec<TransactionDigest>,

    /// Commitments to checkpoint state
    #[schemars(with = "Vec<CheckpointCommitmentSchema>")]
    #[serde_as(as = "Vec<CheckpointCommitmentSchema>")]
    pub checkpoint_commitments: Vec<CheckpointCommitment>,
    /// Validator Signature
    #[schemars(with = "Base64Schema")]
    pub validator_signature: AggregateAuthoritySignature,
}

impl
    From<(
        CheckpointSummary,
        CheckpointContents,
        AggregateAuthoritySignature,
    )> for Checkpoint
{
    fn from(
        (summary, contents, signature): (
            CheckpointSummary,
            CheckpointContents,
            AggregateAuthoritySignature,
        ),
    ) -> Self {
        let digest = summary.digest();
        let CheckpointSummary {
            epoch,
            sequence_number,
            network_total_transactions,
            previous_digest,
            epoch_rolling_gas_cost_summary,
            timestamp_ms,
            end_of_epoch_data,
            ..
        } = summary;

        Checkpoint {
            epoch,
            sequence_number,
            digest,
            network_total_transactions,
            previous_digest,
            epoch_rolling_gas_cost_summary,
            timestamp_ms,
            end_of_epoch_data,
            transactions: contents.iter().map(|digest| digest.transaction).collect(),
            // TODO: populate commitment for rpc clients. Most likely, rpc clients don't need this
            // info (if they need it, they need to get signed BCS data anyway in order to trust
            // it).
            checkpoint_commitments: Default::default(),
            validator_signature: signature,
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", rename = "EndOfEpochData")]
pub struct EndOfEpochDataSchema {
    /// next_epoch_committee is `Some` if and only if the current checkpoint is
    /// the last checkpoint of an epoch.
    /// Therefore next_epoch_committee can be used to pick the last checkpoint
    /// of an epoch, which is often useful to get epoch level summary stats
    /// like total gas cost of an epoch, or the total number of transactions
    /// from genesis to the end of an epoch. The committee is stored as a
    /// vector of validator pub key and stake pairs. The vector
    /// should be sorted based on the Committee data structure.
    #[schemars(with = "Vec<(IotaAuthorityPublicKeyBytes, String)>")]
    #[serde_as(as = "Vec<(_, DisplayFromStr)>")]
    pub next_epoch_committee: Vec<(AuthorityName, StakeUnit)>,

    /// The protocol version that is in effect during the epoch that starts
    /// immediately after this checkpoint.
    #[schemars(with = "ProtocolVersionSchema")]
    #[serde_as(as = "ProtocolVersionSchema")]
    pub next_epoch_protocol_version: ProtocolVersion,

    /// Commitments to epoch specific state (e.g. live object set)
    pub epoch_commitments: Vec<CheckpointCommitmentSchema>,

    /// The number of tokens that were minted (if positive) or burnt (if
    /// negative) in this epoch.
    pub epoch_supply_change: i64,
}

impl SerializeAs<EndOfEpochData> for EndOfEpochDataSchema {
    fn serialize_as<S>(source: &EndOfEpochData, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let iota_data = EndOfEpochDataSchema::from(source.clone());
        iota_data.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, EndOfEpochData> for EndOfEpochDataSchema {
    fn deserialize_as<D>(deserializer: D) -> Result<EndOfEpochData, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let iota_data = EndOfEpochDataSchema::deserialize(deserializer)?;
        Ok(iota_data.into())
    }
}

impl From<EndOfEpochDataSchema> for EndOfEpochData {
    fn from(iota_data: EndOfEpochDataSchema) -> Self {
        let EndOfEpochDataSchema {
            next_epoch_committee,
            next_epoch_protocol_version,
            epoch_commitments,
            epoch_supply_change,
        } = iota_data;
        EndOfEpochData {
            next_epoch_committee: next_epoch_committee.into_iter().collect(),
            next_epoch_protocol_version,
            epoch_commitments: epoch_commitments.into_iter().map(Into::into).collect(),
            epoch_supply_change,
        }
    }
}

impl From<EndOfEpochData> for EndOfEpochDataSchema {
    fn from(data: EndOfEpochData) -> Self {
        let EndOfEpochData {
            next_epoch_committee,
            next_epoch_protocol_version,
            epoch_commitments,
            epoch_supply_change,
        } = data;
        EndOfEpochDataSchema {
            next_epoch_committee: next_epoch_committee.into_iter().collect(),
            next_epoch_protocol_version,
            epoch_commitments: epoch_commitments.into_iter().map(Into::into).collect(),
            epoch_supply_change,
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CheckpointCommitment")]
pub enum CheckpointCommitmentSchema {
    ECMHLiveObjectSetDigest(
        #[schemars(with = "ECMHLiveObjectSetDigestSchema")]
        #[serde_as(as = "ECMHLiveObjectSetDigestSchema")]
        ECMHLiveObjectSetDigest,
    ),
}

impl SerializeAs<CheckpointCommitment> for CheckpointCommitmentSchema {
    fn serialize_as<S>(source: &CheckpointCommitment, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let iota_commitment = CheckpointCommitmentSchema::from(source.clone());
        iota_commitment.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, CheckpointCommitment> for CheckpointCommitmentSchema {
    fn deserialize_as<D>(deserializer: D) -> Result<CheckpointCommitment, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let iota_commitment = CheckpointCommitmentSchema::deserialize(deserializer)?;
        Ok(iota_commitment.into())
    }
}

impl From<CheckpointCommitmentSchema> for CheckpointCommitment {
    fn from(iota_commitment: CheckpointCommitmentSchema) -> Self {
        match iota_commitment {
            CheckpointCommitmentSchema::ECMHLiveObjectSetDigest(digest) => {
                CheckpointCommitment::ECMHLiveObjectSetDigest(digest)
            }
        }
    }
}

impl From<CheckpointCommitment> for CheckpointCommitmentSchema {
    fn from(commitment: CheckpointCommitment) -> Self {
        match commitment {
            CheckpointCommitment::ECMHLiveObjectSetDigest(digest) => {
                CheckpointCommitmentSchema::ECMHLiveObjectSetDigest(digest)
            }
        }
    }
}

/// The Sha256 digest of an EllipticCurveMultisetHash committing to the live
/// object set.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename = "ECMHLiveObjectSetDigest")]
pub struct ECMHLiveObjectSetDigestSchema {
    #[schemars(with = "[u8; 32]")]
    pub digest: Digest,
}

impl SerializeAs<ECMHLiveObjectSetDigest> for ECMHLiveObjectSetDigestSchema {
    fn serialize_as<S>(source: &ECMHLiveObjectSetDigest, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let iota_digest = ECMHLiveObjectSetDigestSchema::from(source.clone());
        iota_digest.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, ECMHLiveObjectSetDigest> for ECMHLiveObjectSetDigestSchema {
    fn deserialize_as<D>(deserializer: D) -> Result<ECMHLiveObjectSetDigest, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let iota_digest = ECMHLiveObjectSetDigestSchema::deserialize(deserializer)?;
        Ok(iota_digest.into())
    }
}

impl From<ECMHLiveObjectSetDigestSchema> for ECMHLiveObjectSetDigest {
    fn from(iota_digest: ECMHLiveObjectSetDigestSchema) -> Self {
        Self {
            digest: iota_digest.digest,
        }
    }
}

impl From<ECMHLiveObjectSetDigest> for ECMHLiveObjectSetDigestSchema {
    fn from(digest: ECMHLiveObjectSetDigest) -> Self {
        Self {
            digest: digest.digest,
        }
    }
}

#[serde_as]
#[derive(Clone, Copy, Debug, JsonSchema, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CheckpointId {
    SequenceNumber(
        #[schemars(with = "String")]
        #[serde_as(as = "DisplayFromStr")]
        CheckpointSequenceNumber,
    ),
    Digest(#[schemars(with = "Base58Schema")] CheckpointDigest),
}

impl From<CheckpointSequenceNumber> for CheckpointId {
    fn from(seq: CheckpointSequenceNumber) -> Self {
        Self::SequenceNumber(seq)
    }
}

impl From<CheckpointDigest> for CheckpointId {
    fn from(digest: CheckpointDigest) -> Self {
        Self::Digest(digest)
    }
}
