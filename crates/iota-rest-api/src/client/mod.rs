// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod sdk;

use iota_sdk_types::EpochId;
use iota_types::{
    base_types::{IotaAddress, ObjectID, SequenceNumber, TypeTag},
    crypto::AuthorityStrongQuorumSignInfo,
    effects::{TransactionEffects, TransactionEvents},
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointSequenceNumber},
    object::Object,
    transaction::Transaction,
};
pub use reqwest;
use sdk::Result;

use self::sdk::Response;
use crate::transactions::ExecuteTransactionQueryParameters;

#[derive(Clone)]
pub struct Client {
    inner: sdk::Client,
}

impl Client {
    pub fn new<S: AsRef<str>>(base_url: S) -> Self {
        Self {
            inner: sdk::Client::new(base_url.as_ref()).unwrap(),
        }
    }

    pub fn inner(&self) -> &sdk::Client {
        &self.inner
    }

    pub async fn get_latest_checkpoint(&self) -> Result<CertifiedCheckpointSummary> {
        self.inner
            .get_latest_checkpoint()
            .await
            .map(Response::into_inner)
            .and_then(|checkpoint| checkpoint.try_into().map_err(Into::into))
    }

    pub async fn get_full_checkpoint(
        &self,
        checkpoint_sequence_number: CheckpointSequenceNumber,
    ) -> Result<CheckpointData> {
        let url = self
            .inner
            .url()
            .join(&format!("checkpoints/{checkpoint_sequence_number}/full"))?;

        let response = self
            .inner
            .client()
            .get(url)
            .header(reqwest::header::ACCEPT, crate::APPLICATION_BCS)
            .send()
            .await?;

        self.inner.bcs(response).await.map(Response::into_inner)
    }

    pub async fn get_checkpoint_summary(
        &self,
        checkpoint_sequence_number: CheckpointSequenceNumber,
    ) -> Result<CertifiedCheckpointSummary> {
        self.inner
            .get_checkpoint(checkpoint_sequence_number)
            .await
            .map(Response::into_inner)
            .and_then(|checkpoint| {
                iota_sdk_types::SignedCheckpointSummary {
                    checkpoint: checkpoint.checkpoint,
                    signature: checkpoint.signature,
                }
                .try_into()
                .map_err(Into::into)
            })
    }

    pub async fn get_object(&self, object_id: ObjectID) -> Result<Object> {
        self.inner
            .get_object(object_id)
            .await
            .map(Response::into_inner)
            .and_then(|object| object.try_into().map_err(Into::into))
    }

    pub async fn get_object_with_version(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
    ) -> Result<Object> {
        self.inner
            .get_object_with_version(object_id, version)
            .await
            .map(Response::into_inner)
            .and_then(|object| object.try_into().map_err(Into::into))
    }

    pub async fn execute_transaction(
        &self,
        parameters: &ExecuteTransactionQueryParameters,
        transaction: &Transaction,
    ) -> Result<TransactionExecutionResponse> {
        #[derive(serde::Serialize)]
        struct SignedTransaction<'a> {
            transaction: &'a iota_types::transaction::TransactionData,
            signatures: &'a [iota_types::signature::GenericSignature],
        }

        let url = self.inner.url().join("transactions")?;
        let body = bcs::to_bytes(&SignedTransaction {
            transaction: &transaction.inner().intent_message.value,
            signatures: &transaction.inner().tx_signatures,
        })?;

        let response = self
            .inner
            .client()
            .post(url)
            .query(parameters)
            .header(reqwest::header::ACCEPT, crate::APPLICATION_BCS)
            .header(reqwest::header::CONTENT_TYPE, crate::APPLICATION_BCS)
            .body(body)
            .send()
            .await?;

        self.inner.bcs(response).await.map(Response::into_inner)
    }

    pub async fn get_epoch_last_checkpoint(
        &self,
        epoch: EpochId,
    ) -> Result<CertifiedCheckpointSummary> {
        self.inner
            .get_epoch_last_checkpoint(epoch)
            .await
            .map(Response::into_inner)
            .and_then(|checkpoint| checkpoint.try_into().map_err(Into::into))
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TransactionExecutionResponse {
    pub effects: TransactionEffects,

    pub finality: EffectsFinality,
    pub events: Option<TransactionEvents>,
    pub balance_changes: Option<Vec<BalanceChange>>,
    pub input_objects: Option<Vec<Object>>,
    pub output_objects: Option<Vec<Object>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum EffectsFinality {
    Certified {
        signature: AuthorityStrongQuorumSignInfo,
    },
    Checkpointed {
        checkpoint: CheckpointSequenceNumber,
    },
}

type ReadableDisplay =
    ::serde_with::As<::serde_with::IfIsHumanReadable<::serde_with::DisplayFromStr>>;

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BalanceChange {
    /// Owner of the balance change
    pub address: IotaAddress,
    /// Type of the Coin
    pub coin_type: TypeTag,
    /// The amount indicate the balance value changes,
    /// negative amount means spending coin value and positive means receiving
    /// coin value.
    #[serde(with = "ReadableDisplay")]
    #[schemars(with = "I128")]
    pub amount: i128,
}

use schemars::{
    JsonSchema,
    schema::{InstanceType, Metadata, SchemaObject},
};

pub(crate) struct I128;

impl JsonSchema for I128 {
    fn schema_name() -> String {
        "i128".to_owned()
    }

    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("Radix-10 encoded 128-bit signed integer".to_owned()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            format: Some("i128".to_owned()),
            ..Default::default()
        }
        .into()
    }

    fn is_referenceable() -> bool {
        false
    }
}
