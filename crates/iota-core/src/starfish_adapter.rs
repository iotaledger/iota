// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use arc_swap::{ArcSwapOption, Guard};
use iota_types::{
    error::{IotaError, IotaResult, UserInputError},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
};
use starfish_core::{ClientError, TransactionClient};
use tap::prelude::*;
use tokio::time::{Instant, sleep};
use tracing::{error, info, warn};

use crate::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    consensus_adapter,
    consensus_adapter::{BlockStatusReceiver, ConsensusClient},
    consensus_handler::SequencedConsensusTransactionKey,
};

/// Gets a client to submit transactions to Starfish, or waits for one to be
/// available. This hides the complexities of async consensus initialization and
/// submitting to different instances of consensus across epochs.
#[derive(Default, Clone)]
pub struct LazyStarfishClient {
    client: Arc<ArcSwapOption<TransactionClient>>,
}

impl LazyStarfishClient {
    pub fn new() -> Self {
        Self {
            client: Arc::new(ArcSwapOption::empty()),
        }
    }

    async fn get(&self) -> Guard<Option<Arc<TransactionClient>>> {
        let client = self.client.load();
        if client.is_some() {
            return client;
        }

        // Consensus client is initialized after validators or epoch starts, and cleared
        // after an epoch ends. But calls to get() can happen during validator
        // startup or epoch change, before consensus finished initializations.

        // TODO: https://github.com/iotaledger/iota/issues/8359
        // Consider listening to updates from consensus manager instead of polling.
        let mut count = 0;
        let start = Instant::now();
        const RETRY_INTERVAL: Duration = Duration::from_millis(100);
        loop {
            let client = self.client.load();
            if client.is_some() {
                return client;
            } else {
                sleep(RETRY_INTERVAL).await;
                count += 1;
                if count % 100 == 0 {
                    warn!(
                        "Waiting for consensus to initialize after {:?}",
                        Instant::now() - start
                    );
                }
            }
        }
    }

    pub fn set(&self, client: Arc<TransactionClient>) {
        self.client.store(Some(client));
    }

    pub fn clear(&self) {
        self.client.store(None);
    }
}

#[async_trait::async_trait]
impl ConsensusClient for LazyStarfishClient {
    async fn submit(
        &self,
        transactions: &[ConsensusTransaction],
        _epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<BlockStatusReceiver> {
        // TODO: https://github.com/iotaledger/iota/issues/8354
        // The retrieved TransactionClient can be from the past epoch. Submit would fail
        // after Starfish shuts down, so there should be no correctness issue.
        let client = self.get().await;
        let transactions_bytes = transactions
            .iter()
            .map(|t| bcs::to_bytes(t).expect("Serializing consensus transaction cannot fail"))
            .collect::<Vec<_>>();

        let (block_ref, status_waiter) = client
            .as_ref()
            .expect("Client should always be returned")
            .submit(transactions_bytes)
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

/// Returns the error [`LazyStarfishClient`] reports for a Starfish refusal:
/// [`UserInputError::SizeLimitExceeded`] for a submission larger than
/// consensus accepts, and [`IotaError::FailedToSubmitToConsensus`] when
/// consensus is shutting down.
fn to_iota_error(error: ClientError) -> IotaError {
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
