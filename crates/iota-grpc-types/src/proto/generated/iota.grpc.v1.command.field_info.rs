// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v1::bcs::BcsData;
    #[allow(unused_imports)]
    use crate::v1::bcs::BcsDataFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::TypeTag;
    #[allow(unused_imports)]
    use crate::v1::types::TypeTagFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::command::argument::Unknown;
    #[allow(unused_imports)]
    use crate::v1::command::argument::GasCoin;
    #[allow(unused_imports)]
    use crate::v1::command::argument::Input;
    #[allow(unused_imports)]
    use crate::v1::command::argument::Result;
    pub mod argument {
        use super::*;
        impl Unknown {}
        impl MessageFields for Unknown {
            const FIELDS: &'static [&'static MessageField] = &[];
        }
        impl Unknown {
            pub fn path_builder() -> UnknownFieldPathBuilder {
                UnknownFieldPathBuilder::new()
            }
        }
        pub struct UnknownFieldPathBuilder {
            path: Vec<&'static str>,
        }
        impl UnknownFieldPathBuilder {
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
        }
        impl GasCoin {}
        impl MessageFields for GasCoin {
            const FIELDS: &'static [&'static MessageField] = &[];
        }
        impl GasCoin {
            pub fn path_builder() -> GasCoinFieldPathBuilder {
                GasCoinFieldPathBuilder::new()
            }
        }
        pub struct GasCoinFieldPathBuilder {
            path: Vec<&'static str>,
        }
        impl GasCoinFieldPathBuilder {
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
        }
        impl Input {
            pub const INDEX_FIELD: &'static MessageField = &MessageField {
                name: "index",
                json_name: "index",
                number: 1i32,
                is_optional: true,
                is_map: false,
                message_fields: None,
            };
        }
        impl MessageFields for Input {
            const FIELDS: &'static [&'static MessageField] = &[Self::INDEX_FIELD];
        }
        impl Input {
            pub fn path_builder() -> InputFieldPathBuilder {
                InputFieldPathBuilder::new()
            }
        }
        pub struct InputFieldPathBuilder {
            path: Vec<&'static str>,
        }
        impl InputFieldPathBuilder {
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
            pub fn index(mut self) -> String {
                self.path.push(Input::INDEX_FIELD.name);
                self.finish()
            }
        }
        impl Result {
            pub const INDEX_FIELD: &'static MessageField = &MessageField {
                name: "index",
                json_name: "index",
                number: 1i32,
                is_optional: true,
                is_map: false,
                message_fields: None,
            };
            pub const NESTED_RESULT_INDEX_FIELD: &'static MessageField = &MessageField {
                name: "nested_result_index",
                json_name: "nestedResultIndex",
                number: 2i32,
                is_optional: true,
                is_map: false,
                message_fields: None,
            };
        }
        impl MessageFields for Result {
            const FIELDS: &'static [&'static MessageField] = &[
                Self::INDEX_FIELD,
                Self::NESTED_RESULT_INDEX_FIELD,
            ];
        }
        impl Result {
            pub fn path_builder() -> ResultFieldPathBuilder {
                ResultFieldPathBuilder::new()
            }
        }
        pub struct ResultFieldPathBuilder {
            path: Vec<&'static str>,
        }
        impl ResultFieldPathBuilder {
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
            pub fn index(mut self) -> String {
                self.path.push(Result::INDEX_FIELD.name);
                self.finish()
            }
            pub fn nested_result_index(mut self) -> String {
                self.path.push(Result::NESTED_RESULT_INDEX_FIELD.name);
                self.finish()
            }
        }
    }
    impl Argument {
        pub const UNKNOWN_FIELD: &'static MessageField = &MessageField {
            name: "unknown",
            json_name: "unknown",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Unknown::FIELDS),
        };
        pub const GAS_COIN_FIELD: &'static MessageField = &MessageField {
            name: "gas_coin",
            json_name: "gasCoin",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(GasCoin::FIELDS),
        };
        pub const INPUT_FIELD: &'static MessageField = &MessageField {
            name: "input",
            json_name: "input",
            number: 3i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Input::FIELDS),
        };
        pub const RESULT_FIELD: &'static MessageField = &MessageField {
            name: "result",
            json_name: "result",
            number: 4i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Result::FIELDS),
        };
    }
    impl Argument {
        pub const KIND_ONEOF: &'static str = "kind";
    }
    impl MessageFields for Argument {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::UNKNOWN_FIELD,
            Self::GAS_COIN_FIELD,
            Self::INPUT_FIELD,
            Self::RESULT_FIELD,
        ];
        const ONEOFS: &'static [&'static str] = &["kind"];
    }
    impl Argument {
        pub fn path_builder() -> ArgumentFieldPathBuilder {
            ArgumentFieldPathBuilder::new()
        }
    }
    pub struct ArgumentFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ArgumentFieldPathBuilder {
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
        pub fn unknown(mut self) -> argument::UnknownFieldPathBuilder {
            self.path.push(Argument::UNKNOWN_FIELD.name);
            argument::UnknownFieldPathBuilder::new_with_base(self.path)
        }
        pub fn gas_coin(mut self) -> argument::GasCoinFieldPathBuilder {
            self.path.push(Argument::GAS_COIN_FIELD.name);
            argument::GasCoinFieldPathBuilder::new_with_base(self.path)
        }
        pub fn input(mut self) -> argument::InputFieldPathBuilder {
            self.path.push(Argument::INPUT_FIELD.name);
            argument::InputFieldPathBuilder::new_with_base(self.path)
        }
        pub fn result(mut self) -> argument::ResultFieldPathBuilder {
            self.path.push(Argument::RESULT_FIELD.name);
            argument::ResultFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl CommandOutput {
        pub const ARGUMENT_FIELD: &'static MessageField = &MessageField {
            name: "argument",
            json_name: "argument",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Argument::FIELDS),
        };
        pub const TYPE_TAG_FIELD: &'static MessageField = &MessageField {
            name: "type_tag",
            json_name: "typeTag",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(TypeTag::FIELDS),
        };
        pub const BCS_FIELD: &'static MessageField = &MessageField {
            name: "bcs",
            json_name: "bcs",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
        pub const JSON_FIELD: &'static MessageField = &MessageField {
            name: "json",
            json_name: "json",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for CommandOutput {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::ARGUMENT_FIELD,
            Self::TYPE_TAG_FIELD,
            Self::BCS_FIELD,
            Self::JSON_FIELD,
        ];
    }
    impl CommandOutput {
        pub fn path_builder() -> CommandOutputFieldPathBuilder {
            CommandOutputFieldPathBuilder::new()
        }
    }
    pub struct CommandOutputFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CommandOutputFieldPathBuilder {
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
        pub fn argument(mut self) -> ArgumentFieldPathBuilder {
            self.path.push(CommandOutput::ARGUMENT_FIELD.name);
            ArgumentFieldPathBuilder::new_with_base(self.path)
        }
        pub fn type_tag(mut self) -> TypeTagFieldPathBuilder {
            self.path.push(CommandOutput::TYPE_TAG_FIELD.name);
            TypeTagFieldPathBuilder::new_with_base(self.path)
        }
        pub fn bcs(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(CommandOutput::BCS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
        pub fn json(mut self) -> String {
            self.path.push(CommandOutput::JSON_FIELD.name);
            self.finish()
        }
    }
    impl CommandOutputs {
        pub const OUTPUTS_FIELD: &'static MessageField = &MessageField {
            name: "outputs",
            json_name: "outputs",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(CommandOutput::FIELDS),
        };
    }
    impl MessageFields for CommandOutputs {
        const FIELDS: &'static [&'static MessageField] = &[Self::OUTPUTS_FIELD];
    }
    impl CommandOutputs {
        pub fn path_builder() -> CommandOutputsFieldPathBuilder {
            CommandOutputsFieldPathBuilder::new()
        }
    }
    pub struct CommandOutputsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CommandOutputsFieldPathBuilder {
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
        pub fn outputs(mut self) -> CommandOutputFieldPathBuilder {
            self.path.push(CommandOutputs::OUTPUTS_FIELD.name);
            CommandOutputFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl CommandResult {
        pub const MUTATED_BY_REF_FIELD: &'static MessageField = &MessageField {
            name: "mutated_by_ref",
            json_name: "mutatedByRef",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(CommandOutput::FIELDS),
        };
        pub const RETURN_VALUES_FIELD: &'static MessageField = &MessageField {
            name: "return_values",
            json_name: "returnValues",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(CommandOutput::FIELDS),
        };
    }
    impl MessageFields for CommandResult {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::MUTATED_BY_REF_FIELD,
            Self::RETURN_VALUES_FIELD,
        ];
    }
    impl CommandResult {
        pub fn path_builder() -> CommandResultFieldPathBuilder {
            CommandResultFieldPathBuilder::new()
        }
    }
    pub struct CommandResultFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CommandResultFieldPathBuilder {
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
        pub fn mutated_by_ref(mut self) -> CommandOutputFieldPathBuilder {
            self.path.push(CommandResult::MUTATED_BY_REF_FIELD.name);
            CommandOutputFieldPathBuilder::new_with_base(self.path)
        }
        pub fn return_values(mut self) -> CommandOutputFieldPathBuilder {
            self.path.push(CommandResult::RETURN_VALUES_FIELD.name);
            CommandOutputFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl CommandResults {
        pub const RESULTS_FIELD: &'static MessageField = &MessageField {
            name: "results",
            json_name: "results",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(CommandResult::FIELDS),
        };
    }
    impl MessageFields for CommandResults {
        const FIELDS: &'static [&'static MessageField] = &[Self::RESULTS_FIELD];
    }
    impl CommandResults {
        pub fn path_builder() -> CommandResultsFieldPathBuilder {
            CommandResultsFieldPathBuilder::new()
        }
    }
    pub struct CommandResultsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CommandResultsFieldPathBuilder {
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
        pub fn results(mut self) -> CommandResultFieldPathBuilder {
            self.path.push(CommandResults::RESULTS_FIELD.name);
            CommandResultFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
