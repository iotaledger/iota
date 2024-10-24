// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod narwhal {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Transaction {
        #[prost(bytes = "bytes", repeated, tag = "1")]
        pub transactions: ::prost::alloc::vec::Vec<::prost::bytes::Bytes>,
    }
    /// Empty message for when we don't have anything to return
    #[derive(Clone, Copy, PartialEq, ::prost::Message)]
    pub struct Empty {}

    include!(concat!(env!("OUT_DIR"), "/narwhal.Transactions.rs"));
}

use bytes::Bytes;
pub use narwhal::{
    Empty, Transaction as TransactionProto,
    transactions_client::TransactionsClient,
    transactions_server::{Transactions, TransactionsServer},
};

use crate::Transaction;

impl From<Transaction> for TransactionProto {
    fn from(transaction: Transaction) -> Self {
        TransactionProto {
            transactions: vec![Bytes::from(transaction)],
        }
    }
}

impl From<Vec<Transaction>> for TransactionProto {
    fn from(transactions: Vec<Transaction>) -> Self {
        TransactionProto {
            transactions: transactions.into_iter().map(Bytes::from).collect(),
        }
    }
}
