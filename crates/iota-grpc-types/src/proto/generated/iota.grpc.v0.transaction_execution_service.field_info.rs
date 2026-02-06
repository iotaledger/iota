// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v0::command::CommandResults;
    #[allow(unused_imports)]
    use crate::v0::command::CommandResultsFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v0::signatures::UserSignatures;
    #[allow(unused_imports)]
    use crate::v0::signatures::UserSignaturesFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v0::transaction::ExecutedTransaction;
    #[allow(unused_imports)]
    use crate::v0::transaction::ExecutedTransactionFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v0::transaction::Transaction;
    #[allow(unused_imports)]
    use crate::v0::transaction::TransactionFieldPathBuilder;
    impl ExecuteTransactionRequest {
        pub const TRANSACTION_FIELD: &'static MessageField = &MessageField {
            name: "transaction",
            json_name: "transaction",
            number: 1i32,
            is_optional: true,
            message_fields: Some(Transaction::FIELDS),
        };
        pub const SIGNATURES_FIELD: &'static MessageField = &MessageField {
            name: "signatures",
            json_name: "signatures",
            number: 2i32,
            is_optional: true,
            message_fields: Some(UserSignatures::FIELDS),
        };
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
            number: 3i32,
            is_optional: true,
            message_fields: None,
        };
    }
    impl MessageFields for ExecuteTransactionRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::TRANSACTION_FIELD,
            Self::SIGNATURES_FIELD,
            Self::READ_MASK_FIELD,
        ];
    }
    impl ExecuteTransactionRequest {
        pub fn path_builder() -> ExecuteTransactionRequestFieldPathBuilder {
            ExecuteTransactionRequestFieldPathBuilder::new()
        }
    }
    pub struct ExecuteTransactionRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ExecuteTransactionRequestFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn transaction(mut self) -> TransactionFieldPathBuilder {
            self.path.push(ExecuteTransactionRequest::TRANSACTION_FIELD.name);
            TransactionFieldPathBuilder::new_with_base(self.path)
        }
        pub fn signatures(mut self) -> UserSignaturesFieldPathBuilder {
            self.path.push(ExecuteTransactionRequest::SIGNATURES_FIELD.name);
            UserSignaturesFieldPathBuilder::new_with_base(self.path)
        }
        pub fn read_mask(mut self) -> String {
            self.path.push(ExecuteTransactionRequest::READ_MASK_FIELD.name);
            self.finish()
        }
    }
    impl ExecuteTransactionResponse {
        pub const TRANSACTION_FIELD: &'static MessageField = &MessageField {
            name: "transaction",
            json_name: "transaction",
            number: 1i32,
            is_optional: true,
            message_fields: Some(ExecutedTransaction::FIELDS),
        };
    }
    impl MessageFields for ExecuteTransactionResponse {
        const FIELDS: &'static [&'static MessageField] = &[Self::TRANSACTION_FIELD];
    }
    impl ExecuteTransactionResponse {
        pub fn path_builder() -> ExecuteTransactionResponseFieldPathBuilder {
            ExecuteTransactionResponseFieldPathBuilder::new()
        }
    }
    pub struct ExecuteTransactionResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ExecuteTransactionResponseFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn transaction(mut self) -> ExecutedTransactionFieldPathBuilder {
            self.path.push(ExecuteTransactionResponse::TRANSACTION_FIELD.name);
            ExecutedTransactionFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl SimulateTransactionRequest {
        pub const TRANSACTION_FIELD: &'static MessageField = &MessageField {
            name: "transaction",
            json_name: "transaction",
            number: 1i32,
            is_optional: true,
            message_fields: Some(Transaction::FIELDS),
        };
        pub const TX_CHECKS_FIELD: &'static MessageField = &MessageField {
            name: "tx_checks",
            json_name: "txChecks",
            number: 2i32,
            is_optional: false,
            message_fields: None,
        };
        pub const ESTIMATE_GAS_BUDGET_FIELD: &'static MessageField = &MessageField {
            name: "estimate_gas_budget",
            json_name: "estimateGasBudget",
            number: 3i32,
            is_optional: true,
            message_fields: None,
        };
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
            number: 4i32,
            is_optional: true,
            message_fields: None,
        };
    }
    impl MessageFields for SimulateTransactionRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::TRANSACTION_FIELD,
            Self::TX_CHECKS_FIELD,
            Self::ESTIMATE_GAS_BUDGET_FIELD,
            Self::READ_MASK_FIELD,
        ];
    }
    impl SimulateTransactionRequest {
        pub fn path_builder() -> SimulateTransactionRequestFieldPathBuilder {
            SimulateTransactionRequestFieldPathBuilder::new()
        }
    }
    pub struct SimulateTransactionRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl SimulateTransactionRequestFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn transaction(mut self) -> TransactionFieldPathBuilder {
            self.path.push(SimulateTransactionRequest::TRANSACTION_FIELD.name);
            TransactionFieldPathBuilder::new_with_base(self.path)
        }
        pub fn tx_checks(mut self) -> String {
            self.path.push(SimulateTransactionRequest::TX_CHECKS_FIELD.name);
            self.finish()
        }
        pub fn estimate_gas_budget(mut self) -> String {
            self.path.push(SimulateTransactionRequest::ESTIMATE_GAS_BUDGET_FIELD.name);
            self.finish()
        }
        pub fn read_mask(mut self) -> String {
            self.path.push(SimulateTransactionRequest::READ_MASK_FIELD.name);
            self.finish()
        }
    }
    impl SimulateTransactionResponse {
        pub const TRANSACTION_FIELD: &'static MessageField = &MessageField {
            name: "transaction",
            json_name: "transaction",
            number: 1i32,
            is_optional: true,
            message_fields: Some(ExecutedTransaction::FIELDS),
        };
        pub const COMMAND_RESULTS_FIELD: &'static MessageField = &MessageField {
            name: "command_results",
            json_name: "commandResults",
            number: 2i32,
            is_optional: true,
            message_fields: Some(CommandResults::FIELDS),
        };
    }
    impl MessageFields for SimulateTransactionResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::TRANSACTION_FIELD,
            Self::COMMAND_RESULTS_FIELD,
        ];
    }
    impl SimulateTransactionResponse {
        pub fn path_builder() -> SimulateTransactionResponseFieldPathBuilder {
            SimulateTransactionResponseFieldPathBuilder::new()
        }
    }
    pub struct SimulateTransactionResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl SimulateTransactionResponseFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn transaction(mut self) -> ExecutedTransactionFieldPathBuilder {
            self.path.push(SimulateTransactionResponse::TRANSACTION_FIELD.name);
            ExecutedTransactionFieldPathBuilder::new_with_base(self.path)
        }
        pub fn command_results(mut self) -> CommandResultsFieldPathBuilder {
            self.path.push(SimulateTransactionResponse::COMMAND_RESULTS_FIELD.name);
            CommandResultsFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
