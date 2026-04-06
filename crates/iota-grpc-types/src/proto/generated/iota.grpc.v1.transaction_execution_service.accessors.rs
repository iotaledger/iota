// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::ExecuteTransactionItem {
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
    }
    impl super::ExecuteTransactionResult {
        /// Sets `executed_transaction` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_executed_transaction<
            T: Into<super::super::transaction::ExecutedTransaction>,
        >(mut self, field: T) -> Self {
            self.result = Some(
                super::execute_transaction_result::Result::ExecutedTransaction(
                    field.into(),
                ),
            );
            self
        }
        /// Sets `error` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_error<
            T: Into<super::super::super::super::super::google::rpc::Status>,
        >(mut self, field: T) -> Self {
            self.result = Some(
                super::execute_transaction_result::Result::Error(field.into()),
            );
            self
        }
    }
    impl super::ExecuteTransactionsRequest {
        /// Sets `transactions` with the provided value.
        pub fn with_transactions(
            mut self,
            field: Vec<super::ExecuteTransactionItem>,
        ) -> Self {
            self.transactions = field;
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
        /// Sets `checkpoint_inclusion_timeout_ms` with the provided value.
        pub fn with_checkpoint_inclusion_timeout_ms(mut self, field: u64) -> Self {
            self.checkpoint_inclusion_timeout_ms = Some(field);
            self
        }
    }
    impl super::ExecuteTransactionsResponse {
        /// Sets `transaction_results` with the provided value.
        pub fn with_transaction_results(
            mut self,
            field: Vec<super::ExecuteTransactionResult>,
        ) -> Self {
            self.transaction_results = field;
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
    impl super::SimulateTransactionItem {
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
    }
    impl super::SimulateTransactionResult {
        /// Sets `simulated_transaction` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_simulated_transaction<T: Into<super::SimulatedTransaction>>(
            mut self,
            field: T,
        ) -> Self {
            self.result = Some(
                super::simulate_transaction_result::Result::SimulatedTransaction(
                    field.into(),
                ),
            );
            self
        }
        /// Sets `error` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_error<
            T: Into<super::super::super::super::super::google::rpc::Status>,
        >(mut self, field: T) -> Self {
            self.result = Some(
                super::simulate_transaction_result::Result::Error(field.into()),
            );
            self
        }
    }
    impl super::SimulateTransactionsRequest {
        /// Sets `transactions` with the provided value.
        pub fn with_transactions(
            mut self,
            field: Vec<super::SimulateTransactionItem>,
        ) -> Self {
            self.transactions = field;
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
    impl super::SimulateTransactionsResponse {
        /// Sets `transaction_results` with the provided value.
        pub fn with_transaction_results(
            mut self,
            field: Vec<super::SimulateTransactionResult>,
        ) -> Self {
            self.transaction_results = field;
            self
        }
    }
    impl super::SimulatedTransaction {
        /// Sets `executed_transaction` with the provided value.
        pub fn with_executed_transaction<
            T: Into<super::super::transaction::ExecutedTransaction>,
        >(mut self, field: T) -> Self {
            self.executed_transaction = Some(field.into());
            self
        }
        /// Sets `suggested_gas_price` with the provided value.
        pub fn with_suggested_gas_price(mut self, field: u64) -> Self {
            self.suggested_gas_price = Some(field);
            self
        }
        /// Sets `command_results` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_command_results<T: Into<super::super::command::CommandResults>>(
            mut self,
            field: T,
        ) -> Self {
            self.execution_result = Some(
                super::simulated_transaction::ExecutionResult::CommandResults(
                    field.into(),
                ),
            );
            self
        }
        /// Sets `execution_error` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_execution_error<T: Into<super::ExecutionError>>(
            mut self,
            field: T,
        ) -> Self {
            self.execution_result = Some(
                super::simulated_transaction::ExecutionResult::ExecutionError(
                    field.into(),
                ),
            );
            self
        }
    }
}
