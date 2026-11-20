// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use iota_types::{
    committee::EpochId,
    error::{IotaError, IotaResult, UserInputError},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
};
use starfish_core::{ClientError, Priority, TransactionClient};
use tap::prelude::*;
use tokio::{
    sync::watch,
    time::{Instant, timeout},
};
use tracing::{error, info, warn};

use crate::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    consensus_adapter,
    consensus_adapter::{BlockStatusReceiver, ConsensusClient, is_high_priority},
    consensus_handler::SequencedConsensusTransactionKey,
};

/// Gets a client to submit transactions to Starfish, or waits for one to be
/// available. This hides the complexities of async consensus initialization and
/// submitting to different instances of consensus across epochs.
#[derive(Default, Clone)]
pub struct LazyStarfishClient {
    client: watch::Sender<Option<EpochClient>>,
}

/// The client of one Starfish instance and the epoch that instance runs.
struct EpochClient {
    epoch: EpochId,
    client: Arc<TransactionClient>,
}

impl LazyStarfishClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the client of the Starfish instance that runs `epoch`, waiting
    /// until that instance is set. A caller of an ended epoch never gets a
    /// client, so it must be cancelled when its epoch ends.
    async fn get(&self, epoch: EpochId) -> Arc<TransactionClient> {
        const WARN_INTERVAL: Duration = Duration::from_secs(10);
        let mut updates = self.client.subscribe();
        let start = Instant::now();
        loop {
            let wait = updates.wait_for(|current| {
                current
                    .as_ref()
                    .is_some_and(|current| current.epoch == epoch)
            });
            match timeout(WARN_INTERVAL, wait).await {
                Ok(current) => {
                    return current
                        .expect("the sender lives as long as self")
                        .as_ref()
                        .expect("the wait ends only on a set client")
                        .client
                        .clone();
                }
                Err(_) => warn!(
                    "Waiting for the consensus client of epoch {epoch} after {:?}",
                    start.elapsed()
                ),
            }
        }
    }

    #[cfg(test)]
    pub(crate) async fn get_for_testing(&self, epoch: EpochId) -> Arc<TransactionClient> {
        self.get(epoch).await
    }

    /// Sets the client of the Starfish instance that runs `epoch`.
    pub fn set(&self, epoch: EpochId, client: Arc<TransactionClient>) {
        self.client
            .send_replace(Some(EpochClient { epoch, client }));
    }

    pub fn clear(&self) {
        self.client.send_replace(None);
    }
}

#[async_trait::async_trait]
impl ConsensusClient for LazyStarfishClient {
    async fn submit(
        &self,
        transactions: &[ConsensusTransaction],
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<BlockStatusReceiver> {
        // The epoch store of the next epoch goes live before the consensus of the
        // ending epoch stops, and that consensus drops what the next epoch submits.
        let client = self.get(epoch_store.epoch()).await;
        let transactions_bytes = transactions
            .iter()
            .map(|t| bcs::to_bytes(t).expect("Serializing consensus transaction cannot fail"))
            .collect::<Vec<_>>();

        let priority = match transactions {
            [transaction] if is_high_priority(transaction) => Priority::High,
            _ => Priority::Normal,
        };
        let (block_ref, status_waiter) = client
            .submit(transactions_bytes, priority)
            .await
            .tap_err(|err| {
                // Will be logged by caller as well.
                let msg = format!("Transaction submission failed with: {err:?}");
                match err {
                    ClientError::ConsensusShuttingDown(_) => {
                        info!("{}", msg);
                    }
                    ClientError::OversizedTransaction(_, _)
                    | ClientError::OversizedTransactionBundleBytes(_, _)
                    | ClientError::OversizedTransactionBundleCount(_, _) => {
                        if cfg!(debug_assertions) {
                            panic!("{}", msg);
                        } else {
                            error!("{}", msg);
                        }
                    }
                };
            })
            .map_err(to_iota_error)?;

        let is_soft_bundle = transactions.len() > 1;

        if !is_soft_bundle
            && matches!(
                transactions[0].kind,
                ConsensusTransactionKind::EndOfPublish(_)
                    | ConsensusTransactionKind::CapabilityNotificationV1(_)
                    | ConsensusTransactionKind::RandomnessDkgMessage(_, _)
                    | ConsensusTransactionKind::RandomnessDkgConfirmation(_, _)
                    | ConsensusTransactionKind::OverloadNotificationV1(_, _, _)
                    | ConsensusTransactionKind::TransactionDenyRuleProposal(_)
            )
        {
            let transaction_key = SequencedConsensusTransactionKey::External(transactions[0].key());
            tracing::info!("Transaction {transaction_key:?} was included in {block_ref}",)
        };

        // Transform the status waiter into a receiver that can be used by the consensus
        // adapter.

        let (tx_internal, rx_internal) =
            tokio::sync::oneshot::channel::<consensus_adapter::BlockStatusInternal>();

        tokio::spawn(async move {
            if let Ok(status) = status_waiter.await {
                let _ = tx_internal.send(status.into());
            }
        });

        Ok(rx_internal)
    }
}

/// Returns the [`IotaError`] for a Starfish refusal:
/// [`UserInputError::SizeLimitExceeded`] for a submission larger than
/// consensus accepts, and [`IotaError::FailedToSubmitToConsensus`] when
/// consensus is shutting down.
pub(crate) fn to_iota_error(error: ClientError) -> IotaError {
    let size_limit_exceeded = |limit: String, value: u64| IotaError::UserInput {
        error: UserInputError::SizeLimitExceeded {
            limit,
            value: value.to_string(),
        },
    };

    match error {
        ClientError::OversizedTransaction(size, max_size) => size_limit_exceeded(
            format!("serialized consensus transaction size exceeded maximum of {max_size}"),
            size,
        ),
        ClientError::OversizedTransactionBundleBytes(size, max_size) => size_limit_exceeded(
            format!(
                "total serialized size of consensus transactions in one submission exceeded \
                    maximum of {max_size}"
            ),
            size,
        ),
        ClientError::OversizedTransactionBundleCount(count, max_count) => size_limit_exceeded(
            format!(
                "number of consensus transactions in one submission exceeded maximum of \
                    {max_count}"
            ),
            count,
        ),
        ClientError::ConsensusShuttingDown(_) => {
            IotaError::FailedToSubmitToConsensus(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_types::error::{IotaError, UserInputError};
    use starfish_core::ClientError;

    use super::to_iota_error;

    /// Starfish's refusals of an oversized submission become the size error
    /// the consensus adapter does not retry, and a shutdown stays an error
    /// the adapter retries.
    #[test]
    fn oversized_refusals_become_size_errors() {
        for refusal in [
            ClientError::OversizedTransaction(300, 200),
            ClientError::OversizedTransactionBundleBytes(600, 500),
            ClientError::OversizedTransactionBundleCount(9, 8),
        ] {
            assert!(
                matches!(
                    to_iota_error(refusal),
                    IotaError::UserInput {
                        error: UserInputError::SizeLimitExceeded { .. }
                    }
                ),
                "an oversized submission should be refused as too large",
            );
        }
        assert!(matches!(
            to_iota_error(ClientError::ConsensusShuttingDown(
                "shutting down".to_string()
            )),
            IotaError::FailedToSubmitToConsensus(_)
        ));
    }
}
