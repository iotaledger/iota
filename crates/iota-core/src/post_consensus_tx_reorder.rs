// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_metrics::monitored_scope;
use iota_protocol_config::ConsensusTransactionOrdering;
use iota_types::messages_consensus::ConsensusTransactionKind;

use crate::consensus_handler::{
    SequencedConsensusTransactionKind, VerifiedSequencedConsensusTransaction,
};

pub struct PostConsensusTxReorder {}

impl PostConsensusTxReorder {
    pub fn reorder(
        transactions: &mut [VerifiedSequencedConsensusTransaction],
        kind: ConsensusTransactionOrdering,
    ) {
        // TODO: make the reordering algorithm richer and depend on object hotness as
        // well. Order transactions based on their gas prices. System
        // transactions without gas price are put to the beginning of the
        // sequenced_transactions vector.
        match kind {
            ConsensusTransactionOrdering::ByGasPrice => Self::order_by_gas_price(transactions),
            ConsensusTransactionOrdering::None => (),
        }
    }

    fn order_by_gas_price(transactions: &mut [VerifiedSequencedConsensusTransaction]) {
        let _scope = monitored_scope("HandleConsensusOutput::order_by_gas_price");
        transactions.sort_by_key(|txn| {
            // Reverse order, so that transactions with higher gas price are put to the
            // beginning.
            std::cmp::Reverse({
                match &txn.0.transaction {
                    // Listed exhaustively (no `_` arm) so a new user-transaction kind must
                    // be classified here rather than silently sorting to the front.
                    SequencedConsensusTransactionKind::External(ext) => match &ext.kind {
                        ConsensusTransactionKind::CertifiedTransaction(cert) => cert.gas_price(),
                        ConsensusTransactionKind::UserTransactionV1(tx) => tx.gas_price(),
                        // Non-user messages carry no gas price and sort to the front.
                        ConsensusTransactionKind::CheckpointSignature(_)
                        | ConsensusTransactionKind::EndOfPublish(_)
                        | ConsensusTransactionKind::CapabilityNotificationV1(_)
                        | ConsensusTransactionKind::SignedCapabilityNotificationV1(_)
                        | ConsensusTransactionKind::RandomnessDkgMessage(..)
                        | ConsensusTransactionKind::RandomnessDkgConfirmation(..)
                        | ConsensusTransactionKind::MisbehaviorReport(_)
                        | ConsensusTransactionKind::OverloadNotificationV1(..) => u64::MAX,
                        #[allow(deprecated)]
                        ConsensusTransactionKind::NewJWKFetchedDeprecated => u64::MAX,
                    },
                    // System transactions carry no gas price and sort to the front.
                    SequencedConsensusTransactionKind::System(_) => u64::MAX,
                }
            })
        })
    }
}
