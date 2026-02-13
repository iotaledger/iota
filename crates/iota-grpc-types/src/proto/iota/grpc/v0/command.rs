// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.command.rs");
include!("../../../generated/iota.grpc.v0.command.field_info.rs");

use crate::{
    proto::{GrpcConversionError, TryFromProtoError, get_inner_field},
    v0::bcs::BcsData,
};

impl TryFrom<iota_sdk_types::transaction::Argument> for Argument {
    type Error = GrpcConversionError;

    fn try_from(arg: iota_sdk_types::transaction::Argument) -> Result<Self, Self::Error> {
        let kind = match arg {
            iota_sdk_types::transaction::Argument::Gas => {
                argument::Kind::GasCoin(argument::GasCoin {})
            }
            iota_sdk_types::transaction::Argument::Input(idx) => {
                argument::Kind::Input(argument::Input {
                    index: Some(idx as u32),
                })
            }
            iota_sdk_types::transaction::Argument::Result(idx) => {
                argument::Kind::Result(argument::Result {
                    index: Some(idx as u32),
                    nested_result_index: None,
                })
            }
            iota_sdk_types::transaction::Argument::NestedResult(idx, nested_idx) => {
                argument::Kind::Result(argument::Result {
                    index: Some(idx as u32),
                    nested_result_index: Some(nested_idx as u32),
                })
            }
            _ => {
                return Err(GrpcConversionError::UnsupportedArgumentType {
                    arg_type: format!("{:?}", arg),
                });
            }
        };

        Ok(Self { kind: Some(kind) })
    }
}

impl TryFrom<&Argument> for iota_sdk_types::transaction::Argument {
    type Error = TryFromProtoError;

    fn try_from(value: &Argument) -> Result<Self, Self::Error> {
        match &value.kind {
            Some(argument::Kind::GasCoin(_)) => Ok(iota_sdk_types::transaction::Argument::Gas),
            Some(argument::Kind::Input(input)) => {
                let index = input
                    .index
                    .ok_or_else(|| TryFromProtoError::missing("argument.input.index"))?;
                Ok(iota_sdk_types::transaction::Argument::Input(index as u16))
            }
            Some(argument::Kind::Result(result)) => {
                let index = result
                    .index
                    .ok_or_else(|| TryFromProtoError::missing("argument.result.index"))?;
                match result.nested_result_index {
                    Some(nested_idx) => Ok(iota_sdk_types::transaction::Argument::NestedResult(
                        index as u16,
                        nested_idx as u16,
                    )),
                    None => Ok(iota_sdk_types::transaction::Argument::Result(index as u16)),
                }
            }
            Some(argument::Kind::Unknown(_)) => Err(TryFromProtoError::invalid(
                "argument.kind",
                "unknown argument type",
            )),
            None => Err(TryFromProtoError::missing("argument.kind")),
        }
    }
}

// Argument
//

impl Argument {
    /// Deserialize the argument to SDK type.
    pub fn argument(&self) -> Result<iota_sdk_types::transaction::Argument, TryFromProtoError> {
        self.try_into()
    }
}

// CommandOutput
//

impl CommandOutput {
    /// Deserialize the argument to SDK type.
    pub fn argument(&self) -> Result<iota_sdk_types::transaction::Argument, TryFromProtoError> {
        get_inner_field!(self.argument, Self::ARGUMENT_FIELD, argument)
    }

    /// Deserialize the type tag to SDK type.
    pub fn type_tag(&self) -> Result<iota_sdk_types::TypeTag, TryFromProtoError> {
        get_inner_field!(self.type_tag, Self::TYPE_TAG_FIELD, type_tag)
    }

    /// Get the raw BCS bytes.
    pub fn output_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_FIELD.name))
    }

    /// Get the JSON value.
    pub fn output_json(&self) -> Result<serde_json::Value, TryFromProtoError> {
        self.json
            .as_ref()
            .map(crate::proto::prost_to_json)
            .ok_or_else(|| TryFromProtoError::missing(Self::JSON_FIELD.name))
    }
}

// CommandOutputs
//

impl CommandOutputs {
    /// Deserialize all arguments to SDK types.
    pub fn arguments(
        &self,
    ) -> Result<Vec<iota_sdk_types::transaction::Argument>, TryFromProtoError> {
        self.outputs
            .iter()
            .enumerate()
            .map(|(i, o)| {
                o.argument()
                    .map_err(|e| e.nested_at(Self::OUTPUTS_FIELD.name, i))
            })
            .collect()
    }

    /// Deserialize all type tags to SDK types.
    pub fn type_tags(&self) -> Result<Vec<iota_sdk_types::TypeTag>, TryFromProtoError> {
        self.outputs
            .iter()
            .enumerate()
            .map(|(i, o)| {
                o.type_tag()
                    .map_err(|e| e.nested_at(Self::OUTPUTS_FIELD.name, i))
            })
            .collect()
    }

    /// Get all BCS bytes.
    pub fn all_bcs(&self) -> Result<Vec<&[u8]>, TryFromProtoError> {
        self.outputs
            .iter()
            .enumerate()
            .map(|(i, o)| {
                o.output_bcs()
                    .map_err(|e| e.nested_at(Self::OUTPUTS_FIELD.name, i))
            })
            .collect()
    }

    /// Get all JSON values.
    pub fn all_json(&self) -> Result<Vec<serde_json::Value>, TryFromProtoError> {
        self.outputs
            .iter()
            .enumerate()
            .map(|(i, o)| {
                o.output_json()
                    .map_err(|e| e.nested_at(Self::OUTPUTS_FIELD.name, i))
            })
            .collect()
    }
}

// CommandResult
//

impl CommandResult {
    /// Get the arguments for outputs mutated by reference.
    pub fn mutated_by_ref_arguments(
        &self,
    ) -> Result<Vec<iota_sdk_types::transaction::Argument>, TryFromProtoError> {
        get_inner_field!(self.mutated_by_ref, Self::MUTATED_BY_REF_FIELD, arguments)
    }

    /// Get the type tags for outputs mutated by reference.
    pub fn mutated_by_ref_type_tags(
        &self,
    ) -> Result<Vec<iota_sdk_types::TypeTag>, TryFromProtoError> {
        get_inner_field!(self.mutated_by_ref, Self::MUTATED_BY_REF_FIELD, type_tags)
    }

    /// Get the BCS bytes for outputs mutated by reference.
    pub fn mutated_by_ref_bcs(&self) -> Result<Vec<&[u8]>, TryFromProtoError> {
        get_inner_field!(self.mutated_by_ref, Self::MUTATED_BY_REF_FIELD, all_bcs)
    }

    /// Get the JSON values for outputs mutated by reference.
    pub fn mutated_by_ref_json(&self) -> Result<Vec<serde_json::Value>, TryFromProtoError> {
        get_inner_field!(self.mutated_by_ref, Self::MUTATED_BY_REF_FIELD, all_json)
    }

    /// Get the arguments for return values.
    pub fn return_values_arguments(
        &self,
    ) -> Result<Vec<iota_sdk_types::transaction::Argument>, TryFromProtoError> {
        get_inner_field!(self.return_values, Self::RETURN_VALUES_FIELD, arguments)
    }

    /// Get the type tags for return values.
    pub fn return_values_type_tags(
        &self,
    ) -> Result<Vec<iota_sdk_types::TypeTag>, TryFromProtoError> {
        get_inner_field!(self.return_values, Self::RETURN_VALUES_FIELD, type_tags)
    }

    /// Get the BCS bytes for return values.
    pub fn return_values_bcs(&self) -> Result<Vec<&[u8]>, TryFromProtoError> {
        get_inner_field!(self.return_values, Self::RETURN_VALUES_FIELD, all_bcs)
    }

    /// Get the JSON values for return values.
    pub fn return_values_json(&self) -> Result<Vec<serde_json::Value>, TryFromProtoError> {
        get_inner_field!(self.return_values, Self::RETURN_VALUES_FIELD, all_json)
    }
}

// CommandResults
//

impl CommandResults {
    /// Get all mutated-by-reference arguments across all commands.
    pub fn all_mutated_by_ref_arguments(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::transaction::Argument>>, TryFromProtoError> {
        self.results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                r.mutated_by_ref_arguments()
                    .map_err(|e| e.nested_at(Self::RESULTS_FIELD.name, i))
            })
            .collect()
    }

    /// Get all return value arguments across all commands.
    pub fn all_return_values_arguments(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::transaction::Argument>>, TryFromProtoError> {
        self.results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                r.return_values_arguments()
                    .map_err(|e| e.nested_at(Self::RESULTS_FIELD.name, i))
            })
            .collect()
    }

    /// Get all mutated-by-reference type tags across all commands.
    pub fn all_mutated_by_ref_type_tags(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::TypeTag>>, TryFromProtoError> {
        self.results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                r.mutated_by_ref_type_tags()
                    .map_err(|e| e.nested_at(Self::RESULTS_FIELD.name, i))
            })
            .collect()
    }

    /// Get all return value type tags across all commands.
    pub fn all_return_values_type_tags(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::TypeTag>>, TryFromProtoError> {
        self.results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                r.return_values_type_tags()
                    .map_err(|e| e.nested_at(Self::RESULTS_FIELD.name, i))
            })
            .collect()
    }
}
