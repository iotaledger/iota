// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_types::{
    base_types::IotaAddress, effects::TransactionEffectsExt,
    full_checkpoint_content::CheckpointData, object::Owner, transaction::TransactionDataAPI,
};

use crate::{BigTableClient, KeyValueStoreWriter, TransactionData};

/// This worker implementation is responsible for processing checkpoints by
/// storing its data as Key-Value pairs. The Key-Value pairs are stored in a
/// BigTableDB.
pub struct KvWorker {
    pub client: BigTableClient,
}

#[async_trait]
impl Worker for KvWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(&self, checkpoint: Arc<CheckpointData>) -> anyhow::Result<()> {
        let mut client = self.client.clone();
        let mut objects = vec![];
        let mut transactions = Vec::with_capacity(checkpoint.transactions.len());

        for transaction in &checkpoint.transactions {
            for object in &transaction.output_objects {
                objects.push(object);
            }
            transactions.push(TransactionData::new(
                transaction,
                checkpoint.checkpoint_summary.sequence_number,
            ));
        }

        client.save_objects(&objects).await?;
        client.save_transactions(&transactions).await?;

        let entries_by_address = checkpoint
            .checkpoint_contents
            .enumerate_transactions(&checkpoint.checkpoint_summary)
            .zip(&checkpoint.transactions)
            .flat_map(|((seq, exec_digest), tx)| {
                let digest = exec_digest.transaction;
                let tx_data = tx.transaction.transaction_data();

                let affected = std::iter::once(tx_data.sender())
                    .chain(std::iter::once(tx_data.gas_owner()))
                    .chain(tx.effects.all_changed_objects().into_iter().filter_map(
                        |(_object_ref, owner, _write_kind)| match owner {
                            Owner::Address(a) => Some(a),
                            _ => None,
                        },
                    ))
                    .collect::<HashSet<IotaAddress>>();

                affected
                    .into_iter()
                    .map(move |address| (address, seq, digest))
            });

        client
            .save_transactions_by_address(entries_by_address)
            .await?;
        client.save_checkpoint(&checkpoint).await?;

        Ok(())
    }
}
