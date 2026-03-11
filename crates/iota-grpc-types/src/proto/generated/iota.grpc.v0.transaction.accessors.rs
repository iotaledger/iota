// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::ExecutedTransaction {
        /// Sets `transaction` with the provided value.
        pub fn with_transaction<T: Into<super::Transaction>>(
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
        /// Sets `effects` with the provided value.
        pub fn with_effects<T: Into<super::TransactionEffects>>(
            mut self,
            field: T,
        ) -> Self {
            self.effects = Some(field.into());
            self
        }
        /// Sets `events` with the provided value.
        pub fn with_events<T: Into<super::TransactionEvents>>(
            mut self,
            field: T,
        ) -> Self {
            self.events = Some(field.into());
            self
        }
        /// Sets `checkpoint` with the provided value.
        pub fn with_checkpoint(mut self, field: u64) -> Self {
            self.checkpoint = Some(field);
            self
        }
        /// Sets `timestamp` with the provided value.
        pub fn with_timestamp<T: Into<::prost_types::Timestamp>>(
            mut self,
            field: T,
        ) -> Self {
            self.timestamp = Some(field.into());
            self
        }
        /// Sets `input_objects` with the provided value.
        pub fn with_input_objects<T: Into<super::super::object::Objects>>(
            mut self,
            field: T,
        ) -> Self {
            self.input_objects = Some(field.into());
            self
        }
        /// Sets `output_objects` with the provided value.
        pub fn with_output_objects<T: Into<super::super::object::Objects>>(
            mut self,
            field: T,
        ) -> Self {
            self.output_objects = Some(field.into());
            self
        }
    }
    impl super::ExecutedTransactions {
        /// Sets `executed_transactions` with the provided value.
        pub fn with_executed_transactions(
            mut self,
            field: Vec<super::ExecutedTransaction>,
        ) -> Self {
            self.executed_transactions = field;
            self
        }
    }
    impl super::Transaction {
        /// Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.digest = Some(field.into());
            self
        }
        /// Sets `bcs` with the provided value.
        pub fn with_bcs<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.bcs = Some(field.into());
            self
        }
    }
    impl super::TransactionEffects {
        /// Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.digest = Some(field.into());
            self
        }
        /// Sets `bcs` with the provided value.
        pub fn with_bcs<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.bcs = Some(field.into());
            self
        }
    }
    impl super::TransactionEvents {
        /// Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.digest = Some(field.into());
            self
        }
        /// Sets `events` with the provided value.
        pub fn with_events<T: Into<super::super::event::Events>>(
            mut self,
            field: T,
        ) -> Self {
            self.events = Some(field.into());
            self
        }
    }
}
