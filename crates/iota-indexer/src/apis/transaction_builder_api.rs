// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub(crate) use iota_json_rpc::transaction_builder_api::TransactionBuilderApi;

use crate::read::IndexerReader;

impl From<IndexerReader> for TransactionBuilderApi {
    fn from(inner: IndexerReader) -> Self {
        Self::new_with_data_reader(std::sync::Arc::new(inner))
    }
}
