// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use iota_json_rpc_types::{
    DisplayFieldsResponse, IotaMoveStruct, IotaMoveValue, IotaMoveVariant, IotaObjectResponseError,
};
use iota_sdk_types::StructTag;
use iota_types::{collection_types::VecMap, error::IotaError, object::MoveObjectExt};
use move_core_types::annotated_value::{MoveStruct, MoveStructLayout, MoveValue};

use crate::error::Error;

const MAX_DISPLAY_NESTED_LEVEL: usize = 10;

#[derive(Debug, thiserror::Error)]
pub enum ObjectDisplayError {
    #[error("Not a move struct")]
    NotMoveStruct,

    #[error("Failed to extract layout")]
    Layout,

    #[error("Failed to extract Move object")]
    MoveObject,

    #[error(transparent)]
    Deserialization(#[from] IotaError),
}

pub fn get_object_type_and_struct(
    o: &iota_types::object::Object,
    layout: &Option<MoveStructLayout>,
) -> Result<Option<(StructTag, MoveStruct)>, ObjectDisplayError> {
    if let Some(object_type) = o.type_() {
        let move_struct = get_move_struct(o, layout)?;
        Ok(Some((object_type.clone().into(), move_struct)))
    } else {
        Ok(None)
    }
}

fn get_move_struct(
    o: &iota_types::object::Object,
    layout: &Option<MoveStructLayout>,
) -> Result<MoveStruct, ObjectDisplayError> {
    let layout = layout.as_ref().ok_or(ObjectDisplayError::Layout)?;
    Ok(o.data
        .as_opt_struct()
        .ok_or(ObjectDisplayError::MoveObject)?
        .to_move_struct(layout)?)
}

pub fn get_rendered_fields(
    fields: VecMap<String, String>,
    move_struct: &MoveStruct,
) -> Result<DisplayFieldsResponse, ObjectDisplayError> {
    let iota_move_value: IotaMoveValue = MoveValue::Struct(move_struct.clone()).into();
    if let IotaMoveValue::Struct(move_struct) = iota_move_value {
        let fields =
            fields
                .contents
                .iter()
                .map(|entry| match parse_template(&entry.value, &move_struct) {
                    Ok(value) => Ok((entry.key.clone(), value)),
                    Err(e) => Err(e),
                });
        let (oks, errs): (Vec<_>, Vec<_>) = fields.partition(Result::is_ok);
        let success = oks.into_iter().filter_map(Result::ok).collect();
        let errors: Vec<_> = errs.into_iter().filter_map(Result::err).collect();
        let error_string = errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join("; ");
        let error = if !error_string.is_empty() {
            Some(IotaObjectResponseError::Display {
                error: anyhow!("{error_string}").to_string(),
            })
        } else {
            None
        };

        return Ok(DisplayFieldsResponse {
            data: Some(success),
            error,
        });
    }
    Err(ObjectDisplayError::NotMoveStruct)?
}

fn parse_template(template: &str, move_struct: &IotaMoveStruct) -> Result<String, Error> {
    let mut output = template.to_string();
    let mut var_name = String::new();
    let mut in_braces = false;
    let mut escaped = false;

    for ch in template.chars() {
        match ch {
            '\\' => {
                escaped = true;
                continue;
            }
            '{' if !escaped => {
                in_braces = true;
                var_name.clear();
            }
            '}' if !escaped => {
                in_braces = false;
                let value = get_value_from_move_struct(move_struct, &var_name)?;
                output = output.replace(&format!("{{{var_name}}}"), &value.to_string());
            }
            _ if !escaped && in_braces => {
                var_name.push(ch);
            }
            _ => {}
        }
        escaped = false;
    }

    Ok(output.replace('\\', ""))
}

fn get_value_from_move_struct(
    move_struct: &IotaMoveStruct,
    var_name: &str,
) -> Result<String, Error> {
    let parts: Vec<&str> = var_name.split('.').collect();
    if parts.is_empty() {
        Err(anyhow!("Display template value cannot be empty"))?;
    }
    if parts.len() > MAX_DISPLAY_NESTED_LEVEL {
        Err(anyhow!(
            "Display template value nested depth cannot exist {MAX_DISPLAY_NESTED_LEVEL}"
        ))?;
    }
    let mut current_value = &IotaMoveValue::Struct(move_struct.clone());
    // iterate over the parts and try to access the corresponding field
    for part in parts {
        match current_value {
            IotaMoveValue::Struct(move_struct) => {
                if let IotaMoveStruct::WithTypes { type_: _, fields }
                | IotaMoveStruct::WithFields(fields) = move_struct
                {
                    if let Some(value) = fields.get(part) {
                        current_value = value;
                    } else {
                        Err(anyhow!("Field value {var_name} cannot be found in struct"))?;
                    }
                } else {
                    Err(Error::Unexpected(format!(
                        "Unexpected move struct type for field {var_name}"
                    )))?;
                }
            }
            IotaMoveValue::Variant(IotaMoveVariant {
                fields, variant, ..
            }) => {
                if let Some(value) = fields.get(part) {
                    current_value = value;
                } else {
                    Err(anyhow!(
                        "Field value {var_name} cannot be found in variant {variant}",
                    ))?
                }
            }
            _ => {
                Err(Error::Unexpected(format!(
                    "Unexpected move value type for field {var_name}"
                )))?;
            }
        }
    }

    match current_value {
        IotaMoveValue::Option(move_option) => match move_option.as_ref() {
            Some(move_value) => Ok(move_value.to_string()),
            None => Ok("".to_string()),
        },
        IotaMoveValue::Vector(_) => Err(anyhow!(
            "Vector is not supported as a Display value {var_name}"
        ))?,

        _ => Ok(current_value.to_string()),
    }
}
