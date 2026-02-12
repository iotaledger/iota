// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::AddressFilter {
        /// Sets `address` with the provided value.
        pub fn with_address<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.address = Some(field.into());
            self
        }
    }
    impl super::AllEventFilter {
        /// Sets `filters` with the provided value.
        pub fn with_filters(mut self, field: Vec<super::EventFilter>) -> Self {
            self.filters = field;
            self
        }
    }
    impl super::AllTransactionFilter {
        /// Sets `filters` with the provided value.
        pub fn with_filters(mut self, field: Vec<super::TransactionFilter>) -> Self {
            self.filters = field;
            self
        }
    }
    impl super::AnyEventFilter {
        /// Sets `filters` with the provided value.
        pub fn with_filters(mut self, field: Vec<super::EventFilter>) -> Self {
            self.filters = field;
            self
        }
    }
    impl super::AnyTransactionFilter {
        /// Sets `filters` with the provided value.
        pub fn with_filters(mut self, field: Vec<super::TransactionFilter>) -> Self {
            self.filters = field;
            self
        }
    }
    impl super::EventFilter {
        /// Sets `all` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_all<T: Into<super::AllEventFilter>>(mut self, field: T) -> Self {
            self.filter = Some(super::event_filter::Filter::All(field.into()));
            self
        }
        /// Sets `any` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_any<T: Into<super::AnyEventFilter>>(mut self, field: T) -> Self {
            self.filter = Some(super::event_filter::Filter::Any(field.into()));
            self
        }
        /// Sets `negation` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_negation<T: Into<::prost::alloc::boxed::Box<super::NotEventFilter>>>(
            mut self,
            field: T,
        ) -> Self {
            self.filter = Some(super::event_filter::Filter::Negation(field.into()));
            self
        }
        /// Sets `sender` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_sender<T: Into<super::AddressFilter>>(mut self, field: T) -> Self {
            self.filter = Some(super::event_filter::Filter::Sender(field.into()));
            self
        }
        /// Sets `move_package_and_module` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_move_package_and_module<T: Into<super::MovePackageAndModuleFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.filter = Some(
                super::event_filter::Filter::MovePackageAndModule(field.into()),
            );
            self
        }
        /// Sets `move_event_package_and_module` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_move_event_package_and_module<
            T: Into<super::MovePackageAndModuleFilter>,
        >(mut self, field: T) -> Self {
            self.filter = Some(
                super::event_filter::Filter::MoveEventPackageAndModule(field.into()),
            );
            self
        }
        /// Sets `move_event_type` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_move_event_type<T: Into<super::MoveEventTypeFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.filter = Some(super::event_filter::Filter::MoveEventType(field.into()));
            self
        }
    }
    impl super::MoveCallFilter {
        /// Sets `package_id` with the provided value.
        pub fn with_package_id<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.package_id = Some(field.into());
            self
        }
        /// Sets `module` with the provided value.
        pub fn with_module<T: Into<String>>(mut self, field: T) -> Self {
            self.module = Some(field.into());
            self
        }
        /// Sets `function` with the provided value.
        pub fn with_function<T: Into<String>>(mut self, field: T) -> Self {
            self.function = Some(field.into());
            self
        }
    }
    impl super::MoveEventTypeFilter {
        /// Sets `struct_tag` with the provided value.
        pub fn with_struct_tag<T: Into<String>>(mut self, field: T) -> Self {
            self.struct_tag = field.into();
            self
        }
    }
    impl super::MovePackageAndModuleFilter {
        /// Sets `package_id` with the provided value.
        pub fn with_package_id<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.package_id = Some(field.into());
            self
        }
        /// Sets `module` with the provided value.
        pub fn with_module<T: Into<String>>(mut self, field: T) -> Self {
            self.module = Some(field.into());
            self
        }
    }
    impl super::NotEventFilter {
        /// Sets `filter` with the provided value.
        pub fn with_filter<T: Into<::prost::alloc::boxed::Box<super::EventFilter>>>(
            mut self,
            field: T,
        ) -> Self {
            self.filter = Some(field.into());
            self
        }
    }
    impl super::NotTransactionFilter {
        /// Sets `filter` with the provided value.
        pub fn with_filter<
            T: Into<::prost::alloc::boxed::Box<super::TransactionFilter>>,
        >(mut self, field: T) -> Self {
            self.filter = Some(field.into());
            self
        }
    }
    impl super::ObjectIdFilter {
        /// Sets `object_ref` with the provided value.
        pub fn with_object_ref<T: Into<super::super::types::ObjectReference>>(
            mut self,
            field: T,
        ) -> Self {
            self.object_ref = Some(field.into());
            self
        }
    }
    impl super::TransactionFilter {
        /// Sets `all` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_all<T: Into<super::AllTransactionFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.filter = Some(super::transaction_filter::Filter::All(field.into()));
            self
        }
        /// Sets `any` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_any<T: Into<super::AnyTransactionFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.filter = Some(super::transaction_filter::Filter::Any(field.into()));
            self
        }
        /// Sets `negation` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_negation<
            T: Into<::prost::alloc::boxed::Box<super::NotTransactionFilter>>,
        >(mut self, field: T) -> Self {
            self.filter = Some(
                super::transaction_filter::Filter::Negation(field.into()),
            );
            self
        }
        /// Sets `transaction_kinds` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_transaction_kinds<T: Into<super::TransactionKindsFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.filter = Some(
                super::transaction_filter::Filter::TransactionKinds(field.into()),
            );
            self
        }
        /// Sets `sender` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_sender<T: Into<super::AddressFilter>>(mut self, field: T) -> Self {
            self.filter = Some(super::transaction_filter::Filter::Sender(field.into()));
            self
        }
        /// Sets `receiver` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_receiver<T: Into<super::AddressFilter>>(mut self, field: T) -> Self {
            self.filter = Some(
                super::transaction_filter::Filter::Receiver(field.into()),
            );
            self
        }
        /// Sets `affected_object` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_affected_object<T: Into<super::ObjectIdFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.filter = Some(
                super::transaction_filter::Filter::AffectedObject(field.into()),
            );
            self
        }
        /// Sets `move_call` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_move_call<T: Into<super::MoveCallFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.filter = Some(
                super::transaction_filter::Filter::MoveCall(field.into()),
            );
            self
        }
        /// Sets `event` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_event<T: Into<super::EventFilter>>(mut self, field: T) -> Self {
            self.filter = Some(super::transaction_filter::Filter::Event(field.into()));
            self
        }
    }
    impl super::TransactionKindsFilter {
        /// Sets `kinds` with the provided value.
        pub fn with_kinds(mut self, field: Vec<i32>) -> Self {
            self.kinds = field;
            self
        }
    }
}
