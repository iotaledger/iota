// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::ExecuteTransactionRequest {
        /// Sets `transaction` with the provided value.
        pub fn with_transaction<T: Into<super::super::transaction::Transaction>>(
            mut self,
            field: T,
        ) -> Self {
            self.transaction = Some(field.into());
            self
        }
        /// Sets `signatures` with the provided value.
        pub fn with_signatures<T: Into<super::super::signatures::UserSignatures>>(
            mut self,
            field: T,
        ) -> Self {
            self.signatures = Some(field.into());
            self
        }
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
    }
    impl super::ExecuteTransactionResponse {
        /// Sets `executed_transaction` with the provided value.
        pub fn with_executed_transaction<
            T: Into<super::super::transaction::ExecutedTransaction>,
        >(mut self, field: T) -> Self {
            self.executed_transaction = Some(field.into());
            self
        }
    }
    impl super::ExecutionError {
        /// Sets `bcs_kind` with the provided value.
        pub fn with_bcs_kind<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.bcs_kind = Some(field.into());
            self
        }
        /// Sets `source` with the provided value.
        pub fn with_source<T: Into<String>>(mut self, field: T) -> Self {
            self.source = Some(field.into());
            self
        }
        /// Sets `command_index` with the provided value.
        pub fn with_command_index(mut self, field: u64) -> Self {
            self.command_index = Some(field);
            self
        }
    }
    impl super::SimulateTransactionRequest {
        /// Sets `transaction` with the provided value.
        pub fn with_transaction<T: Into<super::super::transaction::Transaction>>(
            mut self,
            field: T,
        ) -> Self {
            self.transaction = Some(field.into());
            self
        }
        /// Sets `tx_checks` with the provided value.
        pub fn with_tx_checks(mut self, field: Vec<i32>) -> Self {
            self.tx_checks = field;
            self
        }
        /// Sets `estimate_gas_budget` with the provided value.
        pub fn with_estimate_gas_budget(mut self, field: bool) -> Self {
            self.estimate_gas_budget = Some(field);
            self
        }
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
    }
}
