// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::ExecuteTransactionRequest {
        pub const fn const_default() -> Self {
            Self {
                transaction: None,
                signatures: None,
                read_mask: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ExecuteTransactionRequest = super::ExecuteTransactionRequest::const_default();
            &DEFAULT
        }
        ///Returns the value of `transaction`, or the default value if `transaction` is unset.
        pub fn transaction(&self) -> &super::super::transaction::Transaction {
            self.transaction
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::transaction::Transaction::default_instance() as _
                })
        }
        ///If `transaction` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transaction_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::transaction::Transaction> {
            self.transaction.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `transaction`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transaction_mut(
            &mut self,
        ) -> &mut super::super::transaction::Transaction {
            self.transaction.get_or_insert_default()
        }
        ///If `transaction` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transaction_opt(
            &self,
        ) -> Option<&super::super::transaction::Transaction> {
            self.transaction.as_ref().map(|field| field as _)
        }
        ///Sets `transaction` with the provided value.
        pub fn set_transaction<T: Into<super::super::transaction::Transaction>>(
            &mut self,
            field: T,
        ) {
            self.transaction = Some(field.into().into());
        }
        ///Sets `transaction` with the provided value.
        pub fn with_transaction<T: Into<super::super::transaction::Transaction>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_transaction(field.into());
            self
        }
        ///Returns the value of `signatures`, or the default value if `signatures` is unset.
        pub fn signatures(&self) -> &super::super::signatures::UserSignatures {
            self.signatures
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::signatures::UserSignatures::default_instance() as _
                })
        }
        ///If `signatures` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn signatures_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::signatures::UserSignatures> {
            self.signatures.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `signatures`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn signatures_mut(
            &mut self,
        ) -> &mut super::super::signatures::UserSignatures {
            self.signatures.get_or_insert_default()
        }
        ///If `signatures` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn signatures_opt(
            &self,
        ) -> Option<&super::super::signatures::UserSignatures> {
            self.signatures.as_ref().map(|field| field as _)
        }
        ///Sets `signatures` with the provided value.
        pub fn set_signatures<T: Into<super::super::signatures::UserSignatures>>(
            &mut self,
            field: T,
        ) {
            self.signatures = Some(field.into().into());
        }
        ///Sets `signatures` with the provided value.
        pub fn with_signatures<T: Into<super::super::signatures::UserSignatures>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_signatures(field.into());
            self
        }
        ///If `read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn read_mask_opt_mut(&mut self) -> Option<&mut ::prost_types::FieldMask> {
            self.read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.read_mask.get_or_insert_default()
        }
        ///If `read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `read_mask` with the provided value.
        pub fn set_read_mask<T: Into<::prost_types::FieldMask>>(&mut self, field: T) {
            self.read_mask = Some(field.into().into());
        }
        ///Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_read_mask(field.into());
            self
        }
    }
    impl super::ExecuteTransactionResponse {
        pub const fn const_default() -> Self {
            Self { transaction: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ExecuteTransactionResponse = super::ExecuteTransactionResponse::const_default();
            &DEFAULT
        }
        ///Returns the value of `transaction`, or the default value if `transaction` is unset.
        pub fn transaction(&self) -> &super::super::transaction::ExecutedTransaction {
            self.transaction
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::transaction::ExecutedTransaction::default_instance()
                        as _
                })
        }
        ///If `transaction` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transaction_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::transaction::ExecutedTransaction> {
            self.transaction.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `transaction`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transaction_mut(
            &mut self,
        ) -> &mut super::super::transaction::ExecutedTransaction {
            self.transaction.get_or_insert_default()
        }
        ///If `transaction` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transaction_opt(
            &self,
        ) -> Option<&super::super::transaction::ExecutedTransaction> {
            self.transaction.as_ref().map(|field| field as _)
        }
        ///Sets `transaction` with the provided value.
        pub fn set_transaction<T: Into<super::super::transaction::ExecutedTransaction>>(
            &mut self,
            field: T,
        ) {
            self.transaction = Some(field.into().into());
        }
        ///Sets `transaction` with the provided value.
        pub fn with_transaction<T: Into<super::super::transaction::ExecutedTransaction>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_transaction(field.into());
            self
        }
    }
    impl super::SimulateTransactionRequest {
        pub const fn const_default() -> Self {
            Self {
                transaction: None,
                tx_checks: Vec::new(),
                estimate_gas_budget: None,
                read_mask: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::SimulateTransactionRequest = super::SimulateTransactionRequest::const_default();
            &DEFAULT
        }
        ///Returns the value of `transaction`, or the default value if `transaction` is unset.
        pub fn transaction(&self) -> &super::super::transaction::Transaction {
            self.transaction
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::transaction::Transaction::default_instance() as _
                })
        }
        ///If `transaction` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transaction_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::transaction::Transaction> {
            self.transaction.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `transaction`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transaction_mut(
            &mut self,
        ) -> &mut super::super::transaction::Transaction {
            self.transaction.get_or_insert_default()
        }
        ///If `transaction` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transaction_opt(
            &self,
        ) -> Option<&super::super::transaction::Transaction> {
            self.transaction.as_ref().map(|field| field as _)
        }
        ///Sets `transaction` with the provided value.
        pub fn set_transaction<T: Into<super::super::transaction::Transaction>>(
            &mut self,
            field: T,
        ) {
            self.transaction = Some(field.into().into());
        }
        ///Sets `transaction` with the provided value.
        pub fn with_transaction<T: Into<super::super::transaction::Transaction>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_transaction(field.into());
            self
        }
        ///If `estimate_gas_budget` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn estimate_gas_budget_opt_mut(&mut self) -> Option<&mut bool> {
            self.estimate_gas_budget.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `estimate_gas_budget`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn estimate_gas_budget_mut(&mut self) -> &mut bool {
            self.estimate_gas_budget.get_or_insert_default()
        }
        ///If `estimate_gas_budget` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn estimate_gas_budget_opt(&self) -> Option<bool> {
            self.estimate_gas_budget.as_ref().map(|field| *field)
        }
        ///Sets `estimate_gas_budget` with the provided value.
        pub fn set_estimate_gas_budget(&mut self, field: bool) {
            self.estimate_gas_budget = Some(field);
        }
        ///Sets `estimate_gas_budget` with the provided value.
        pub fn with_estimate_gas_budget(mut self, field: bool) -> Self {
            self.set_estimate_gas_budget(field);
            self
        }
        ///If `read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn read_mask_opt_mut(&mut self) -> Option<&mut ::prost_types::FieldMask> {
            self.read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.read_mask.get_or_insert_default()
        }
        ///If `read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `read_mask` with the provided value.
        pub fn set_read_mask<T: Into<::prost_types::FieldMask>>(&mut self, field: T) {
            self.read_mask = Some(field.into().into());
        }
        ///Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_read_mask(field.into());
            self
        }
    }
    impl super::SimulateTransactionResponse {
        pub const fn const_default() -> Self {
            Self {
                transaction: None,
                command_results: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::SimulateTransactionResponse = super::SimulateTransactionResponse::const_default();
            &DEFAULT
        }
        ///Returns the value of `transaction`, or the default value if `transaction` is unset.
        pub fn transaction(&self) -> &super::super::transaction::ExecutedTransaction {
            self.transaction
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::transaction::ExecutedTransaction::default_instance()
                        as _
                })
        }
        ///If `transaction` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transaction_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::transaction::ExecutedTransaction> {
            self.transaction.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `transaction`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transaction_mut(
            &mut self,
        ) -> &mut super::super::transaction::ExecutedTransaction {
            self.transaction.get_or_insert_default()
        }
        ///If `transaction` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transaction_opt(
            &self,
        ) -> Option<&super::super::transaction::ExecutedTransaction> {
            self.transaction.as_ref().map(|field| field as _)
        }
        ///Sets `transaction` with the provided value.
        pub fn set_transaction<T: Into<super::super::transaction::ExecutedTransaction>>(
            &mut self,
            field: T,
        ) {
            self.transaction = Some(field.into().into());
        }
        ///Sets `transaction` with the provided value.
        pub fn with_transaction<T: Into<super::super::transaction::ExecutedTransaction>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_transaction(field.into());
            self
        }
        ///Returns the value of `command_results`, or the default value if `command_results` is unset.
        pub fn command_results(&self) -> &super::super::command::CommandResults {
            self.command_results
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::command::CommandResults::default_instance() as _
                })
        }
        ///If `command_results` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn command_results_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::command::CommandResults> {
            self.command_results.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `command_results`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn command_results_mut(
            &mut self,
        ) -> &mut super::super::command::CommandResults {
            self.command_results.get_or_insert_default()
        }
        ///If `command_results` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn command_results_opt(
            &self,
        ) -> Option<&super::super::command::CommandResults> {
            self.command_results.as_ref().map(|field| field as _)
        }
        ///Sets `command_results` with the provided value.
        pub fn set_command_results<T: Into<super::super::command::CommandResults>>(
            &mut self,
            field: T,
        ) {
            self.command_results = Some(field.into().into());
        }
        ///Sets `command_results` with the provided value.
        pub fn with_command_results<T: Into<super::super::command::CommandResults>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_command_results(field.into());
            self
        }
    }
}
