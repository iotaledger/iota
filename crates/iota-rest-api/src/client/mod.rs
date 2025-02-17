// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod sdk;

use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectID, SequenceNumber},
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
            .and_then(|checkpoint| checkpoint.try_into().map_err(Into::into))
    }

    pub async fn get_object(&self, object_id: ObjectID) -> Result<Object> {
        self.inner
            .get_object(object_id.into())
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
            .get_object_with_version(object_id.into(), version.into())
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

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct BalanceChange {
    /// Owner of the balance change
    pub address: IotaAddress,
    /// Type of the Coin
    pub coin_type: TypeTag,
    /// The amount indicate the balance value changes,
    /// negative amount means spending coin value and positive means receiving
    /// coin value.
    pub amount: i128,
}
