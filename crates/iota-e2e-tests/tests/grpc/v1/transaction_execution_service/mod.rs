// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
mod execute;
mod header;
mod simulate;

use iota_grpc_types::v1::{
    bcs::BcsData,
    signatures::{UserSignature, UserSignatures},
    transaction::Transaction as ProtoTransaction,
    transaction_execution_service::ExecuteTransactionItem,
};

/// Build an `ExecuteTransactionItem` from a signed transaction.
fn build_item(txn: &iota_types::transaction::Transaction) -> ExecuteTransactionItem {
    let transaction = ProtoTransaction::default()
        .with_bcs(BcsData::default().with_data(bcs::to_bytes(txn.transaction_data()).unwrap()));

    let signatures = UserSignatures::default().with_signatures(
        txn.tx_signatures()
            .iter()
            .map(|s| {
                UserSignature::default()
                    .with_bcs(BcsData::default().with_data(bcs::to_bytes(s).unwrap()))
            })
            .collect(),
    );

    ExecuteTransactionItem::default()
        .with_transaction(transaction)
        .with_signatures(signatures)
}
