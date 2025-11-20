// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::BadRequest {
        pub const fn const_default() -> Self {
            Self {
                field_violations: Vec::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::BadRequest = super::BadRequest::const_default();
            &DEFAULT
        }
        ///Returns the value of `field_violations`, or the default value if `field_violations` is unset.
        pub fn field_violations(&self) -> &[super::bad_request::FieldViolation] {
            &self.field_violations
        }
        ///Returns a mutable reference to `field_violations`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn field_violations_mut(
            &mut self,
        ) -> &mut Vec<super::bad_request::FieldViolation> {
            &mut self.field_violations
        }
        ///Sets `field_violations` with the provided value.
        pub fn set_field_violations(
            &mut self,
            field: Vec<super::bad_request::FieldViolation>,
        ) {
            self.field_violations = field;
        }
        ///Sets `field_violations` with the provided value.
        pub fn with_field_violations(
            mut self,
            field: Vec<super::bad_request::FieldViolation>,
        ) -> Self {
            self.set_field_violations(field);
            self
        }
    }
    impl super::bad_request::FieldViolation {
        pub const fn const_default() -> Self {
            Self {
                field: String::new(),
                description: String::new(),
                reason: String::new(),
                localized_message: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::bad_request::FieldViolation = super::bad_request::FieldViolation::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `field`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn field_mut(&mut self) -> &mut String {
            &mut self.field
        }
        ///Sets `field` with the provided value.
        pub fn set_field<T: Into<String>>(&mut self, field: T) {
            self.field = field.into().into();
        }
        ///Sets `field` with the provided value.
        pub fn with_field<T: Into<String>>(mut self, field: T) -> Self {
            self.set_field(field.into());
            self
        }
        ///Returns a mutable reference to `description`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn description_mut(&mut self) -> &mut String {
            &mut self.description
        }
        ///Sets `description` with the provided value.
        pub fn set_description<T: Into<String>>(&mut self, field: T) {
            self.description = field.into().into();
        }
        ///Sets `description` with the provided value.
        pub fn with_description<T: Into<String>>(mut self, field: T) -> Self {
            self.set_description(field.into());
            self
        }
        ///Returns a mutable reference to `reason`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn reason_mut(&mut self) -> &mut String {
            &mut self.reason
        }
        ///Sets `reason` with the provided value.
        pub fn set_reason<T: Into<String>>(&mut self, field: T) {
            self.reason = field.into().into();
        }
        ///Sets `reason` with the provided value.
        pub fn with_reason<T: Into<String>>(mut self, field: T) -> Self {
            self.set_reason(field.into());
            self
        }
        ///Returns the value of `localized_message`, or the default value if `localized_message` is unset.
        pub fn localized_message(&self) -> &super::LocalizedMessage {
            self.localized_message
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::LocalizedMessage::default_instance() as _)
        }
        ///If `localized_message` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn localized_message_opt_mut(
            &mut self,
        ) -> Option<&mut super::LocalizedMessage> {
            self.localized_message.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `localized_message`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn localized_message_mut(&mut self) -> &mut super::LocalizedMessage {
            self.localized_message.get_or_insert_default()
        }
        ///If `localized_message` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn localized_message_opt(&self) -> Option<&super::LocalizedMessage> {
            self.localized_message.as_ref().map(|field| field as _)
        }
        ///Sets `localized_message` with the provided value.
        pub fn set_localized_message<T: Into<super::LocalizedMessage>>(
            &mut self,
            field: T,
        ) {
            self.localized_message = Some(field.into().into());
        }
        ///Sets `localized_message` with the provided value.
        pub fn with_localized_message<T: Into<super::LocalizedMessage>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_localized_message(field.into());
            self
        }
    }
    impl super::DebugInfo {
        pub const fn const_default() -> Self {
            Self {
                stack_entries: Vec::new(),
                detail: String::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::DebugInfo = super::DebugInfo::const_default();
            &DEFAULT
        }
        ///Returns the value of `stack_entries`, or the default value if `stack_entries` is unset.
        pub fn stack_entries(&self) -> &[String] {
            &self.stack_entries
        }
        ///Returns a mutable reference to `stack_entries`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn stack_entries_mut(&mut self) -> &mut Vec<String> {
            &mut self.stack_entries
        }
        ///Sets `stack_entries` with the provided value.
        pub fn set_stack_entries(&mut self, field: Vec<String>) {
            self.stack_entries = field;
        }
        ///Sets `stack_entries` with the provided value.
        pub fn with_stack_entries(mut self, field: Vec<String>) -> Self {
            self.set_stack_entries(field);
            self
        }
        ///Returns a mutable reference to `detail`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn detail_mut(&mut self) -> &mut String {
            &mut self.detail
        }
        ///Sets `detail` with the provided value.
        pub fn set_detail<T: Into<String>>(&mut self, field: T) {
            self.detail = field.into().into();
        }
        ///Sets `detail` with the provided value.
        pub fn with_detail<T: Into<String>>(mut self, field: T) -> Self {
            self.set_detail(field.into());
            self
        }
    }
    impl super::ErrorInfo {
        pub const fn const_default() -> Self {
            Self {
                reason: String::new(),
                domain: String::new(),
                metadata: std::collections::BTreeMap::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ErrorInfo = super::ErrorInfo::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `reason`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn reason_mut(&mut self) -> &mut String {
            &mut self.reason
        }
        ///Sets `reason` with the provided value.
        pub fn set_reason<T: Into<String>>(&mut self, field: T) {
            self.reason = field.into().into();
        }
        ///Sets `reason` with the provided value.
        pub fn with_reason<T: Into<String>>(mut self, field: T) -> Self {
            self.set_reason(field.into());
            self
        }
        ///Returns a mutable reference to `domain`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn domain_mut(&mut self) -> &mut String {
            &mut self.domain
        }
        ///Sets `domain` with the provided value.
        pub fn set_domain<T: Into<String>>(&mut self, field: T) {
            self.domain = field.into().into();
        }
        ///Sets `domain` with the provided value.
        pub fn with_domain<T: Into<String>>(mut self, field: T) -> Self {
            self.set_domain(field.into());
            self
        }
        ///Returns the value of `metadata`, or the default value if `metadata` is unset.
        pub fn metadata(&self) -> &::std::collections::BTreeMap<String, String> {
            &self.metadata
        }
        ///Returns a mutable reference to `metadata`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn metadata_mut(
            &mut self,
        ) -> &mut ::std::collections::BTreeMap<String, String> {
            &mut self.metadata
        }
        ///Sets `metadata` with the provided value.
        pub fn set_metadata(
            &mut self,
            field: ::std::collections::BTreeMap<String, String>,
        ) {
            self.metadata = field;
        }
        ///Sets `metadata` with the provided value.
        pub fn with_metadata(
            mut self,
            field: ::std::collections::BTreeMap<String, String>,
        ) -> Self {
            self.set_metadata(field);
            self
        }
    }
    impl super::Help {
        pub const fn const_default() -> Self {
            Self { links: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Help = super::Help::const_default();
            &DEFAULT
        }
        ///Returns the value of `links`, or the default value if `links` is unset.
        pub fn links(&self) -> &[super::help::Link] {
            &self.links
        }
        ///Returns a mutable reference to `links`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn links_mut(&mut self) -> &mut Vec<super::help::Link> {
            &mut self.links
        }
        ///Sets `links` with the provided value.
        pub fn set_links(&mut self, field: Vec<super::help::Link>) {
            self.links = field;
        }
        ///Sets `links` with the provided value.
        pub fn with_links(mut self, field: Vec<super::help::Link>) -> Self {
            self.set_links(field);
            self
        }
    }
    impl super::help::Link {
        pub const fn const_default() -> Self {
            Self {
                description: String::new(),
                url: String::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::help::Link = super::help::Link::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `description`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn description_mut(&mut self) -> &mut String {
            &mut self.description
        }
        ///Sets `description` with the provided value.
        pub fn set_description<T: Into<String>>(&mut self, field: T) {
            self.description = field.into().into();
        }
        ///Sets `description` with the provided value.
        pub fn with_description<T: Into<String>>(mut self, field: T) -> Self {
            self.set_description(field.into());
            self
        }
        ///Returns a mutable reference to `url`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn url_mut(&mut self) -> &mut String {
            &mut self.url
        }
        ///Sets `url` with the provided value.
        pub fn set_url<T: Into<String>>(&mut self, field: T) {
            self.url = field.into().into();
        }
        ///Sets `url` with the provided value.
        pub fn with_url<T: Into<String>>(mut self, field: T) -> Self {
            self.set_url(field.into());
            self
        }
    }
    impl super::LocalizedMessage {
        pub const fn const_default() -> Self {
            Self {
                locale: String::new(),
                message: String::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::LocalizedMessage = super::LocalizedMessage::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `locale`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn locale_mut(&mut self) -> &mut String {
            &mut self.locale
        }
        ///Sets `locale` with the provided value.
        pub fn set_locale<T: Into<String>>(&mut self, field: T) {
            self.locale = field.into().into();
        }
        ///Sets `locale` with the provided value.
        pub fn with_locale<T: Into<String>>(mut self, field: T) -> Self {
            self.set_locale(field.into());
            self
        }
        ///Returns a mutable reference to `message`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn message_mut(&mut self) -> &mut String {
            &mut self.message
        }
        ///Sets `message` with the provided value.
        pub fn set_message<T: Into<String>>(&mut self, field: T) {
            self.message = field.into().into();
        }
        ///Sets `message` with the provided value.
        pub fn with_message<T: Into<String>>(mut self, field: T) -> Self {
            self.set_message(field.into());
            self
        }
    }
    impl super::PreconditionFailure {
        pub const fn const_default() -> Self {
            Self { violations: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::PreconditionFailure = super::PreconditionFailure::const_default();
            &DEFAULT
        }
        ///Returns the value of `violations`, or the default value if `violations` is unset.
        pub fn violations(&self) -> &[super::precondition_failure::Violation] {
            &self.violations
        }
        ///Returns a mutable reference to `violations`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn violations_mut(
            &mut self,
        ) -> &mut Vec<super::precondition_failure::Violation> {
            &mut self.violations
        }
        ///Sets `violations` with the provided value.
        pub fn set_violations(
            &mut self,
            field: Vec<super::precondition_failure::Violation>,
        ) {
            self.violations = field;
        }
        ///Sets `violations` with the provided value.
        pub fn with_violations(
            mut self,
            field: Vec<super::precondition_failure::Violation>,
        ) -> Self {
            self.set_violations(field);
            self
        }
    }
    impl super::precondition_failure::Violation {
        pub const fn const_default() -> Self {
            Self {
                r#type: String::new(),
                subject: String::new(),
                description: String::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::precondition_failure::Violation = super::precondition_failure::Violation::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `r#type`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn type_mut(&mut self) -> &mut String {
            &mut self.r#type
        }
        ///Sets `r#type` with the provided value.
        pub fn set_type<T: Into<String>>(&mut self, field: T) {
            self.r#type = field.into().into();
        }
        ///Sets `r#type` with the provided value.
        pub fn with_type<T: Into<String>>(mut self, field: T) -> Self {
            self.set_type(field.into());
            self
        }
        ///Returns a mutable reference to `subject`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn subject_mut(&mut self) -> &mut String {
            &mut self.subject
        }
        ///Sets `subject` with the provided value.
        pub fn set_subject<T: Into<String>>(&mut self, field: T) {
            self.subject = field.into().into();
        }
        ///Sets `subject` with the provided value.
        pub fn with_subject<T: Into<String>>(mut self, field: T) -> Self {
            self.set_subject(field.into());
            self
        }
        ///Returns a mutable reference to `description`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn description_mut(&mut self) -> &mut String {
            &mut self.description
        }
        ///Sets `description` with the provided value.
        pub fn set_description<T: Into<String>>(&mut self, field: T) {
            self.description = field.into().into();
        }
        ///Sets `description` with the provided value.
        pub fn with_description<T: Into<String>>(mut self, field: T) -> Self {
            self.set_description(field.into());
            self
        }
    }
    impl super::QuotaFailure {
        pub const fn const_default() -> Self {
            Self { violations: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::QuotaFailure = super::QuotaFailure::const_default();
            &DEFAULT
        }
        ///Returns the value of `violations`, or the default value if `violations` is unset.
        pub fn violations(&self) -> &[super::quota_failure::Violation] {
            &self.violations
        }
        ///Returns a mutable reference to `violations`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn violations_mut(&mut self) -> &mut Vec<super::quota_failure::Violation> {
            &mut self.violations
        }
        ///Sets `violations` with the provided value.
        pub fn set_violations(&mut self, field: Vec<super::quota_failure::Violation>) {
            self.violations = field;
        }
        ///Sets `violations` with the provided value.
        pub fn with_violations(
            mut self,
            field: Vec<super::quota_failure::Violation>,
        ) -> Self {
            self.set_violations(field);
            self
        }
    }
    impl super::quota_failure::Violation {
        pub const fn const_default() -> Self {
            Self {
                subject: String::new(),
                description: String::new(),
                api_service: String::new(),
                quota_metric: String::new(),
                quota_id: String::new(),
                quota_dimensions: std::collections::BTreeMap::new(),
                quota_value: 0,
                future_quota_value: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::quota_failure::Violation = super::quota_failure::Violation::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `subject`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn subject_mut(&mut self) -> &mut String {
            &mut self.subject
        }
        ///Sets `subject` with the provided value.
        pub fn set_subject<T: Into<String>>(&mut self, field: T) {
            self.subject = field.into().into();
        }
        ///Sets `subject` with the provided value.
        pub fn with_subject<T: Into<String>>(mut self, field: T) -> Self {
            self.set_subject(field.into());
            self
        }
        ///Returns a mutable reference to `description`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn description_mut(&mut self) -> &mut String {
            &mut self.description
        }
        ///Sets `description` with the provided value.
        pub fn set_description<T: Into<String>>(&mut self, field: T) {
            self.description = field.into().into();
        }
        ///Sets `description` with the provided value.
        pub fn with_description<T: Into<String>>(mut self, field: T) -> Self {
            self.set_description(field.into());
            self
        }
        ///Returns a mutable reference to `api_service`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn api_service_mut(&mut self) -> &mut String {
            &mut self.api_service
        }
        ///Sets `api_service` with the provided value.
        pub fn set_api_service<T: Into<String>>(&mut self, field: T) {
            self.api_service = field.into().into();
        }
        ///Sets `api_service` with the provided value.
        pub fn with_api_service<T: Into<String>>(mut self, field: T) -> Self {
            self.set_api_service(field.into());
            self
        }
        ///Returns a mutable reference to `quota_metric`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn quota_metric_mut(&mut self) -> &mut String {
            &mut self.quota_metric
        }
        ///Sets `quota_metric` with the provided value.
        pub fn set_quota_metric<T: Into<String>>(&mut self, field: T) {
            self.quota_metric = field.into().into();
        }
        ///Sets `quota_metric` with the provided value.
        pub fn with_quota_metric<T: Into<String>>(mut self, field: T) -> Self {
            self.set_quota_metric(field.into());
            self
        }
        ///Returns a mutable reference to `quota_id`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn quota_id_mut(&mut self) -> &mut String {
            &mut self.quota_id
        }
        ///Sets `quota_id` with the provided value.
        pub fn set_quota_id<T: Into<String>>(&mut self, field: T) {
            self.quota_id = field.into().into();
        }
        ///Sets `quota_id` with the provided value.
        pub fn with_quota_id<T: Into<String>>(mut self, field: T) -> Self {
            self.set_quota_id(field.into());
            self
        }
        ///Returns the value of `quota_dimensions`, or the default value if `quota_dimensions` is unset.
        pub fn quota_dimensions(&self) -> &::std::collections::BTreeMap<String, String> {
            &self.quota_dimensions
        }
        ///Returns a mutable reference to `quota_dimensions`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn quota_dimensions_mut(
            &mut self,
        ) -> &mut ::std::collections::BTreeMap<String, String> {
            &mut self.quota_dimensions
        }
        ///Sets `quota_dimensions` with the provided value.
        pub fn set_quota_dimensions(
            &mut self,
            field: ::std::collections::BTreeMap<String, String>,
        ) {
            self.quota_dimensions = field;
        }
        ///Sets `quota_dimensions` with the provided value.
        pub fn with_quota_dimensions(
            mut self,
            field: ::std::collections::BTreeMap<String, String>,
        ) -> Self {
            self.set_quota_dimensions(field);
            self
        }
        ///Returns a mutable reference to `quota_value`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn quota_value_mut(&mut self) -> &mut i64 {
            &mut self.quota_value
        }
        ///Sets `quota_value` with the provided value.
        pub fn set_quota_value(&mut self, field: i64) {
            self.quota_value = field;
        }
        ///Sets `quota_value` with the provided value.
        pub fn with_quota_value(mut self, field: i64) -> Self {
            self.set_quota_value(field);
            self
        }
        ///If `future_quota_value` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn future_quota_value_opt_mut(&mut self) -> Option<&mut i64> {
            self.future_quota_value.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `future_quota_value`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn future_quota_value_mut(&mut self) -> &mut i64 {
            self.future_quota_value.get_or_insert_default()
        }
        ///If `future_quota_value` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn future_quota_value_opt(&self) -> Option<i64> {
            self.future_quota_value.as_ref().map(|field| *field)
        }
        ///Sets `future_quota_value` with the provided value.
        pub fn set_future_quota_value(&mut self, field: i64) {
            self.future_quota_value = Some(field);
        }
        ///Sets `future_quota_value` with the provided value.
        pub fn with_future_quota_value(mut self, field: i64) -> Self {
            self.set_future_quota_value(field);
            self
        }
    }
    impl super::RequestInfo {
        pub const fn const_default() -> Self {
            Self {
                request_id: String::new(),
                serving_data: String::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::RequestInfo = super::RequestInfo::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `request_id`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn request_id_mut(&mut self) -> &mut String {
            &mut self.request_id
        }
        ///Sets `request_id` with the provided value.
        pub fn set_request_id<T: Into<String>>(&mut self, field: T) {
            self.request_id = field.into().into();
        }
        ///Sets `request_id` with the provided value.
        pub fn with_request_id<T: Into<String>>(mut self, field: T) -> Self {
            self.set_request_id(field.into());
            self
        }
        ///Returns a mutable reference to `serving_data`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn serving_data_mut(&mut self) -> &mut String {
            &mut self.serving_data
        }
        ///Sets `serving_data` with the provided value.
        pub fn set_serving_data<T: Into<String>>(&mut self, field: T) {
            self.serving_data = field.into().into();
        }
        ///Sets `serving_data` with the provided value.
        pub fn with_serving_data<T: Into<String>>(mut self, field: T) -> Self {
            self.set_serving_data(field.into());
            self
        }
    }
    impl super::ResourceInfo {
        pub const fn const_default() -> Self {
            Self {
                resource_type: String::new(),
                resource_name: String::new(),
                owner: String::new(),
                description: String::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ResourceInfo = super::ResourceInfo::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `resource_type`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn resource_type_mut(&mut self) -> &mut String {
            &mut self.resource_type
        }
        ///Sets `resource_type` with the provided value.
        pub fn set_resource_type<T: Into<String>>(&mut self, field: T) {
            self.resource_type = field.into().into();
        }
        ///Sets `resource_type` with the provided value.
        pub fn with_resource_type<T: Into<String>>(mut self, field: T) -> Self {
            self.set_resource_type(field.into());
            self
        }
        ///Returns a mutable reference to `resource_name`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn resource_name_mut(&mut self) -> &mut String {
            &mut self.resource_name
        }
        ///Sets `resource_name` with the provided value.
        pub fn set_resource_name<T: Into<String>>(&mut self, field: T) {
            self.resource_name = field.into().into();
        }
        ///Sets `resource_name` with the provided value.
        pub fn with_resource_name<T: Into<String>>(mut self, field: T) -> Self {
            self.set_resource_name(field.into());
            self
        }
        ///Returns a mutable reference to `owner`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn owner_mut(&mut self) -> &mut String {
            &mut self.owner
        }
        ///Sets `owner` with the provided value.
        pub fn set_owner<T: Into<String>>(&mut self, field: T) {
            self.owner = field.into().into();
        }
        ///Sets `owner` with the provided value.
        pub fn with_owner<T: Into<String>>(mut self, field: T) -> Self {
            self.set_owner(field.into());
            self
        }
        ///Returns a mutable reference to `description`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn description_mut(&mut self) -> &mut String {
            &mut self.description
        }
        ///Sets `description` with the provided value.
        pub fn set_description<T: Into<String>>(&mut self, field: T) {
            self.description = field.into().into();
        }
        ///Sets `description` with the provided value.
        pub fn with_description<T: Into<String>>(mut self, field: T) -> Self {
            self.set_description(field.into());
            self
        }
    }
    impl super::RetryInfo {
        pub const fn const_default() -> Self {
            Self { retry_delay: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::RetryInfo = super::RetryInfo::const_default();
            &DEFAULT
        }
        ///If `retry_delay` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn retry_delay_opt_mut(&mut self) -> Option<&mut ::prost_types::Duration> {
            self.retry_delay.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `retry_delay`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn retry_delay_mut(&mut self) -> &mut ::prost_types::Duration {
            self.retry_delay.get_or_insert_default()
        }
        ///If `retry_delay` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn retry_delay_opt(&self) -> Option<&::prost_types::Duration> {
            self.retry_delay.as_ref().map(|field| field as _)
        }
        ///Sets `retry_delay` with the provided value.
        pub fn set_retry_delay<T: Into<::prost_types::Duration>>(&mut self, field: T) {
            self.retry_delay = Some(field.into().into());
        }
        ///Sets `retry_delay` with the provided value.
        pub fn with_retry_delay<T: Into<::prost_types::Duration>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_retry_delay(field.into());
            self
        }
    }
    impl super::Status {
        pub const fn const_default() -> Self {
            Self {
                code: 0,
                message: String::new(),
                details: Vec::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Status = super::Status::const_default();
            &DEFAULT
        }
        ///Returns a mutable reference to `code`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn code_mut(&mut self) -> &mut i32 {
            &mut self.code
        }
        ///Sets `code` with the provided value.
        pub fn set_code(&mut self, field: i32) {
            self.code = field;
        }
        ///Sets `code` with the provided value.
        pub fn with_code(mut self, field: i32) -> Self {
            self.set_code(field);
            self
        }
        ///Returns a mutable reference to `message`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn message_mut(&mut self) -> &mut String {
            &mut self.message
        }
        ///Sets `message` with the provided value.
        pub fn set_message<T: Into<String>>(&mut self, field: T) {
            self.message = field.into().into();
        }
        ///Sets `message` with the provided value.
        pub fn with_message<T: Into<String>>(mut self, field: T) -> Self {
            self.set_message(field.into());
            self
        }
        ///Returns the value of `details`, or the default value if `details` is unset.
        pub fn details(&self) -> &[::prost_types::Any] {
            &self.details
        }
        ///Returns a mutable reference to `details`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn details_mut(&mut self) -> &mut Vec<::prost_types::Any> {
            &mut self.details
        }
        ///Sets `details` with the provided value.
        pub fn set_details(&mut self, field: Vec<::prost_types::Any>) {
            self.details = field;
        }
        ///Sets `details` with the provided value.
        pub fn with_details(mut self, field: Vec<::prost_types::Any>) -> Self {
            self.set_details(field);
            self
        }
    }
}
