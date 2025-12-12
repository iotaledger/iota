// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Argument {
        pub const fn const_default() -> Self {
            Self { kind: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Argument = super::Argument::const_default();
            &DEFAULT
        }
        ///Returns the value of `unknown`, or the default value if `unknown` is unset.
        pub fn unknown(&self) -> &super::argument::Unknown {
            if let Some(super::argument::Kind::Unknown(field)) = &self.kind {
                field as _
            } else {
                super::argument::Unknown::default_instance() as _
            }
        }
        ///If `unknown` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn unknown_opt(&self) -> Option<&super::argument::Unknown> {
            if let Some(super::argument::Kind::Unknown(field)) = &self.kind {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `unknown` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn unknown_opt_mut(&mut self) -> Option<&mut super::argument::Unknown> {
            if let Some(super::argument::Kind::Unknown(field)) = &mut self.kind {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `unknown`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn unknown_mut(&mut self) -> &mut super::argument::Unknown {
            if self.unknown_opt_mut().is_none() {
                self.kind = Some(
                    super::argument::Kind::Unknown(super::argument::Unknown::default()),
                );
            }
            self.unknown_opt_mut().unwrap()
        }
        ///Sets `unknown` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_unknown<T: Into<super::argument::Unknown>>(&mut self, field: T) {
            self.kind = Some(super::argument::Kind::Unknown(field.into().into()));
        }
        ///Sets `unknown` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_unknown<T: Into<super::argument::Unknown>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_unknown(field.into());
            self
        }
        ///Returns the value of `gas_coin`, or the default value if `gas_coin` is unset.
        pub fn gas_coin(&self) -> &super::argument::GasCoin {
            if let Some(super::argument::Kind::GasCoin(field)) = &self.kind {
                field as _
            } else {
                super::argument::GasCoin::default_instance() as _
            }
        }
        ///If `gas_coin` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn gas_coin_opt(&self) -> Option<&super::argument::GasCoin> {
            if let Some(super::argument::Kind::GasCoin(field)) = &self.kind {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `gas_coin` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn gas_coin_opt_mut(&mut self) -> Option<&mut super::argument::GasCoin> {
            if let Some(super::argument::Kind::GasCoin(field)) = &mut self.kind {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `gas_coin`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn gas_coin_mut(&mut self) -> &mut super::argument::GasCoin {
            if self.gas_coin_opt_mut().is_none() {
                self.kind = Some(
                    super::argument::Kind::GasCoin(super::argument::GasCoin::default()),
                );
            }
            self.gas_coin_opt_mut().unwrap()
        }
        ///Sets `gas_coin` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_gas_coin<T: Into<super::argument::GasCoin>>(&mut self, field: T) {
            self.kind = Some(super::argument::Kind::GasCoin(field.into().into()));
        }
        ///Sets `gas_coin` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_gas_coin<T: Into<super::argument::GasCoin>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_gas_coin(field.into());
            self
        }
        ///Returns the value of `input`, or the default value if `input` is unset.
        pub fn input(&self) -> &super::argument::Input {
            if let Some(super::argument::Kind::Input(field)) = &self.kind {
                field as _
            } else {
                super::argument::Input::default_instance() as _
            }
        }
        ///If `input` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn input_opt(&self) -> Option<&super::argument::Input> {
            if let Some(super::argument::Kind::Input(field)) = &self.kind {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `input` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn input_opt_mut(&mut self) -> Option<&mut super::argument::Input> {
            if let Some(super::argument::Kind::Input(field)) = &mut self.kind {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `input`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn input_mut(&mut self) -> &mut super::argument::Input {
            if self.input_opt_mut().is_none() {
                self.kind = Some(
                    super::argument::Kind::Input(super::argument::Input::default()),
                );
            }
            self.input_opt_mut().unwrap()
        }
        ///Sets `input` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_input<T: Into<super::argument::Input>>(&mut self, field: T) {
            self.kind = Some(super::argument::Kind::Input(field.into().into()));
        }
        ///Sets `input` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_input<T: Into<super::argument::Input>>(mut self, field: T) -> Self {
            self.set_input(field.into());
            self
        }
        ///Returns the value of `result`, or the default value if `result` is unset.
        pub fn result(&self) -> &super::argument::Result {
            if let Some(super::argument::Kind::Result(field)) = &self.kind {
                field as _
            } else {
                super::argument::Result::default_instance() as _
            }
        }
        ///If `result` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn result_opt(&self) -> Option<&super::argument::Result> {
            if let Some(super::argument::Kind::Result(field)) = &self.kind {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `result` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn result_opt_mut(&mut self) -> Option<&mut super::argument::Result> {
            if let Some(super::argument::Kind::Result(field)) = &mut self.kind {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `result`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn result_mut(&mut self) -> &mut super::argument::Result {
            if self.result_opt_mut().is_none() {
                self.kind = Some(
                    super::argument::Kind::Result(super::argument::Result::default()),
                );
            }
            self.result_opt_mut().unwrap()
        }
        ///Sets `result` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_result<T: Into<super::argument::Result>>(&mut self, field: T) {
            self.kind = Some(super::argument::Kind::Result(field.into().into()));
        }
        ///Sets `result` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_result<T: Into<super::argument::Result>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_result(field.into());
            self
        }
    }
    impl super::argument::GasCoin {
        pub const fn const_default() -> Self {
            Self {}
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::argument::GasCoin = super::argument::GasCoin::const_default();
            &DEFAULT
        }
    }
    impl super::argument::Input {
        pub const fn const_default() -> Self {
            Self { index: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::argument::Input = super::argument::Input::const_default();
            &DEFAULT
        }
        ///If `index` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn index_opt_mut(&mut self) -> Option<&mut u32> {
            self.index.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `index`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn index_mut(&mut self) -> &mut u32 {
            self.index.get_or_insert_default()
        }
        ///If `index` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn index_opt(&self) -> Option<u32> {
            self.index.as_ref().map(|field| *field)
        }
        ///Sets `index` with the provided value.
        pub fn set_index(&mut self, field: u32) {
            self.index = Some(field);
        }
        ///Sets `index` with the provided value.
        pub fn with_index(mut self, field: u32) -> Self {
            self.set_index(field);
            self
        }
    }
    impl super::argument::Result {
        pub const fn const_default() -> Self {
            Self {
                index: None,
                nested_result_index: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::argument::Result = super::argument::Result::const_default();
            &DEFAULT
        }
        ///If `index` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn index_opt_mut(&mut self) -> Option<&mut u32> {
            self.index.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `index`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn index_mut(&mut self) -> &mut u32 {
            self.index.get_or_insert_default()
        }
        ///If `index` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn index_opt(&self) -> Option<u32> {
            self.index.as_ref().map(|field| *field)
        }
        ///Sets `index` with the provided value.
        pub fn set_index(&mut self, field: u32) {
            self.index = Some(field);
        }
        ///Sets `index` with the provided value.
        pub fn with_index(mut self, field: u32) -> Self {
            self.set_index(field);
            self
        }
        ///If `nested_result_index` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn nested_result_index_opt_mut(&mut self) -> Option<&mut u32> {
            self.nested_result_index.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `nested_result_index`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn nested_result_index_mut(&mut self) -> &mut u32 {
            self.nested_result_index.get_or_insert_default()
        }
        ///If `nested_result_index` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn nested_result_index_opt(&self) -> Option<u32> {
            self.nested_result_index.as_ref().map(|field| *field)
        }
        ///Sets `nested_result_index` with the provided value.
        pub fn set_nested_result_index(&mut self, field: u32) {
            self.nested_result_index = Some(field);
        }
        ///Sets `nested_result_index` with the provided value.
        pub fn with_nested_result_index(mut self, field: u32) -> Self {
            self.set_nested_result_index(field);
            self
        }
    }
    impl super::argument::Unknown {
        pub const fn const_default() -> Self {
            Self {}
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::argument::Unknown = super::argument::Unknown::const_default();
            &DEFAULT
        }
    }
    impl super::CommandOutput {
        pub const fn const_default() -> Self {
            Self {
                argument: None,
                type_tag: None,
                bcs: None,
                json: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::CommandOutput = super::CommandOutput::const_default();
            &DEFAULT
        }
        ///Returns the value of `argument`, or the default value if `argument` is unset.
        pub fn argument(&self) -> &super::Argument {
            self.argument
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::Argument::default_instance() as _)
        }
        ///If `argument` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn argument_opt_mut(&mut self) -> Option<&mut super::Argument> {
            self.argument.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `argument`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn argument_mut(&mut self) -> &mut super::Argument {
            self.argument.get_or_insert_default()
        }
        ///If `argument` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn argument_opt(&self) -> Option<&super::Argument> {
            self.argument.as_ref().map(|field| field as _)
        }
        ///Sets `argument` with the provided value.
        pub fn set_argument<T: Into<super::Argument>>(&mut self, field: T) {
            self.argument = Some(field.into().into());
        }
        ///Sets `argument` with the provided value.
        pub fn with_argument<T: Into<super::Argument>>(mut self, field: T) -> Self {
            self.set_argument(field.into());
            self
        }
        ///Returns the value of `type_tag`, or the default value if `type_tag` is unset.
        pub fn type_tag(&self) -> &super::super::types::TypeTag {
            self.type_tag
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::TypeTag::default_instance() as _)
        }
        ///If `type_tag` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn type_tag_opt_mut(&mut self) -> Option<&mut super::super::types::TypeTag> {
            self.type_tag.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `type_tag`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn type_tag_mut(&mut self) -> &mut super::super::types::TypeTag {
            self.type_tag.get_or_insert_default()
        }
        ///If `type_tag` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn type_tag_opt(&self) -> Option<&super::super::types::TypeTag> {
            self.type_tag.as_ref().map(|field| field as _)
        }
        ///Sets `type_tag` with the provided value.
        pub fn set_type_tag<T: Into<super::super::types::TypeTag>>(&mut self, field: T) {
            self.type_tag = Some(field.into().into());
        }
        ///Sets `type_tag` with the provided value.
        pub fn with_type_tag<T: Into<super::super::types::TypeTag>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_type_tag(field.into());
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
        ///If `json` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn json_opt_mut(&mut self) -> Option<&mut ::prost_types::Value> {
            self.json.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `json`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn json_mut(&mut self) -> &mut ::prost_types::Value {
            self.json.get_or_insert_default()
        }
        ///If `json` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn json_opt(&self) -> Option<&::prost_types::Value> {
            self.json.as_ref().map(|field| field as _)
        }
        ///Sets `json` with the provided value.
        pub fn set_json<T: Into<::prost_types::Value>>(&mut self, field: T) {
            self.json = Some(field.into().into());
        }
        ///Sets `json` with the provided value.
        pub fn with_json<T: Into<::prost_types::Value>>(mut self, field: T) -> Self {
            self.set_json(field.into());
            self
        }
    }
    impl super::CommandOutputs {
        pub const fn const_default() -> Self {
            Self { outputs: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::CommandOutputs = super::CommandOutputs::const_default();
            &DEFAULT
        }
        ///Returns the value of `outputs`, or the default value if `outputs` is unset.
        pub fn outputs(&self) -> &[super::CommandOutput] {
            &self.outputs
        }
        ///Returns a mutable reference to `outputs`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn outputs_mut(&mut self) -> &mut Vec<super::CommandOutput> {
            &mut self.outputs
        }
        ///Sets `outputs` with the provided value.
        pub fn set_outputs(&mut self, field: Vec<super::CommandOutput>) {
            self.outputs = field;
        }
        ///Sets `outputs` with the provided value.
        pub fn with_outputs(mut self, field: Vec<super::CommandOutput>) -> Self {
            self.set_outputs(field);
            self
        }
    }
    impl super::CommandResult {
        pub const fn const_default() -> Self {
            Self {
                mutated_by_ref: None,
                return_values: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::CommandResult = super::CommandResult::const_default();
            &DEFAULT
        }
        ///Returns the value of `mutated_by_ref`, or the default value if `mutated_by_ref` is unset.
        pub fn mutated_by_ref(&self) -> &super::CommandOutputs {
            self.mutated_by_ref
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::CommandOutputs::default_instance() as _)
        }
        ///If `mutated_by_ref` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn mutated_by_ref_opt_mut(&mut self) -> Option<&mut super::CommandOutputs> {
            self.mutated_by_ref.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `mutated_by_ref`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn mutated_by_ref_mut(&mut self) -> &mut super::CommandOutputs {
            self.mutated_by_ref.get_or_insert_default()
        }
        ///If `mutated_by_ref` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn mutated_by_ref_opt(&self) -> Option<&super::CommandOutputs> {
            self.mutated_by_ref.as_ref().map(|field| field as _)
        }
        ///Sets `mutated_by_ref` with the provided value.
        pub fn set_mutated_by_ref<T: Into<super::CommandOutputs>>(&mut self, field: T) {
            self.mutated_by_ref = Some(field.into().into());
        }
        ///Sets `mutated_by_ref` with the provided value.
        pub fn with_mutated_by_ref<T: Into<super::CommandOutputs>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_mutated_by_ref(field.into());
            self
        }
        ///Returns the value of `return_values`, or the default value if `return_values` is unset.
        pub fn return_values(&self) -> &super::CommandOutputs {
            self.return_values
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::CommandOutputs::default_instance() as _)
        }
        ///If `return_values` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn return_values_opt_mut(&mut self) -> Option<&mut super::CommandOutputs> {
            self.return_values.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `return_values`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn return_values_mut(&mut self) -> &mut super::CommandOutputs {
            self.return_values.get_or_insert_default()
        }
        ///If `return_values` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn return_values_opt(&self) -> Option<&super::CommandOutputs> {
            self.return_values.as_ref().map(|field| field as _)
        }
        ///Sets `return_values` with the provided value.
        pub fn set_return_values<T: Into<super::CommandOutputs>>(&mut self, field: T) {
            self.return_values = Some(field.into().into());
        }
        ///Sets `return_values` with the provided value.
        pub fn with_return_values<T: Into<super::CommandOutputs>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_return_values(field.into());
            self
        }
    }
    impl super::CommandResults {
        pub const fn const_default() -> Self {
            Self { results: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::CommandResults = super::CommandResults::const_default();
            &DEFAULT
        }
        ///Returns the value of `results`, or the default value if `results` is unset.
        pub fn results(&self) -> &[super::CommandResult] {
            &self.results
        }
        ///Returns a mutable reference to `results`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn results_mut(&mut self) -> &mut Vec<super::CommandResult> {
            &mut self.results
        }
        ///Sets `results` with the provided value.
        pub fn set_results(&mut self, field: Vec<super::CommandResult>) {
            self.results = field;
        }
        ///Sets `results` with the provided value.
        pub fn with_results(mut self, field: Vec<super::CommandResult>) -> Self {
            self.set_results(field);
            self
        }
    }
}
