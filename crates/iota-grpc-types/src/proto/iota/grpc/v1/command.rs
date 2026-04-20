// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.command.rs");
include!("../../../generated/iota.grpc.v1.command.field_info.rs");

use crate::{
    proto::{GrpcConversionError, TryFromProtoError, get_inner_field},
    v1::bcs::BcsData,
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
    ///
    /// Requires `argument` in the read_mask.
    pub fn argument(&self) -> Result<iota_sdk_types::transaction::Argument, TryFromProtoError> {
        get_inner_field!(self.argument, Self::ARGUMENT_FIELD, argument)
    }

    /// Deserialize the type tag to SDK type.
    ///
    /// Requires `type_tag` in the read_mask.
    pub fn type_tag(&self) -> Result<iota_sdk_types::TypeTag, TryFromProtoError> {
        get_inner_field!(self.type_tag, Self::TYPE_TAG_FIELD, type_tag)
    }

    /// Get the raw BCS bytes.
    ///
    /// Requires `bcs` in the read_mask.
    pub fn output_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_FIELD.name))
    }

    /// Get the JSON value.
    ///
    /// Requires `json` in the read_mask.
    pub fn output_json(&self) -> Result<serde_json::Value, TryFromProtoError> {
        self.json
            .as_ref()
            .map(crate::proto::prost_to_json)
            .ok_or_else(|| TryFromProtoError::missing(Self::JSON_FIELD.name))
    }
}

// CommandResult
//

impl CommandResult {
    /// Get the mutated-by-reference outputs.
    ///
    /// Requires `mutated_by_ref` in the read_mask.
    pub fn mutated_by_ref(&self) -> Result<&CommandOutputs, TryFromProtoError> {
        self.mutated_by_ref
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::MUTATED_BY_REF_FIELD.name))
    }

    /// Get the return value outputs.
    ///
    /// Requires `return_values` in the read_mask.
    pub fn return_values(&self) -> Result<&CommandOutputs, TryFromProtoError> {
        self.return_values
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::RETURN_VALUES_FIELD.name))
    }
}
