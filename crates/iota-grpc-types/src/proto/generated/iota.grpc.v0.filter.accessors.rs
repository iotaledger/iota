// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::AddressFilter {
        pub const fn const_default() -> Self {
            Self { address: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::AddressFilter = super::AddressFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `address`, or the default value if `address` is unset.
        pub fn address(&self) -> &super::super::types::Address {
            self.address
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Address::default_instance() as _)
        }
        ///If `address` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn address_opt_mut(&mut self) -> Option<&mut super::super::types::Address> {
            self.address.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `address`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn address_mut(&mut self) -> &mut super::super::types::Address {
            self.address.get_or_insert_default()
        }
        ///If `address` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn address_opt(&self) -> Option<&super::super::types::Address> {
            self.address.as_ref().map(|field| field as _)
        }
        ///Sets `address` with the provided value.
        pub fn set_address<T: Into<super::super::types::Address>>(&mut self, field: T) {
            self.address = Some(field.into().into());
        }
        ///Sets `address` with the provided value.
        pub fn with_address<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_address(field.into());
            self
        }
    }
    impl super::AllEventFilter {
        pub const fn const_default() -> Self {
            Self { filters: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::AllEventFilter = super::AllEventFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `filters`, or the default value if `filters` is unset.
        pub fn filters(&self) -> &[super::EventFilter] {
            &self.filters
        }
        ///Returns a mutable reference to `filters`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn filters_mut(&mut self) -> &mut Vec<super::EventFilter> {
            &mut self.filters
        }
        ///Sets `filters` with the provided value.
        pub fn set_filters(&mut self, field: Vec<super::EventFilter>) {
            self.filters = field;
        }
        ///Sets `filters` with the provided value.
        pub fn with_filters(mut self, field: Vec<super::EventFilter>) -> Self {
            self.set_filters(field);
            self
        }
    }
    impl super::AllTransactionFilter {
        pub const fn const_default() -> Self {
            Self { filters: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::AllTransactionFilter = super::AllTransactionFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `filters`, or the default value if `filters` is unset.
        pub fn filters(&self) -> &[super::TransactionFilter] {
            &self.filters
        }
        ///Returns a mutable reference to `filters`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn filters_mut(&mut self) -> &mut Vec<super::TransactionFilter> {
            &mut self.filters
        }
        ///Sets `filters` with the provided value.
        pub fn set_filters(&mut self, field: Vec<super::TransactionFilter>) {
            self.filters = field;
        }
        ///Sets `filters` with the provided value.
        pub fn with_filters(mut self, field: Vec<super::TransactionFilter>) -> Self {
            self.set_filters(field);
            self
        }
    }
    impl super::AnyEventFilter {
        pub const fn const_default() -> Self {
            Self { filters: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::AnyEventFilter = super::AnyEventFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `filters`, or the default value if `filters` is unset.
        pub fn filters(&self) -> &[super::EventFilter] {
            &self.filters
        }
        ///Returns a mutable reference to `filters`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn filters_mut(&mut self) -> &mut Vec<super::EventFilter> {
            &mut self.filters
        }
        ///Sets `filters` with the provided value.
        pub fn set_filters(&mut self, field: Vec<super::EventFilter>) {
            self.filters = field;
        }
        ///Sets `filters` with the provided value.
        pub fn with_filters(mut self, field: Vec<super::EventFilter>) -> Self {
            self.set_filters(field);
            self
        }
    }
    impl super::AnyTransactionFilter {
        pub const fn const_default() -> Self {
            Self { filters: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::AnyTransactionFilter = super::AnyTransactionFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `filters`, or the default value if `filters` is unset.
        pub fn filters(&self) -> &[super::TransactionFilter] {
            &self.filters
        }
        ///Returns a mutable reference to `filters`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn filters_mut(&mut self) -> &mut Vec<super::TransactionFilter> {
            &mut self.filters
        }
        ///Sets `filters` with the provided value.
        pub fn set_filters(&mut self, field: Vec<super::TransactionFilter>) {
            self.filters = field;
        }
        ///Sets `filters` with the provided value.
        pub fn with_filters(mut self, field: Vec<super::TransactionFilter>) -> Self {
            self.set_filters(field);
            self
        }
    }
    impl super::EventFilter {
        pub const fn const_default() -> Self {
            Self { filter: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::EventFilter = super::EventFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `all`, or the default value if `all` is unset.
        pub fn all(&self) -> &super::AllEventFilter {
            if let Some(super::event_filter::Filter::All(field)) = &self.filter {
                field as _
            } else {
                super::AllEventFilter::default_instance() as _
            }
        }
        ///If `all` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn all_opt(&self) -> Option<&super::AllEventFilter> {
            if let Some(super::event_filter::Filter::All(field)) = &self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `all` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn all_opt_mut(&mut self) -> Option<&mut super::AllEventFilter> {
            if let Some(super::event_filter::Filter::All(field)) = &mut self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `all`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn all_mut(&mut self) -> &mut super::AllEventFilter {
            if self.all_opt_mut().is_none() {
                self.filter = Some(
                    super::event_filter::Filter::All(super::AllEventFilter::default()),
                );
            }
            self.all_opt_mut().unwrap()
        }
        ///Sets `all` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_all<T: Into<super::AllEventFilter>>(&mut self, field: T) {
            self.filter = Some(super::event_filter::Filter::All(field.into().into()));
        }
        ///Sets `all` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_all<T: Into<super::AllEventFilter>>(mut self, field: T) -> Self {
            self.set_all(field.into());
            self
        }
        ///Returns the value of `any`, or the default value if `any` is unset.
        pub fn any(&self) -> &super::AnyEventFilter {
            if let Some(super::event_filter::Filter::Any(field)) = &self.filter {
                field as _
            } else {
                super::AnyEventFilter::default_instance() as _
            }
        }
        ///If `any` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn any_opt(&self) -> Option<&super::AnyEventFilter> {
            if let Some(super::event_filter::Filter::Any(field)) = &self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `any` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn any_opt_mut(&mut self) -> Option<&mut super::AnyEventFilter> {
            if let Some(super::event_filter::Filter::Any(field)) = &mut self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `any`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn any_mut(&mut self) -> &mut super::AnyEventFilter {
            if self.any_opt_mut().is_none() {
                self.filter = Some(
                    super::event_filter::Filter::Any(super::AnyEventFilter::default()),
                );
            }
            self.any_opt_mut().unwrap()
        }
        ///Sets `any` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_any<T: Into<super::AnyEventFilter>>(&mut self, field: T) {
            self.filter = Some(super::event_filter::Filter::Any(field.into().into()));
        }
        ///Sets `any` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_any<T: Into<super::AnyEventFilter>>(mut self, field: T) -> Self {
            self.set_any(field.into());
            self
        }
        ///Returns the value of `negation`, or the default value if `negation` is unset.
        pub fn negation(&self) -> &super::NotEventFilter {
            if let Some(super::event_filter::Filter::Negation(field)) = &self.filter {
                field as _
            } else {
                super::NotEventFilter::default_instance() as _
            }
        }
        ///If `negation` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn negation_opt(&self) -> Option<&super::NotEventFilter> {
            if let Some(super::event_filter::Filter::Negation(field)) = &self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `negation` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn negation_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost::alloc::boxed::Box<super::NotEventFilter>> {
            if let Some(super::event_filter::Filter::Negation(field)) = &mut self.filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `negation`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn negation_mut(
            &mut self,
        ) -> &mut ::prost::alloc::boxed::Box<super::NotEventFilter> {
            if self.negation_opt_mut().is_none() {
                self.filter = Some(
                    super::event_filter::Filter::Negation(
                        ::prost::alloc::boxed::Box::default(),
                    ),
                );
            }
            self.negation_opt_mut().unwrap()
        }
        ///Sets `negation` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_negation<T: Into<::prost::alloc::boxed::Box<super::NotEventFilter>>>(
            &mut self,
            field: T,
        ) {
            self.filter = Some(
                super::event_filter::Filter::Negation(field.into().into()),
            );
        }
        ///Sets `negation` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_negation<T: Into<::prost::alloc::boxed::Box<super::NotEventFilter>>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_negation(field.into());
            self
        }
        ///Returns the value of `sender`, or the default value if `sender` is unset.
        pub fn sender(&self) -> &super::AddressFilter {
            if let Some(super::event_filter::Filter::Sender(field)) = &self.filter {
                field as _
            } else {
                super::AddressFilter::default_instance() as _
            }
        }
        ///If `sender` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn sender_opt(&self) -> Option<&super::AddressFilter> {
            if let Some(super::event_filter::Filter::Sender(field)) = &self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `sender` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn sender_opt_mut(&mut self) -> Option<&mut super::AddressFilter> {
            if let Some(super::event_filter::Filter::Sender(field)) = &mut self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `sender`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn sender_mut(&mut self) -> &mut super::AddressFilter {
            if self.sender_opt_mut().is_none() {
                self.filter = Some(
                    super::event_filter::Filter::Sender(super::AddressFilter::default()),
                );
            }
            self.sender_opt_mut().unwrap()
        }
        ///Sets `sender` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_sender<T: Into<super::AddressFilter>>(&mut self, field: T) {
            self.filter = Some(super::event_filter::Filter::Sender(field.into().into()));
        }
        ///Sets `sender` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_sender<T: Into<super::AddressFilter>>(mut self, field: T) -> Self {
            self.set_sender(field.into());
            self
        }
        ///Returns the value of `move_package_and_module`, or the default value if `move_package_and_module` is unset.
        pub fn move_package_and_module(&self) -> &super::MovePackageAndModuleFilter {
            if let Some(super::event_filter::Filter::MovePackageAndModule(field)) = &self
                .filter
            {
                field as _
            } else {
                super::MovePackageAndModuleFilter::default_instance() as _
            }
        }
        ///If `move_package_and_module` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn move_package_and_module_opt(
            &self,
        ) -> Option<&super::MovePackageAndModuleFilter> {
            if let Some(super::event_filter::Filter::MovePackageAndModule(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `move_package_and_module` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn move_package_and_module_opt_mut(
            &mut self,
        ) -> Option<&mut super::MovePackageAndModuleFilter> {
            if let Some(super::event_filter::Filter::MovePackageAndModule(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `move_package_and_module`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn move_package_and_module_mut(
            &mut self,
        ) -> &mut super::MovePackageAndModuleFilter {
            if self.move_package_and_module_opt_mut().is_none() {
                self.filter = Some(
                    super::event_filter::Filter::MovePackageAndModule(
                        super::MovePackageAndModuleFilter::default(),
                    ),
                );
            }
            self.move_package_and_module_opt_mut().unwrap()
        }
        ///Sets `move_package_and_module` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_move_package_and_module<T: Into<super::MovePackageAndModuleFilter>>(
            &mut self,
            field: T,
        ) {
            self.filter = Some(
                super::event_filter::Filter::MovePackageAndModule(field.into().into()),
            );
        }
        ///Sets `move_package_and_module` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_move_package_and_module<T: Into<super::MovePackageAndModuleFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_move_package_and_module(field.into());
            self
        }
        ///Returns the value of `move_event_package_and_module`, or the default value if `move_event_package_and_module` is unset.
        pub fn move_event_package_and_module(
            &self,
        ) -> &super::MovePackageAndModuleFilter {
            if let Some(super::event_filter::Filter::MoveEventPackageAndModule(field)) = &self
                .filter
            {
                field as _
            } else {
                super::MovePackageAndModuleFilter::default_instance() as _
            }
        }
        ///If `move_event_package_and_module` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn move_event_package_and_module_opt(
            &self,
        ) -> Option<&super::MovePackageAndModuleFilter> {
            if let Some(super::event_filter::Filter::MoveEventPackageAndModule(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `move_event_package_and_module` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn move_event_package_and_module_opt_mut(
            &mut self,
        ) -> Option<&mut super::MovePackageAndModuleFilter> {
            if let Some(super::event_filter::Filter::MoveEventPackageAndModule(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `move_event_package_and_module`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn move_event_package_and_module_mut(
            &mut self,
        ) -> &mut super::MovePackageAndModuleFilter {
            if self.move_event_package_and_module_opt_mut().is_none() {
                self.filter = Some(
                    super::event_filter::Filter::MoveEventPackageAndModule(
                        super::MovePackageAndModuleFilter::default(),
                    ),
                );
            }
            self.move_event_package_and_module_opt_mut().unwrap()
        }
        ///Sets `move_event_package_and_module` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_move_event_package_and_module<
            T: Into<super::MovePackageAndModuleFilter>,
        >(&mut self, field: T) {
            self.filter = Some(
                super::event_filter::Filter::MoveEventPackageAndModule(
                    field.into().into(),
                ),
            );
        }
        ///Sets `move_event_package_and_module` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_move_event_package_and_module<
            T: Into<super::MovePackageAndModuleFilter>,
        >(mut self, field: T) -> Self {
            self.set_move_event_package_and_module(field.into());
            self
        }
        ///Returns the value of `move_event_type`, or the default value if `move_event_type` is unset.
        pub fn move_event_type(&self) -> &super::MoveEventTypeFilter {
            if let Some(super::event_filter::Filter::MoveEventType(field)) = &self.filter
            {
                field as _
            } else {
                super::MoveEventTypeFilter::default_instance() as _
            }
        }
        ///If `move_event_type` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn move_event_type_opt(&self) -> Option<&super::MoveEventTypeFilter> {
            if let Some(super::event_filter::Filter::MoveEventType(field)) = &self.filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `move_event_type` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn move_event_type_opt_mut(
            &mut self,
        ) -> Option<&mut super::MoveEventTypeFilter> {
            if let Some(super::event_filter::Filter::MoveEventType(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `move_event_type`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn move_event_type_mut(&mut self) -> &mut super::MoveEventTypeFilter {
            if self.move_event_type_opt_mut().is_none() {
                self.filter = Some(
                    super::event_filter::Filter::MoveEventType(
                        super::MoveEventTypeFilter::default(),
                    ),
                );
            }
            self.move_event_type_opt_mut().unwrap()
        }
        ///Sets `move_event_type` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_move_event_type<T: Into<super::MoveEventTypeFilter>>(
            &mut self,
            field: T,
        ) {
            self.filter = Some(
                super::event_filter::Filter::MoveEventType(field.into().into()),
            );
        }
        ///Sets `move_event_type` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_move_event_type<T: Into<super::MoveEventTypeFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_move_event_type(field.into());
            self
        }
        ///Returns the value of `move_event_field`, or the default value if `move_event_field` is unset.
        pub fn move_event_field(&self) -> &super::MoveEventFieldFilter {
            if let Some(super::event_filter::Filter::MoveEventField(field)) = &self
                .filter
            {
                field as _
            } else {
                super::MoveEventFieldFilter::default_instance() as _
            }
        }
        ///If `move_event_field` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn move_event_field_opt(&self) -> Option<&super::MoveEventFieldFilter> {
            if let Some(super::event_filter::Filter::MoveEventField(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `move_event_field` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn move_event_field_opt_mut(
            &mut self,
        ) -> Option<&mut super::MoveEventFieldFilter> {
            if let Some(super::event_filter::Filter::MoveEventField(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `move_event_field`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn move_event_field_mut(&mut self) -> &mut super::MoveEventFieldFilter {
            if self.move_event_field_opt_mut().is_none() {
                self.filter = Some(
                    super::event_filter::Filter::MoveEventField(
                        super::MoveEventFieldFilter::default(),
                    ),
                );
            }
            self.move_event_field_opt_mut().unwrap()
        }
        ///Sets `move_event_field` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_move_event_field<T: Into<super::MoveEventFieldFilter>>(
            &mut self,
            field: T,
        ) {
            self.filter = Some(
                super::event_filter::Filter::MoveEventField(field.into().into()),
            );
        }
        ///Sets `move_event_field` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_move_event_field<T: Into<super::MoveEventFieldFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_move_event_field(field.into());
            self
        }
    }
    impl super::MoveCallFilter {
        pub const fn const_default() -> Self {
            Self {
                package_id: None,
                module: None,
                function: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::MoveCallFilter = super::MoveCallFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `package_id`, or the default value if `package_id` is unset.
        pub fn package_id(&self) -> &super::super::types::Address {
            self.package_id
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Address::default_instance() as _)
        }
        ///If `package_id` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn package_id_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::types::Address> {
            self.package_id.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `package_id`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn package_id_mut(&mut self) -> &mut super::super::types::Address {
            self.package_id.get_or_insert_default()
        }
        ///If `package_id` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn package_id_opt(&self) -> Option<&super::super::types::Address> {
            self.package_id.as_ref().map(|field| field as _)
        }
        ///Sets `package_id` with the provided value.
        pub fn set_package_id<T: Into<super::super::types::Address>>(
            &mut self,
            field: T,
        ) {
            self.package_id = Some(field.into().into());
        }
        ///Sets `package_id` with the provided value.
        pub fn with_package_id<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_package_id(field.into());
            self
        }
        ///If `module` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn module_opt_mut(&mut self) -> Option<&mut String> {
            self.module.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `module`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn module_mut(&mut self) -> &mut String {
            self.module.get_or_insert_default()
        }
        ///If `module` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn module_opt(&self) -> Option<&str> {
            self.module.as_ref().map(|field| field as _)
        }
        ///Sets `module` with the provided value.
        pub fn set_module<T: Into<String>>(&mut self, field: T) {
            self.module = Some(field.into().into());
        }
        ///Sets `module` with the provided value.
        pub fn with_module<T: Into<String>>(mut self, field: T) -> Self {
            self.set_module(field.into());
            self
        }
        ///If `function` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn function_opt_mut(&mut self) -> Option<&mut String> {
            self.function.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `function`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn function_mut(&mut self) -> &mut String {
            self.function.get_or_insert_default()
        }
        ///If `function` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn function_opt(&self) -> Option<&str> {
            self.function.as_ref().map(|field| field as _)
        }
        ///Sets `function` with the provided value.
        pub fn set_function<T: Into<String>>(&mut self, field: T) {
            self.function = Some(field.into().into());
        }
        ///Sets `function` with the provided value.
        pub fn with_function<T: Into<String>>(mut self, field: T) -> Self {
            self.set_function(field.into());
            self
        }
    }
    impl super::MoveEventFieldFilter {
        pub const fn const_default() -> Self {
            Self {
                json_pointer: String::new(),
                value: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::MoveEventFieldFilter = super::MoveEventFieldFilter::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `json_pointer`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn json_pointer_mut(&mut self) -> &mut String {
            &mut self.json_pointer
        }
        ///Sets `json_pointer` with the provided value.
        pub fn set_json_pointer<T: Into<String>>(&mut self, field: T) {
            self.json_pointer = field.into().into();
        }
        ///Sets `json_pointer` with the provided value.
        pub fn with_json_pointer<T: Into<String>>(mut self, field: T) -> Self {
            self.set_json_pointer(field.into());
            self
        }
        ///If `value` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn value_opt_mut(&mut self) -> Option<&mut String> {
            self.value.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `value`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn value_mut(&mut self) -> &mut String {
            self.value.get_or_insert_default()
        }
        ///If `value` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn value_opt(&self) -> Option<&str> {
            self.value.as_ref().map(|field| field as _)
        }
        ///Sets `value` with the provided value.
        pub fn set_value<T: Into<String>>(&mut self, field: T) {
            self.value = Some(field.into().into());
        }
        ///Sets `value` with the provided value.
        pub fn with_value<T: Into<String>>(mut self, field: T) -> Self {
            self.set_value(field.into());
            self
        }
    }
    impl super::MoveEventTypeFilter {
        pub const fn const_default() -> Self {
            Self { struct_tag: String::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::MoveEventTypeFilter = super::MoveEventTypeFilter::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `struct_tag`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn struct_tag_mut(&mut self) -> &mut String {
            &mut self.struct_tag
        }
        ///Sets `struct_tag` with the provided value.
        pub fn set_struct_tag<T: Into<String>>(&mut self, field: T) {
            self.struct_tag = field.into().into();
        }
        ///Sets `struct_tag` with the provided value.
        pub fn with_struct_tag<T: Into<String>>(mut self, field: T) -> Self {
            self.set_struct_tag(field.into());
            self
        }
    }
    impl super::MovePackageAndModuleFilter {
        pub const fn const_default() -> Self {
            Self {
                package_id: None,
                module: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::MovePackageAndModuleFilter = super::MovePackageAndModuleFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `package_id`, or the default value if `package_id` is unset.
        pub fn package_id(&self) -> &super::super::types::Address {
            self.package_id
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Address::default_instance() as _)
        }
        ///If `package_id` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn package_id_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::types::Address> {
            self.package_id.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `package_id`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn package_id_mut(&mut self) -> &mut super::super::types::Address {
            self.package_id.get_or_insert_default()
        }
        ///If `package_id` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn package_id_opt(&self) -> Option<&super::super::types::Address> {
            self.package_id.as_ref().map(|field| field as _)
        }
        ///Sets `package_id` with the provided value.
        pub fn set_package_id<T: Into<super::super::types::Address>>(
            &mut self,
            field: T,
        ) {
            self.package_id = Some(field.into().into());
        }
        ///Sets `package_id` with the provided value.
        pub fn with_package_id<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_package_id(field.into());
            self
        }
        ///If `module` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn module_opt_mut(&mut self) -> Option<&mut String> {
            self.module.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `module`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn module_mut(&mut self) -> &mut String {
            self.module.get_or_insert_default()
        }
        ///If `module` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn module_opt(&self) -> Option<&str> {
            self.module.as_ref().map(|field| field as _)
        }
        ///Sets `module` with the provided value.
        pub fn set_module<T: Into<String>>(&mut self, field: T) {
            self.module = Some(field.into().into());
        }
        ///Sets `module` with the provided value.
        pub fn with_module<T: Into<String>>(mut self, field: T) -> Self {
            self.set_module(field.into());
            self
        }
    }
    impl super::NotEventFilter {
        pub const fn const_default() -> Self {
            Self { filter: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::NotEventFilter = super::NotEventFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `filter`, or the default value if `filter` is unset.
        pub fn filter(&self) -> &super::EventFilter {
            self.filter
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::EventFilter::default_instance() as _)
        }
        ///If `filter` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn filter_opt_mut(&mut self) -> Option<&mut super::EventFilter> {
            self.filter.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `filter`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn filter_mut(&mut self) -> &mut super::EventFilter {
            self.filter.get_or_insert_default()
        }
        ///If `filter` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn filter_opt(&self) -> Option<&super::EventFilter> {
            self.filter.as_ref().map(|field| field as _)
        }
        ///Sets `filter` with the provided value.
        pub fn set_filter<T: Into<super::EventFilter>>(&mut self, field: T) {
            self.filter = Some(field.into().into());
        }
        ///Sets `filter` with the provided value.
        pub fn with_filter<T: Into<super::EventFilter>>(mut self, field: T) -> Self {
            self.set_filter(field.into());
            self
        }
    }
    impl super::NotTransactionFilter {
        pub const fn const_default() -> Self {
            Self { filter: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::NotTransactionFilter = super::NotTransactionFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `filter`, or the default value if `filter` is unset.
        pub fn filter(&self) -> &super::TransactionFilter {
            self.filter
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::TransactionFilter::default_instance() as _)
        }
        ///If `filter` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn filter_opt_mut(&mut self) -> Option<&mut super::TransactionFilter> {
            self.filter.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `filter`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn filter_mut(&mut self) -> &mut super::TransactionFilter {
            self.filter.get_or_insert_default()
        }
        ///If `filter` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn filter_opt(&self) -> Option<&super::TransactionFilter> {
            self.filter.as_ref().map(|field| field as _)
        }
        ///Sets `filter` with the provided value.
        pub fn set_filter<T: Into<super::TransactionFilter>>(&mut self, field: T) {
            self.filter = Some(field.into().into());
        }
        ///Sets `filter` with the provided value.
        pub fn with_filter<T: Into<super::TransactionFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_filter(field.into());
            self
        }
    }
    impl super::ObjectIdFilter {
        pub const fn const_default() -> Self {
            Self { object_ref: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ObjectIdFilter = super::ObjectIdFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `object_ref`, or the default value if `object_ref` is unset.
        pub fn object_ref(&self) -> &super::super::types::ObjectReference {
            self.object_ref
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::types::ObjectReference::default_instance() as _
                })
        }
        ///If `object_ref` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn object_ref_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::types::ObjectReference> {
            self.object_ref.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `object_ref`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn object_ref_mut(&mut self) -> &mut super::super::types::ObjectReference {
            self.object_ref.get_or_insert_default()
        }
        ///If `object_ref` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn object_ref_opt(&self) -> Option<&super::super::types::ObjectReference> {
            self.object_ref.as_ref().map(|field| field as _)
        }
        ///Sets `object_ref` with the provided value.
        pub fn set_object_ref<T: Into<super::super::types::ObjectReference>>(
            &mut self,
            field: T,
        ) {
            self.object_ref = Some(field.into().into());
        }
        ///Sets `object_ref` with the provided value.
        pub fn with_object_ref<T: Into<super::super::types::ObjectReference>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_object_ref(field.into());
            self
        }
    }
    impl super::TransactionFilter {
        pub const fn const_default() -> Self {
            Self { filter: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TransactionFilter = super::TransactionFilter::const_default();
            &DEFAULT
        }
        ///Returns the value of `all`, or the default value if `all` is unset.
        pub fn all(&self) -> &super::AllTransactionFilter {
            if let Some(super::transaction_filter::Filter::All(field)) = &self.filter {
                field as _
            } else {
                super::AllTransactionFilter::default_instance() as _
            }
        }
        ///If `all` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn all_opt(&self) -> Option<&super::AllTransactionFilter> {
            if let Some(super::transaction_filter::Filter::All(field)) = &self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `all` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn all_opt_mut(&mut self) -> Option<&mut super::AllTransactionFilter> {
            if let Some(super::transaction_filter::Filter::All(field)) = &mut self.filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `all`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn all_mut(&mut self) -> &mut super::AllTransactionFilter {
            if self.all_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::All(
                        super::AllTransactionFilter::default(),
                    ),
                );
            }
            self.all_opt_mut().unwrap()
        }
        ///Sets `all` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_all<T: Into<super::AllTransactionFilter>>(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::All(field.into().into()),
            );
        }
        ///Sets `all` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_all<T: Into<super::AllTransactionFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_all(field.into());
            self
        }
        ///Returns the value of `any`, or the default value if `any` is unset.
        pub fn any(&self) -> &super::AnyTransactionFilter {
            if let Some(super::transaction_filter::Filter::Any(field)) = &self.filter {
                field as _
            } else {
                super::AnyTransactionFilter::default_instance() as _
            }
        }
        ///If `any` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn any_opt(&self) -> Option<&super::AnyTransactionFilter> {
            if let Some(super::transaction_filter::Filter::Any(field)) = &self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `any` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn any_opt_mut(&mut self) -> Option<&mut super::AnyTransactionFilter> {
            if let Some(super::transaction_filter::Filter::Any(field)) = &mut self.filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `any`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn any_mut(&mut self) -> &mut super::AnyTransactionFilter {
            if self.any_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::Any(
                        super::AnyTransactionFilter::default(),
                    ),
                );
            }
            self.any_opt_mut().unwrap()
        }
        ///Sets `any` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_any<T: Into<super::AnyTransactionFilter>>(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::Any(field.into().into()),
            );
        }
        ///Sets `any` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_any<T: Into<super::AnyTransactionFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_any(field.into());
            self
        }
        ///Returns the value of `negation`, or the default value if `negation` is unset.
        pub fn negation(&self) -> &super::NotTransactionFilter {
            if let Some(super::transaction_filter::Filter::Negation(field)) = &self
                .filter
            {
                field as _
            } else {
                super::NotTransactionFilter::default_instance() as _
            }
        }
        ///If `negation` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn negation_opt(&self) -> Option<&super::NotTransactionFilter> {
            if let Some(super::transaction_filter::Filter::Negation(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `negation` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn negation_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost::alloc::boxed::Box<super::NotTransactionFilter>> {
            if let Some(super::transaction_filter::Filter::Negation(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `negation`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn negation_mut(
            &mut self,
        ) -> &mut ::prost::alloc::boxed::Box<super::NotTransactionFilter> {
            if self.negation_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::Negation(
                        ::prost::alloc::boxed::Box::default(),
                    ),
                );
            }
            self.negation_opt_mut().unwrap()
        }
        ///Sets `negation` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_negation<
            T: Into<::prost::alloc::boxed::Box<super::NotTransactionFilter>>,
        >(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::Negation(field.into().into()),
            );
        }
        ///Sets `negation` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_negation<
            T: Into<::prost::alloc::boxed::Box<super::NotTransactionFilter>>,
        >(mut self, field: T) -> Self {
            self.set_negation(field.into());
            self
        }
        ///Returns the value of `transaction_kinds`, or the default value if `transaction_kinds` is unset.
        pub fn transaction_kinds(&self) -> &super::TransactionKindsFilter {
            if let Some(super::transaction_filter::Filter::TransactionKinds(field)) = &self
                .filter
            {
                field as _
            } else {
                super::TransactionKindsFilter::default_instance() as _
            }
        }
        ///If `transaction_kinds` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transaction_kinds_opt(&self) -> Option<&super::TransactionKindsFilter> {
            if let Some(super::transaction_filter::Filter::TransactionKinds(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `transaction_kinds` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transaction_kinds_opt_mut(
            &mut self,
        ) -> Option<&mut super::TransactionKindsFilter> {
            if let Some(super::transaction_filter::Filter::TransactionKinds(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `transaction_kinds`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn transaction_kinds_mut(&mut self) -> &mut super::TransactionKindsFilter {
            if self.transaction_kinds_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::TransactionKinds(
                        super::TransactionKindsFilter::default(),
                    ),
                );
            }
            self.transaction_kinds_opt_mut().unwrap()
        }
        ///Sets `transaction_kinds` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_transaction_kinds<T: Into<super::TransactionKindsFilter>>(
            &mut self,
            field: T,
        ) {
            self.filter = Some(
                super::transaction_filter::Filter::TransactionKinds(field.into().into()),
            );
        }
        ///Sets `transaction_kinds` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_transaction_kinds<T: Into<super::TransactionKindsFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_transaction_kinds(field.into());
            self
        }
        ///Returns the value of `sender`, or the default value if `sender` is unset.
        pub fn sender(&self) -> &super::AddressFilter {
            if let Some(super::transaction_filter::Filter::Sender(field)) = &self.filter
            {
                field as _
            } else {
                super::AddressFilter::default_instance() as _
            }
        }
        ///If `sender` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn sender_opt(&self) -> Option<&super::AddressFilter> {
            if let Some(super::transaction_filter::Filter::Sender(field)) = &self.filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `sender` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn sender_opt_mut(&mut self) -> Option<&mut super::AddressFilter> {
            if let Some(super::transaction_filter::Filter::Sender(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `sender`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn sender_mut(&mut self) -> &mut super::AddressFilter {
            if self.sender_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::Sender(
                        super::AddressFilter::default(),
                    ),
                );
            }
            self.sender_opt_mut().unwrap()
        }
        ///Sets `sender` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_sender<T: Into<super::AddressFilter>>(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::Sender(field.into().into()),
            );
        }
        ///Sets `sender` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_sender<T: Into<super::AddressFilter>>(mut self, field: T) -> Self {
            self.set_sender(field.into());
            self
        }
        ///Returns the value of `receiver`, or the default value if `receiver` is unset.
        pub fn receiver(&self) -> &super::AddressFilter {
            if let Some(super::transaction_filter::Filter::Receiver(field)) = &self
                .filter
            {
                field as _
            } else {
                super::AddressFilter::default_instance() as _
            }
        }
        ///If `receiver` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn receiver_opt(&self) -> Option<&super::AddressFilter> {
            if let Some(super::transaction_filter::Filter::Receiver(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `receiver` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn receiver_opt_mut(&mut self) -> Option<&mut super::AddressFilter> {
            if let Some(super::transaction_filter::Filter::Receiver(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `receiver`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn receiver_mut(&mut self) -> &mut super::AddressFilter {
            if self.receiver_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::Receiver(
                        super::AddressFilter::default(),
                    ),
                );
            }
            self.receiver_opt_mut().unwrap()
        }
        ///Sets `receiver` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_receiver<T: Into<super::AddressFilter>>(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::Receiver(field.into().into()),
            );
        }
        ///Sets `receiver` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_receiver<T: Into<super::AddressFilter>>(mut self, field: T) -> Self {
            self.set_receiver(field.into());
            self
        }
        ///Returns the value of `input_object`, or the default value if `input_object` is unset.
        pub fn input_object(&self) -> &super::ObjectIdFilter {
            if let Some(super::transaction_filter::Filter::InputObject(field)) = &self
                .filter
            {
                field as _
            } else {
                super::ObjectIdFilter::default_instance() as _
            }
        }
        ///If `input_object` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn input_object_opt(&self) -> Option<&super::ObjectIdFilter> {
            if let Some(super::transaction_filter::Filter::InputObject(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `input_object` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn input_object_opt_mut(&mut self) -> Option<&mut super::ObjectIdFilter> {
            if let Some(super::transaction_filter::Filter::InputObject(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `input_object`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn input_object_mut(&mut self) -> &mut super::ObjectIdFilter {
            if self.input_object_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::InputObject(
                        super::ObjectIdFilter::default(),
                    ),
                );
            }
            self.input_object_opt_mut().unwrap()
        }
        ///Sets `input_object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_input_object<T: Into<super::ObjectIdFilter>>(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::InputObject(field.into().into()),
            );
        }
        ///Sets `input_object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_input_object<T: Into<super::ObjectIdFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_input_object(field.into());
            self
        }
        ///Returns the value of `changed_object`, or the default value if `changed_object` is unset.
        pub fn changed_object(&self) -> &super::ObjectIdFilter {
            if let Some(super::transaction_filter::Filter::ChangedObject(field)) = &self
                .filter
            {
                field as _
            } else {
                super::ObjectIdFilter::default_instance() as _
            }
        }
        ///If `changed_object` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn changed_object_opt(&self) -> Option<&super::ObjectIdFilter> {
            if let Some(super::transaction_filter::Filter::ChangedObject(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `changed_object` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn changed_object_opt_mut(&mut self) -> Option<&mut super::ObjectIdFilter> {
            if let Some(super::transaction_filter::Filter::ChangedObject(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `changed_object`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn changed_object_mut(&mut self) -> &mut super::ObjectIdFilter {
            if self.changed_object_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::ChangedObject(
                        super::ObjectIdFilter::default(),
                    ),
                );
            }
            self.changed_object_opt_mut().unwrap()
        }
        ///Sets `changed_object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_changed_object<T: Into<super::ObjectIdFilter>>(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::ChangedObject(field.into().into()),
            );
        }
        ///Sets `changed_object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_changed_object<T: Into<super::ObjectIdFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_changed_object(field.into());
            self
        }
        ///Returns the value of `wrapped_or_deleted_object`, or the default value if `wrapped_or_deleted_object` is unset.
        pub fn wrapped_or_deleted_object(&self) -> &super::ObjectIdFilter {
            if let Some(
                super::transaction_filter::Filter::WrappedOrDeletedObject(field),
            ) = &self.filter
            {
                field as _
            } else {
                super::ObjectIdFilter::default_instance() as _
            }
        }
        ///If `wrapped_or_deleted_object` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn wrapped_or_deleted_object_opt(&self) -> Option<&super::ObjectIdFilter> {
            if let Some(
                super::transaction_filter::Filter::WrappedOrDeletedObject(field),
            ) = &self.filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `wrapped_or_deleted_object` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn wrapped_or_deleted_object_opt_mut(
            &mut self,
        ) -> Option<&mut super::ObjectIdFilter> {
            if let Some(
                super::transaction_filter::Filter::WrappedOrDeletedObject(field),
            ) = &mut self.filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `wrapped_or_deleted_object`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn wrapped_or_deleted_object_mut(&mut self) -> &mut super::ObjectIdFilter {
            if self.wrapped_or_deleted_object_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::WrappedOrDeletedObject(
                        super::ObjectIdFilter::default(),
                    ),
                );
            }
            self.wrapped_or_deleted_object_opt_mut().unwrap()
        }
        ///Sets `wrapped_or_deleted_object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_wrapped_or_deleted_object<T: Into<super::ObjectIdFilter>>(
            &mut self,
            field: T,
        ) {
            self.filter = Some(
                super::transaction_filter::Filter::WrappedOrDeletedObject(
                    field.into().into(),
                ),
            );
        }
        ///Sets `wrapped_or_deleted_object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_wrapped_or_deleted_object<T: Into<super::ObjectIdFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_wrapped_or_deleted_object(field.into());
            self
        }
        ///Returns the value of `affected_object`, or the default value if `affected_object` is unset.
        pub fn affected_object(&self) -> &super::ObjectIdFilter {
            if let Some(super::transaction_filter::Filter::AffectedObject(field)) = &self
                .filter
            {
                field as _
            } else {
                super::ObjectIdFilter::default_instance() as _
            }
        }
        ///If `affected_object` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn affected_object_opt(&self) -> Option<&super::ObjectIdFilter> {
            if let Some(super::transaction_filter::Filter::AffectedObject(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `affected_object` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn affected_object_opt_mut(&mut self) -> Option<&mut super::ObjectIdFilter> {
            if let Some(super::transaction_filter::Filter::AffectedObject(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `affected_object`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn affected_object_mut(&mut self) -> &mut super::ObjectIdFilter {
            if self.affected_object_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::AffectedObject(
                        super::ObjectIdFilter::default(),
                    ),
                );
            }
            self.affected_object_opt_mut().unwrap()
        }
        ///Sets `affected_object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_affected_object<T: Into<super::ObjectIdFilter>>(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::AffectedObject(field.into().into()),
            );
        }
        ///Sets `affected_object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_affected_object<T: Into<super::ObjectIdFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_affected_object(field.into());
            self
        }
        ///Returns the value of `move_call`, or the default value if `move_call` is unset.
        pub fn move_call(&self) -> &super::MoveCallFilter {
            if let Some(super::transaction_filter::Filter::MoveCall(field)) = &self
                .filter
            {
                field as _
            } else {
                super::MoveCallFilter::default_instance() as _
            }
        }
        ///If `move_call` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn move_call_opt(&self) -> Option<&super::MoveCallFilter> {
            if let Some(super::transaction_filter::Filter::MoveCall(field)) = &self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `move_call` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn move_call_opt_mut(&mut self) -> Option<&mut super::MoveCallFilter> {
            if let Some(super::transaction_filter::Filter::MoveCall(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `move_call`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn move_call_mut(&mut self) -> &mut super::MoveCallFilter {
            if self.move_call_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::MoveCall(
                        super::MoveCallFilter::default(),
                    ),
                );
            }
            self.move_call_opt_mut().unwrap()
        }
        ///Sets `move_call` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_move_call<T: Into<super::MoveCallFilter>>(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::MoveCall(field.into().into()),
            );
        }
        ///Sets `move_call` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_move_call<T: Into<super::MoveCallFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_move_call(field.into());
            self
        }
        ///Returns the value of `event`, or the default value if `event` is unset.
        pub fn event(&self) -> &super::EventFilter {
            if let Some(super::transaction_filter::Filter::Event(field)) = &self.filter {
                field as _
            } else {
                super::EventFilter::default_instance() as _
            }
        }
        ///If `event` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn event_opt(&self) -> Option<&super::EventFilter> {
            if let Some(super::transaction_filter::Filter::Event(field)) = &self.filter {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `event` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn event_opt_mut(&mut self) -> Option<&mut super::EventFilter> {
            if let Some(super::transaction_filter::Filter::Event(field)) = &mut self
                .filter
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `event`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn event_mut(&mut self) -> &mut super::EventFilter {
            if self.event_opt_mut().is_none() {
                self.filter = Some(
                    super::transaction_filter::Filter::Event(
                        super::EventFilter::default(),
                    ),
                );
            }
            self.event_opt_mut().unwrap()
        }
        ///Sets `event` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_event<T: Into<super::EventFilter>>(&mut self, field: T) {
            self.filter = Some(
                super::transaction_filter::Filter::Event(field.into().into()),
            );
        }
        ///Sets `event` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_event<T: Into<super::EventFilter>>(mut self, field: T) -> Self {
            self.set_event(field.into());
            self
        }
    }
    impl super::TransactionKindsFilter {
        pub const fn const_default() -> Self {
            Self { kinds: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TransactionKindsFilter = super::TransactionKindsFilter::const_default();
            &DEFAULT
        }
    }
}
