// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::DynamicField {
        /// Sets `kind` with the provided value.
        pub fn with_kind(
            mut self,
            field: super::dynamic_field::DynamicFieldKind,
        ) -> Self {
            self.kind = Some(field.into());
            self
        }
        /// Sets `parent` with the provided value.
        pub fn with_parent<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.parent = Some(field.into());
            self
        }
        /// Sets `field_id` with the provided value.
        pub fn with_field_id<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.field_id = Some(field.into());
            self
        }
        /// Sets `field_object` with the provided value.
        pub fn with_field_object<T: Into<super::super::object::Object>>(
            mut self,
            field: T,
        ) -> Self {
            self.field_object = Some(field.into());
            self
        }
        /// Sets `name` with the provided value.
        pub fn with_name<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.name = Some(field.into());
            self
        }
        /// Sets `value` with the provided value.
        pub fn with_value<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.value = Some(field.into());
            self
        }
        /// Sets `value_type` with the provided value.
        pub fn with_value_type<T: Into<String>>(mut self, field: T) -> Self {
            self.value_type = Some(field.into());
            self
        }
        /// Sets `child_id` with the provided value.
        pub fn with_child_id<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.child_id = Some(field.into());
            self
        }
        /// Sets `child_object` with the provided value.
        pub fn with_child_object<T: Into<super::super::object::Object>>(
            mut self,
            field: T,
        ) -> Self {
            self.child_object = Some(field.into());
            self
        }
    }
}
