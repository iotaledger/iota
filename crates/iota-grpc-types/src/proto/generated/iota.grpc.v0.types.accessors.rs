// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Address {
        pub const fn const_default() -> Self {
            Self {
                address: ::prost::bytes::Bytes::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Address = super::Address::const_default();
            &DEFAULT
        }
        ///Sets `address` with the provided value.
        pub fn set_address<T: Into<::prost::bytes::Bytes>>(&mut self, field: T) {
            self.address = field.into().into();
        }
        ///Sets `address` with the provided value.
        pub fn with_address<T: Into<::prost::bytes::Bytes>>(mut self, field: T) -> Self {
            self.set_address(field.into());
            self
        }
    }
    impl super::Digest {
        pub const fn const_default() -> Self {
            Self {
                digest: ::prost::bytes::Bytes::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Digest = super::Digest::const_default();
            &DEFAULT
        }
        ///Sets `digest` with the provided value.
        pub fn set_digest<T: Into<::prost::bytes::Bytes>>(&mut self, field: T) {
            self.digest = field.into().into();
        }
        ///Sets `digest` with the provided value.
        pub fn with_digest<T: Into<::prost::bytes::Bytes>>(mut self, field: T) -> Self {
            self.set_digest(field.into());
            self
        }
    }
    impl super::ObjectReference {
        pub const fn const_default() -> Self {
            Self {
                object_id: None,
                version: None,
                digest: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ObjectReference = super::ObjectReference::const_default();
            &DEFAULT
        }
        ///If `object_id` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn object_id_opt_mut(&mut self) -> Option<&mut String> {
            self.object_id.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `object_id`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn object_id_mut(&mut self) -> &mut String {
            self.object_id.get_or_insert_default()
        }
        ///If `object_id` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn object_id_opt(&self) -> Option<&str> {
            self.object_id.as_ref().map(|field| field as _)
        }
        ///Sets `object_id` with the provided value.
        pub fn set_object_id<T: Into<String>>(&mut self, field: T) {
            self.object_id = Some(field.into().into());
        }
        ///Sets `object_id` with the provided value.
        pub fn with_object_id<T: Into<String>>(mut self, field: T) -> Self {
            self.set_object_id(field.into());
            self
        }
        ///If `version` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn version_opt_mut(&mut self) -> Option<&mut u64> {
            self.version.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `version`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn version_mut(&mut self) -> &mut u64 {
            self.version.get_or_insert_default()
        }
        ///If `version` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn version_opt(&self) -> Option<u64> {
            self.version.as_ref().map(|field| *field)
        }
        ///Sets `version` with the provided value.
        pub fn set_version(&mut self, field: u64) {
            self.version = Some(field);
        }
        ///Sets `version` with the provided value.
        pub fn with_version(mut self, field: u64) -> Self {
            self.set_version(field);
            self
        }
        ///Returns the value of `digest`, or the default value if `digest` is unset.
        pub fn digest(&self) -> &super::Digest {
            self.digest
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::Digest::default_instance() as _)
        }
        ///If `digest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn digest_opt_mut(&mut self) -> Option<&mut super::Digest> {
            self.digest.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `digest`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn digest_mut(&mut self) -> &mut super::Digest {
            self.digest.get_or_insert_default()
        }
        ///If `digest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn digest_opt(&self) -> Option<&super::Digest> {
            self.digest.as_ref().map(|field| field as _)
        }
        ///Sets `digest` with the provided value.
        pub fn set_digest<T: Into<super::Digest>>(&mut self, field: T) {
            self.digest = Some(field.into().into());
        }
        ///Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::Digest>>(mut self, field: T) -> Self {
            self.set_digest(field.into());
            self
        }
    }
    impl super::TypeTag {
        pub const fn const_default() -> Self {
            Self { type_tag: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TypeTag = super::TypeTag::const_default();
            &DEFAULT
        }
        ///Returns the value of `bool_tag`, or the default value if `bool_tag` is unset.
        pub fn bool_tag(&self) -> bool {
            if let Some(super::type_tag::TypeTag::BoolTag(field)) = &self.type_tag {
                *field
            } else {
                false
            }
        }
        ///If `bool_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn bool_tag_opt(&self) -> Option<bool> {
            if let Some(super::type_tag::TypeTag::BoolTag(field)) = &self.type_tag {
                Some(*field)
            } else {
                None
            }
        }
        ///If `bool_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn bool_tag_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(super::type_tag::TypeTag::BoolTag(field)) = &mut self.type_tag {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `bool_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn bool_tag_mut(&mut self) -> &mut bool {
            if self.bool_tag_opt_mut().is_none() {
                self.type_tag = Some(super::type_tag::TypeTag::BoolTag(bool::default()));
            }
            self.bool_tag_opt_mut().unwrap()
        }
        ///Sets `bool_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_bool_tag(&mut self, field: bool) {
            self.type_tag = Some(super::type_tag::TypeTag::BoolTag(field));
        }
        ///Sets `bool_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_bool_tag(mut self, field: bool) -> Self {
            self.set_bool_tag(field);
            self
        }
        ///Returns the value of `u8_tag`, or the default value if `u8_tag` is unset.
        pub fn u8_tag(&self) -> bool {
            if let Some(super::type_tag::TypeTag::U8Tag(field)) = &self.type_tag {
                *field
            } else {
                false
            }
        }
        ///If `u8_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn u8_tag_opt(&self) -> Option<bool> {
            if let Some(super::type_tag::TypeTag::U8Tag(field)) = &self.type_tag {
                Some(*field)
            } else {
                None
            }
        }
        ///If `u8_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn u8_tag_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(super::type_tag::TypeTag::U8Tag(field)) = &mut self.type_tag {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `u8_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn u8_tag_mut(&mut self) -> &mut bool {
            if self.u8_tag_opt_mut().is_none() {
                self.type_tag = Some(super::type_tag::TypeTag::U8Tag(bool::default()));
            }
            self.u8_tag_opt_mut().unwrap()
        }
        ///Sets `u8_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_u8_tag(&mut self, field: bool) {
            self.type_tag = Some(super::type_tag::TypeTag::U8Tag(field));
        }
        ///Sets `u8_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_u8_tag(mut self, field: bool) -> Self {
            self.set_u8_tag(field);
            self
        }
        ///Returns the value of `u16_tag`, or the default value if `u16_tag` is unset.
        pub fn u16_tag(&self) -> bool {
            if let Some(super::type_tag::TypeTag::U16Tag(field)) = &self.type_tag {
                *field
            } else {
                false
            }
        }
        ///If `u16_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn u16_tag_opt(&self) -> Option<bool> {
            if let Some(super::type_tag::TypeTag::U16Tag(field)) = &self.type_tag {
                Some(*field)
            } else {
                None
            }
        }
        ///If `u16_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn u16_tag_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(super::type_tag::TypeTag::U16Tag(field)) = &mut self.type_tag {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `u16_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn u16_tag_mut(&mut self) -> &mut bool {
            if self.u16_tag_opt_mut().is_none() {
                self.type_tag = Some(super::type_tag::TypeTag::U16Tag(bool::default()));
            }
            self.u16_tag_opt_mut().unwrap()
        }
        ///Sets `u16_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_u16_tag(&mut self, field: bool) {
            self.type_tag = Some(super::type_tag::TypeTag::U16Tag(field));
        }
        ///Sets `u16_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_u16_tag(mut self, field: bool) -> Self {
            self.set_u16_tag(field);
            self
        }
        ///Returns the value of `u32_tag`, or the default value if `u32_tag` is unset.
        pub fn u32_tag(&self) -> bool {
            if let Some(super::type_tag::TypeTag::U32Tag(field)) = &self.type_tag {
                *field
            } else {
                false
            }
        }
        ///If `u32_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn u32_tag_opt(&self) -> Option<bool> {
            if let Some(super::type_tag::TypeTag::U32Tag(field)) = &self.type_tag {
                Some(*field)
            } else {
                None
            }
        }
        ///If `u32_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn u32_tag_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(super::type_tag::TypeTag::U32Tag(field)) = &mut self.type_tag {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `u32_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn u32_tag_mut(&mut self) -> &mut bool {
            if self.u32_tag_opt_mut().is_none() {
                self.type_tag = Some(super::type_tag::TypeTag::U32Tag(bool::default()));
            }
            self.u32_tag_opt_mut().unwrap()
        }
        ///Sets `u32_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_u32_tag(&mut self, field: bool) {
            self.type_tag = Some(super::type_tag::TypeTag::U32Tag(field));
        }
        ///Sets `u32_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_u32_tag(mut self, field: bool) -> Self {
            self.set_u32_tag(field);
            self
        }
        ///Returns the value of `u64_tag`, or the default value if `u64_tag` is unset.
        pub fn u64_tag(&self) -> bool {
            if let Some(super::type_tag::TypeTag::U64Tag(field)) = &self.type_tag {
                *field
            } else {
                false
            }
        }
        ///If `u64_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn u64_tag_opt(&self) -> Option<bool> {
            if let Some(super::type_tag::TypeTag::U64Tag(field)) = &self.type_tag {
                Some(*field)
            } else {
                None
            }
        }
        ///If `u64_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn u64_tag_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(super::type_tag::TypeTag::U64Tag(field)) = &mut self.type_tag {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `u64_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn u64_tag_mut(&mut self) -> &mut bool {
            if self.u64_tag_opt_mut().is_none() {
                self.type_tag = Some(super::type_tag::TypeTag::U64Tag(bool::default()));
            }
            self.u64_tag_opt_mut().unwrap()
        }
        ///Sets `u64_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_u64_tag(&mut self, field: bool) {
            self.type_tag = Some(super::type_tag::TypeTag::U64Tag(field));
        }
        ///Sets `u64_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_u64_tag(mut self, field: bool) -> Self {
            self.set_u64_tag(field);
            self
        }
        ///Returns the value of `u128_tag`, or the default value if `u128_tag` is unset.
        pub fn u128_tag(&self) -> bool {
            if let Some(super::type_tag::TypeTag::U128Tag(field)) = &self.type_tag {
                *field
            } else {
                false
            }
        }
        ///If `u128_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn u128_tag_opt(&self) -> Option<bool> {
            if let Some(super::type_tag::TypeTag::U128Tag(field)) = &self.type_tag {
                Some(*field)
            } else {
                None
            }
        }
        ///If `u128_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn u128_tag_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(super::type_tag::TypeTag::U128Tag(field)) = &mut self.type_tag {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `u128_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn u128_tag_mut(&mut self) -> &mut bool {
            if self.u128_tag_opt_mut().is_none() {
                self.type_tag = Some(super::type_tag::TypeTag::U128Tag(bool::default()));
            }
            self.u128_tag_opt_mut().unwrap()
        }
        ///Sets `u128_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_u128_tag(&mut self, field: bool) {
            self.type_tag = Some(super::type_tag::TypeTag::U128Tag(field));
        }
        ///Sets `u128_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_u128_tag(mut self, field: bool) -> Self {
            self.set_u128_tag(field);
            self
        }
        ///Returns the value of `u256_tag`, or the default value if `u256_tag` is unset.
        pub fn u256_tag(&self) -> bool {
            if let Some(super::type_tag::TypeTag::U256Tag(field)) = &self.type_tag {
                *field
            } else {
                false
            }
        }
        ///If `u256_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn u256_tag_opt(&self) -> Option<bool> {
            if let Some(super::type_tag::TypeTag::U256Tag(field)) = &self.type_tag {
                Some(*field)
            } else {
                None
            }
        }
        ///If `u256_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn u256_tag_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(super::type_tag::TypeTag::U256Tag(field)) = &mut self.type_tag {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `u256_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn u256_tag_mut(&mut self) -> &mut bool {
            if self.u256_tag_opt_mut().is_none() {
                self.type_tag = Some(super::type_tag::TypeTag::U256Tag(bool::default()));
            }
            self.u256_tag_opt_mut().unwrap()
        }
        ///Sets `u256_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_u256_tag(&mut self, field: bool) {
            self.type_tag = Some(super::type_tag::TypeTag::U256Tag(field));
        }
        ///Sets `u256_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_u256_tag(mut self, field: bool) -> Self {
            self.set_u256_tag(field);
            self
        }
        ///Returns the value of `address_tag`, or the default value if `address_tag` is unset.
        pub fn address_tag(&self) -> bool {
            if let Some(super::type_tag::TypeTag::AddressTag(field)) = &self.type_tag {
                *field
            } else {
                false
            }
        }
        ///If `address_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn address_tag_opt(&self) -> Option<bool> {
            if let Some(super::type_tag::TypeTag::AddressTag(field)) = &self.type_tag {
                Some(*field)
            } else {
                None
            }
        }
        ///If `address_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn address_tag_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(super::type_tag::TypeTag::AddressTag(field)) = &mut self.type_tag
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `address_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn address_tag_mut(&mut self) -> &mut bool {
            if self.address_tag_opt_mut().is_none() {
                self.type_tag = Some(
                    super::type_tag::TypeTag::AddressTag(bool::default()),
                );
            }
            self.address_tag_opt_mut().unwrap()
        }
        ///Sets `address_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_address_tag(&mut self, field: bool) {
            self.type_tag = Some(super::type_tag::TypeTag::AddressTag(field));
        }
        ///Sets `address_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_address_tag(mut self, field: bool) -> Self {
            self.set_address_tag(field);
            self
        }
        ///Returns the value of `signer_tag`, or the default value if `signer_tag` is unset.
        pub fn signer_tag(&self) -> bool {
            if let Some(super::type_tag::TypeTag::SignerTag(field)) = &self.type_tag {
                *field
            } else {
                false
            }
        }
        ///If `signer_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn signer_tag_opt(&self) -> Option<bool> {
            if let Some(super::type_tag::TypeTag::SignerTag(field)) = &self.type_tag {
                Some(*field)
            } else {
                None
            }
        }
        ///If `signer_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn signer_tag_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(super::type_tag::TypeTag::SignerTag(field)) = &mut self.type_tag
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `signer_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn signer_tag_mut(&mut self) -> &mut bool {
            if self.signer_tag_opt_mut().is_none() {
                self.type_tag = Some(
                    super::type_tag::TypeTag::SignerTag(bool::default()),
                );
            }
            self.signer_tag_opt_mut().unwrap()
        }
        ///Sets `signer_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_signer_tag(&mut self, field: bool) {
            self.type_tag = Some(super::type_tag::TypeTag::SignerTag(field));
        }
        ///Sets `signer_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_signer_tag(mut self, field: bool) -> Self {
            self.set_signer_tag(field);
            self
        }
        ///Returns the value of `vector_tag`, or the default value if `vector_tag` is unset.
        pub fn vector_tag(&self) -> &super::TypeTagVector {
            if let Some(super::type_tag::TypeTag::VectorTag(field)) = &self.type_tag {
                field as _
            } else {
                super::TypeTagVector::default_instance() as _
            }
        }
        ///If `vector_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn vector_tag_opt(&self) -> Option<&super::TypeTagVector> {
            if let Some(super::type_tag::TypeTag::VectorTag(field)) = &self.type_tag {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `vector_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn vector_tag_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost::alloc::boxed::Box<super::TypeTagVector>> {
            if let Some(super::type_tag::TypeTag::VectorTag(field)) = &mut self.type_tag
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `vector_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn vector_tag_mut(
            &mut self,
        ) -> &mut ::prost::alloc::boxed::Box<super::TypeTagVector> {
            if self.vector_tag_opt_mut().is_none() {
                self.type_tag = Some(
                    super::type_tag::TypeTag::VectorTag(
                        ::prost::alloc::boxed::Box::default(),
                    ),
                );
            }
            self.vector_tag_opt_mut().unwrap()
        }
        ///Sets `vector_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_vector_tag<T: Into<::prost::alloc::boxed::Box<super::TypeTagVector>>>(
            &mut self,
            field: T,
        ) {
            self.type_tag = Some(
                super::type_tag::TypeTag::VectorTag(field.into().into()),
            );
        }
        ///Sets `vector_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_vector_tag<
            T: Into<::prost::alloc::boxed::Box<super::TypeTagVector>>,
        >(mut self, field: T) -> Self {
            self.set_vector_tag(field.into());
            self
        }
        ///Returns the value of `struct_tag`, or the default value if `struct_tag` is unset.
        pub fn struct_tag(&self) -> &super::TypeTagStruct {
            if let Some(super::type_tag::TypeTag::StructTag(field)) = &self.type_tag {
                field as _
            } else {
                super::TypeTagStruct::default_instance() as _
            }
        }
        ///If `struct_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn struct_tag_opt(&self) -> Option<&super::TypeTagStruct> {
            if let Some(super::type_tag::TypeTag::StructTag(field)) = &self.type_tag {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `struct_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn struct_tag_opt_mut(&mut self) -> Option<&mut super::TypeTagStruct> {
            if let Some(super::type_tag::TypeTag::StructTag(field)) = &mut self.type_tag
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `struct_tag`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn struct_tag_mut(&mut self) -> &mut super::TypeTagStruct {
            if self.struct_tag_opt_mut().is_none() {
                self.type_tag = Some(
                    super::type_tag::TypeTag::StructTag(super::TypeTagStruct::default()),
                );
            }
            self.struct_tag_opt_mut().unwrap()
        }
        ///Sets `struct_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_struct_tag<T: Into<super::TypeTagStruct>>(&mut self, field: T) {
            self.type_tag = Some(
                super::type_tag::TypeTag::StructTag(field.into().into()),
            );
        }
        ///Sets `struct_tag` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_struct_tag<T: Into<super::TypeTagStruct>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_struct_tag(field.into());
            self
        }
    }
    impl super::TypeTagStruct {
        pub const fn const_default() -> Self {
            Self { struct_tag: String::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TypeTagStruct = super::TypeTagStruct::const_default();
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
    impl super::TypeTagVector {
        pub const fn const_default() -> Self {
            Self { inner_type: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TypeTagVector = super::TypeTagVector::const_default();
            &DEFAULT
        }
        ///Returns the value of `inner_type`, or the default value if `inner_type` is unset.
        pub fn inner_type(&self) -> &super::TypeTag {
            self.inner_type
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::TypeTag::default_instance() as _)
        }
        ///If `inner_type` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn inner_type_opt_mut(&mut self) -> Option<&mut super::TypeTag> {
            self.inner_type.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `inner_type`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn inner_type_mut(&mut self) -> &mut super::TypeTag {
            self.inner_type.get_or_insert_default()
        }
        ///If `inner_type` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn inner_type_opt(&self) -> Option<&super::TypeTag> {
            self.inner_type.as_ref().map(|field| field as _)
        }
        ///Sets `inner_type` with the provided value.
        pub fn set_inner_type<T: Into<super::TypeTag>>(&mut self, field: T) {
            self.inner_type = Some(field.into().into());
        }
        ///Sets `inner_type` with the provided value.
        pub fn with_inner_type<T: Into<super::TypeTag>>(mut self, field: T) -> Self {
            self.set_inner_type(field.into());
            self
        }
    }
    impl super::TypeTags {
        pub const fn const_default() -> Self {
            Self { type_tags: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TypeTags = super::TypeTags::const_default();
            &DEFAULT
        }
        ///Returns the value of `type_tags`, or the default value if `type_tags` is unset.
        pub fn type_tags(&self) -> &[super::TypeTag] {
            &self.type_tags
        }
        ///Returns a mutable reference to `type_tags`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn type_tags_mut(&mut self) -> &mut Vec<super::TypeTag> {
            &mut self.type_tags
        }
        ///Sets `type_tags` with the provided value.
        pub fn set_type_tags(&mut self, field: Vec<super::TypeTag>) {
            self.type_tags = field;
        }
        ///Sets `type_tags` with the provided value.
        pub fn with_type_tags(mut self, field: Vec<super::TypeTag>) -> Self {
            self.set_type_tags(field);
            self
        }
    }
}
