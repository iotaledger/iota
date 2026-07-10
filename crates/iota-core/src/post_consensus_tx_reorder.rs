// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_metrics::monitored_scope;
use iota_protocol_config::ConsensusTransactionOrdering;
use iota_types::transaction::TransactionDataAPI;

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
            // Reverse order, so transactions with higher gas price sort to the front.
            // Internal messages and system transactions carry no gas price and sort
            // to the front via `u64::MAX`.
            let gas_price = match &txn.0.transaction {
                SequencedConsensusTransactionKind::External(ext) => {
                    ext.kind.as_sender_signed_data()
                }
                SequencedConsensusTransactionKind::System(_) => None,
            }
            .map(|data| data.transaction_data().gas_price())
            .unwrap_or(u64::MAX);

            std::cmp::Reverse(gas_price)
        })
    }
}
