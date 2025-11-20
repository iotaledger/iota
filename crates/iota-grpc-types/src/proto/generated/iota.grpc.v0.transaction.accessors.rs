// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::ExecutedTransaction {
        pub const fn const_default() -> Self {
            Self {
                digest: None,
                transaction: None,
                signatures: None,
                effects: None,
                events: None,
                checkpoint: None,
                timestamp: None,
                input_objects: None,
                output_objects: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ExecutedTransaction = super::ExecutedTransaction::const_default();
            &DEFAULT
        }
        ///Returns the value of `digest`, or the default value if `digest` is unset.
        pub fn digest(&self) -> &super::super::types::Digest {
            self.digest
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Digest::default_instance() as _)
        }
        ///If `digest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn digest_opt_mut(&mut self) -> Option<&mut super::super::types::Digest> {
            self.digest.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `digest`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn digest_mut(&mut self) -> &mut super::super::types::Digest {
            self.digest.get_or_insert_default()
        }
        ///If `digest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn digest_opt(&self) -> Option<&super::super::types::Digest> {
            self.digest.as_ref().map(|field| field as _)
        }
        ///Sets `digest` with the provided value.
        pub fn set_digest<T: Into<super::super::types::Digest>>(&mut self, field: T) {
            self.digest = Some(field.into().into());
        }
        ///Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_digest(field.into());
            self
        }
        ///Returns the value of `transaction`, or the default value if `transaction` is unset.
        pub fn transaction(&self) -> &super::Transaction {
            self.transaction
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::Transaction::default_instance() as _)
        }
        ///If `transaction` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transaction_opt_mut(&mut self) -> Option<&mut super::Transaction> {
            self.transaction.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `transaction`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transaction_mut(&mut self) -> &mut super::Transaction {
            self.transaction.get_or_insert_default()
        }
        ///If `transaction` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transaction_opt(&self) -> Option<&super::Transaction> {
            self.transaction.as_ref().map(|field| field as _)
        }
        ///Sets `transaction` with the provided value.
        pub fn set_transaction<T: Into<super::Transaction>>(&mut self, field: T) {
            self.transaction = Some(field.into().into());
        }
        ///Sets `transaction` with the provided value.
        pub fn with_transaction<T: Into<super::Transaction>>(
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
        ///Returns the value of `effects`, or the default value if `effects` is unset.
        pub fn effects(&self) -> &super::TransactionEffects {
            self.effects
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::TransactionEffects::default_instance() as _)
        }
        ///If `effects` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn effects_opt_mut(&mut self) -> Option<&mut super::TransactionEffects> {
            self.effects.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `effects`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn effects_mut(&mut self) -> &mut super::TransactionEffects {
            self.effects.get_or_insert_default()
        }
        ///If `effects` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn effects_opt(&self) -> Option<&super::TransactionEffects> {
            self.effects.as_ref().map(|field| field as _)
        }
        ///Sets `effects` with the provided value.
        pub fn set_effects<T: Into<super::TransactionEffects>>(&mut self, field: T) {
            self.effects = Some(field.into().into());
        }
        ///Sets `effects` with the provided value.
        pub fn with_effects<T: Into<super::TransactionEffects>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_effects(field.into());
            self
        }
        ///Returns the value of `events`, or the default value if `events` is unset.
        pub fn events(&self) -> &super::TransactionEvents {
            self.events
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::TransactionEvents::default_instance() as _)
        }
        ///If `events` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn events_opt_mut(&mut self) -> Option<&mut super::TransactionEvents> {
            self.events.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `events`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn events_mut(&mut self) -> &mut super::TransactionEvents {
            self.events.get_or_insert_default()
        }
        ///If `events` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn events_opt(&self) -> Option<&super::TransactionEvents> {
            self.events.as_ref().map(|field| field as _)
        }
        ///Sets `events` with the provided value.
        pub fn set_events<T: Into<super::TransactionEvents>>(&mut self, field: T) {
            self.events = Some(field.into().into());
        }
        ///Sets `events` with the provided value.
        pub fn with_events<T: Into<super::TransactionEvents>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_events(field.into());
            self
        }
        ///If `checkpoint` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn checkpoint_opt_mut(&mut self) -> Option<&mut u64> {
            self.checkpoint.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `checkpoint`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn checkpoint_mut(&mut self) -> &mut u64 {
            self.checkpoint.get_or_insert_default()
        }
        ///If `checkpoint` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn checkpoint_opt(&self) -> Option<u64> {
            self.checkpoint.as_ref().map(|field| *field)
        }
        ///Sets `checkpoint` with the provided value.
        pub fn set_checkpoint(&mut self, field: u64) {
            self.checkpoint = Some(field);
        }
        ///Sets `checkpoint` with the provided value.
        pub fn with_checkpoint(mut self, field: u64) -> Self {
            self.set_checkpoint(field);
            self
        }
        ///If `timestamp` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn timestamp_opt_mut(&mut self) -> Option<&mut ::prost_types::Timestamp> {
            self.timestamp.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `timestamp`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn timestamp_mut(&mut self) -> &mut ::prost_types::Timestamp {
            self.timestamp.get_or_insert_default()
        }
        ///If `timestamp` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn timestamp_opt(&self) -> Option<&::prost_types::Timestamp> {
            self.timestamp.as_ref().map(|field| field as _)
        }
        ///Sets `timestamp` with the provided value.
        pub fn set_timestamp<T: Into<::prost_types::Timestamp>>(&mut self, field: T) {
            self.timestamp = Some(field.into().into());
        }
        ///Sets `timestamp` with the provided value.
        pub fn with_timestamp<T: Into<::prost_types::Timestamp>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_timestamp(field.into());
            self
        }
        ///Returns the value of `input_objects`, or the default value if `input_objects` is unset.
        pub fn input_objects(&self) -> &super::super::object::Objects {
            self.input_objects
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::object::Objects::default_instance() as _
                })
        }
        ///If `input_objects` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn input_objects_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::object::Objects> {
            self.input_objects.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `input_objects`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn input_objects_mut(&mut self) -> &mut super::super::object::Objects {
            self.input_objects.get_or_insert_default()
        }
        ///If `input_objects` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn input_objects_opt(&self) -> Option<&super::super::object::Objects> {
            self.input_objects.as_ref().map(|field| field as _)
        }
        ///Sets `input_objects` with the provided value.
        pub fn set_input_objects<T: Into<super::super::object::Objects>>(
            &mut self,
            field: T,
        ) {
            self.input_objects = Some(field.into().into());
        }
        ///Sets `input_objects` with the provided value.
        pub fn with_input_objects<T: Into<super::super::object::Objects>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_input_objects(field.into());
            self
        }
        ///Returns the value of `output_objects`, or the default value if `output_objects` is unset.
        pub fn output_objects(&self) -> &super::super::object::Objects {
            self.output_objects
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::object::Objects::default_instance() as _
                })
        }
        ///If `output_objects` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn output_objects_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::object::Objects> {
            self.output_objects.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `output_objects`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn output_objects_mut(&mut self) -> &mut super::super::object::Objects {
            self.output_objects.get_or_insert_default()
        }
        ///If `output_objects` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn output_objects_opt(&self) -> Option<&super::super::object::Objects> {
            self.output_objects.as_ref().map(|field| field as _)
        }
        ///Sets `output_objects` with the provided value.
        pub fn set_output_objects<T: Into<super::super::object::Objects>>(
            &mut self,
            field: T,
        ) {
            self.output_objects = Some(field.into().into());
        }
        ///Sets `output_objects` with the provided value.
        pub fn with_output_objects<T: Into<super::super::object::Objects>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_output_objects(field.into());
            self
        }
    }
    impl super::ExecutedTransactions {
        pub const fn const_default() -> Self {
            Self { transactions: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ExecutedTransactions = super::ExecutedTransactions::const_default();
            &DEFAULT
        }
        ///Returns the value of `transactions`, or the default value if `transactions` is unset.
        pub fn transactions(&self) -> &[super::ExecutedTransaction] {
            &self.transactions
        }
        ///Returns a mutable reference to `transactions`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transactions_mut(&mut self) -> &mut Vec<super::ExecutedTransaction> {
            &mut self.transactions
        }
        ///Sets `transactions` with the provided value.
        pub fn set_transactions(&mut self, field: Vec<super::ExecutedTransaction>) {
            self.transactions = field;
        }
        ///Sets `transactions` with the provided value.
        pub fn with_transactions(
            mut self,
            field: Vec<super::ExecutedTransaction>,
        ) -> Self {
            self.set_transactions(field);
            self
        }
    }
    impl super::Transaction {
        pub const fn const_default() -> Self {
            Self { digest: None, bcs: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Transaction = super::Transaction::const_default();
            &DEFAULT
        }
        ///Returns the value of `digest`, or the default value if `digest` is unset.
        pub fn digest(&self) -> &super::super::types::Digest {
            self.digest
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Digest::default_instance() as _)
        }
        ///If `digest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn digest_opt_mut(&mut self) -> Option<&mut super::super::types::Digest> {
            self.digest.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `digest`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn digest_mut(&mut self) -> &mut super::super::types::Digest {
            self.digest.get_or_insert_default()
        }
        ///If `digest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn digest_opt(&self) -> Option<&super::super::types::Digest> {
            self.digest.as_ref().map(|field| field as _)
        }
        ///Sets `digest` with the provided value.
        pub fn set_digest<T: Into<super::super::types::Digest>>(&mut self, field: T) {
            self.digest = Some(field.into().into());
        }
        ///Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_digest(field.into());
            self
        }
        ///Returns the value of `bcs`, or the default value if `bcs` is unset.
        pub fn bcs(&self) -> &super::super::bcs::BcsData {
            self.bcs
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::bcs::BcsData::default_instance() as _)
        }
        ///If `bcs` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn bcs_opt_mut(&mut self) -> Option<&mut super::super::bcs::BcsData> {
            self.bcs.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `bcs`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn bcs_mut(&mut self) -> &mut super::super::bcs::BcsData {
            self.bcs.get_or_insert_default()
        }
        ///If `bcs` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn bcs_opt(&self) -> Option<&super::super::bcs::BcsData> {
            self.bcs.as_ref().map(|field| field as _)
        }
        ///Sets `bcs` with the provided value.
        pub fn set_bcs<T: Into<super::super::bcs::BcsData>>(&mut self, field: T) {
            self.bcs = Some(field.into().into());
        }
        ///Sets `bcs` with the provided value.
        pub fn with_bcs<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_bcs(field.into());
            self
        }
    }
    impl super::TransactionEffects {
        pub const fn const_default() -> Self {
            Self { digest: None, bcs: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TransactionEffects = super::TransactionEffects::const_default();
            &DEFAULT
        }
        ///Returns the value of `digest`, or the default value if `digest` is unset.
        pub fn digest(&self) -> &super::super::types::Digest {
            self.digest
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Digest::default_instance() as _)
        }
        ///If `digest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn digest_opt_mut(&mut self) -> Option<&mut super::super::types::Digest> {
            self.digest.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `digest`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn digest_mut(&mut self) -> &mut super::super::types::Digest {
            self.digest.get_or_insert_default()
        }
        ///If `digest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn digest_opt(&self) -> Option<&super::super::types::Digest> {
            self.digest.as_ref().map(|field| field as _)
        }
        ///Sets `digest` with the provided value.
        pub fn set_digest<T: Into<super::super::types::Digest>>(&mut self, field: T) {
            self.digest = Some(field.into().into());
        }
        ///Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_digest(field.into());
            self
        }
        ///Returns the value of `bcs`, or the default value if `bcs` is unset.
        pub fn bcs(&self) -> &super::super::bcs::BcsData {
            self.bcs
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::bcs::BcsData::default_instance() as _)
        }
        ///If `bcs` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn bcs_opt_mut(&mut self) -> Option<&mut super::super::bcs::BcsData> {
            self.bcs.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `bcs`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn bcs_mut(&mut self) -> &mut super::super::bcs::BcsData {
            self.bcs.get_or_insert_default()
        }
        ///If `bcs` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn bcs_opt(&self) -> Option<&super::super::bcs::BcsData> {
            self.bcs.as_ref().map(|field| field as _)
        }
        ///Sets `bcs` with the provided value.
        pub fn set_bcs<T: Into<super::super::bcs::BcsData>>(&mut self, field: T) {
            self.bcs = Some(field.into().into());
        }
        ///Sets `bcs` with the provided value.
        pub fn with_bcs<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_bcs(field.into());
            self
        }
    }
    impl super::TransactionEvents {
        pub const fn const_default() -> Self {
            Self { digest: None, events: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TransactionEvents = super::TransactionEvents::const_default();
            &DEFAULT
        }
        ///Returns the value of `digest`, or the default value if `digest` is unset.
        pub fn digest(&self) -> &super::super::types::Digest {
            self.digest
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Digest::default_instance() as _)
        }
        ///If `digest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn digest_opt_mut(&mut self) -> Option<&mut super::super::types::Digest> {
            self.digest.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `digest`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn digest_mut(&mut self) -> &mut super::super::types::Digest {
            self.digest.get_or_insert_default()
        }
        ///If `digest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn digest_opt(&self) -> Option<&super::super::types::Digest> {
            self.digest.as_ref().map(|field| field as _)
        }
        ///Sets `digest` with the provided value.
        pub fn set_digest<T: Into<super::super::types::Digest>>(&mut self, field: T) {
            self.digest = Some(field.into().into());
        }
        ///Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_digest(field.into());
            self
        }
        ///Returns the value of `events`, or the default value if `events` is unset.
        pub fn events(&self) -> &super::super::event::Events {
            self.events
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::event::Events::default_instance() as _)
        }
        ///If `events` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn events_opt_mut(&mut self) -> Option<&mut super::super::event::Events> {
            self.events.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `events`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn events_mut(&mut self) -> &mut super::super::event::Events {
            self.events.get_or_insert_default()
        }
        ///If `events` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn events_opt(&self) -> Option<&super::super::event::Events> {
            self.events.as_ref().map(|field| field as _)
        }
        ///Sets `events` with the provided value.
        pub fn set_events<T: Into<super::super::event::Events>>(&mut self, field: T) {
            self.events = Some(field.into().into());
        }
        ///Sets `events` with the provided value.
        pub fn with_events<T: Into<super::super::event::Events>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_events(field.into());
            self
        }
    }
}
