// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter, Write},
    str::FromStr,
};

use colored::Colorize;
use iota_macros::EnumVariantOrder;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    error::{IotaError, UserInputError},
    iota_serde::IotaStructTag,
};
use itertools::Itertools;
use move_binary_format::{
    file_format::{Ability, AbilitySet, DatatypeTyParameter, Visibility},
    normalized::{
        self, Enum as NormalizedEnum, Field as NormalizedField, Function as NormalizedFunction,
        Module as NormalizedModule, Struct as NormalizedStruct, Type as NormalizedType,
    },
};
use move_core_types::{
    annotated_value::{MoveStruct, MoveValue, MoveVariant},
    identifier::Identifier,
    language_storage::StructTag,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serde_with::serde_as;
use tracing::warn;

pub type IotaMoveTypeParameterIndex = u16;

#[cfg(test)]
#[path = "unit_tests/iota_move_tests.rs"]
mod iota_move_tests;

#[derive(Serialize, Deserialize, Copy, Clone, Debug, JsonSchema, PartialEq)]
pub enum IotaMoveAbility {
    Copy,
    Drop,
    Store,
    Key,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct IotaMoveAbilitySet {
    pub abilities: Vec<IotaMoveAbility>,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, JsonSchema, PartialEq)]
pub enum IotaMoveVisibility {
    Private,
    Public,
    Friend,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IotaMoveStructTypeParameter {
    pub constraints: IotaMoveAbilitySet,
    pub is_phantom: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct IotaMoveNormalizedField {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: IotaMoveNormalizedType,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IotaMoveNormalizedStruct {
    pub abilities: IotaMoveAbilitySet,
    pub type_parameters: Vec<IotaMoveStructTypeParameter>,
    pub fields: Vec<IotaMoveNormalizedField>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaMoveNormalizedEnum {
    pub abilities: IotaMoveAbilitySet,
    pub type_parameters: Vec<IotaMoveStructTypeParameter>,
    pub variants: BTreeMap<String, Vec<IotaMoveNormalizedField>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq)]
pub enum IotaMoveNormalizedType {
    Bool,
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
    Address,
    Signer,
    Struct {
        #[serde(flatten)]
        inner: Box<IotaMoveNormalizedStructType>,
    },
    Vector(Box<IotaMoveNormalizedType>),
    TypeParameter(IotaMoveTypeParameterIndex),
    Reference(Box<IotaMoveNormalizedType>),
    MutableReference(Box<IotaMoveNormalizedType>),
}

#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IotaMoveNormalizedStructType {
    pub address: String,
    pub module: String,
    pub name: String,
    pub type_arguments: Vec<IotaMoveNormalizedType>,
}
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IotaMoveNormalizedFunction {
    pub visibility: IotaMoveVisibility,
    pub is_entry: bool,
    pub type_parameters: Vec<IotaMoveAbilitySet>,
    pub parameters: Vec<IotaMoveNormalizedType>,
    pub return_: Vec<IotaMoveNormalizedType>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct IotaMoveModuleId {
    address: String,
    name: String,
}

/// Identifies a Move function.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveFunctionName {
    /// The package ID to which the function belongs.
    pub package: ObjectID,
    /// The module name to which the function belongs.
    pub module: String,
    /// The function name.
    pub function: String,
}

impl FromStr for MoveFunctionName {
    type Err = IotaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (module, name) =
            iota_types::parse_iota_fq_name(s).map_err(|e| UserInputError::InvalidIdentifier {
                error: e.to_string(),
            })?;
        let package = ObjectID::from_address(*module.address());
        Ok(Self {
            package,
            module: module.name().to_string(),
            function: name,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaMoveNormalizedModule {
    pub file_format_version: u32,
    pub address: String,
    pub name: String,
    pub friends: Vec<IotaMoveModuleId>,
    pub structs: BTreeMap<String, IotaMoveNormalizedStruct>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub enums: BTreeMap<String, IotaMoveNormalizedEnum>,
    pub exposed_functions: BTreeMap<String, IotaMoveNormalizedFunction>,
}

impl PartialEq for IotaMoveNormalizedModule {
    fn eq(&self, other: &Self) -> bool {
        self.file_format_version == other.file_format_version
            && self.address == other.address
            && self.name == other.name
    }
}

impl<S: std::hash::Hash + Eq + ToString> From<&NormalizedModule<S>> for IotaMoveNormalizedModule {
    fn from(module: &NormalizedModule<S>) -> Self {
        Self {
            file_format_version: module.file_format_version,
            address: module.address().to_hex_literal(),
            name: module.name().to_string(),
            friends: module
                .friends
                .iter()
                .map(|module_id| IotaMoveModuleId {
                    address: module_id.address.to_hex_literal(),
                    name: module_id.name.to_string(),
                })
                .collect::<Vec<IotaMoveModuleId>>(),
            structs: module
                .structs
                .iter()
                .map(|(name, struct_)| {
                    (name.to_string(), IotaMoveNormalizedStruct::from(&**struct_))
                })
                .collect::<BTreeMap<String, IotaMoveNormalizedStruct>>(),
            enums: module
                .enums
                .iter()
                .map(|(name, enum_)| (name.to_string(), IotaMoveNormalizedEnum::from(&**enum_)))
                .collect(),
            exposed_functions: module
                .functions
                .iter()
                .filter(|(_name, function)| {
                    function.is_entry || function.visibility != Visibility::Private
                })
                .map(|(name, function)| {
                    // TODO: Do we want to expose the private functions as well?

                    (
                        name.to_string(),
                        IotaMoveNormalizedFunction::from(&**function),
                    )
                })
                .collect::<BTreeMap<String, IotaMoveNormalizedFunction>>(),
        }
    }
}

impl<S: ToString> From<&NormalizedFunction<S>> for IotaMoveNormalizedFunction {
    fn from(function: &NormalizedFunction<S>) -> Self {
        Self {
            visibility: match function.visibility {
                Visibility::Private => IotaMoveVisibility::Private,
                Visibility::Public => IotaMoveVisibility::Public,
                Visibility::Friend => IotaMoveVisibility::Friend,
            },
            is_entry: function.is_entry,
            type_parameters: function
                .type_parameters
                .iter()
                .copied()
                .map(|a| a.into())
                .collect::<Vec<IotaMoveAbilitySet>>(),
            parameters: function
                .parameters
                .iter()
                .map(|t| IotaMoveNormalizedType::from(&**t))
                .collect::<Vec<IotaMoveNormalizedType>>(),
            return_: function
                .return_
                .iter()
                .map(|t| IotaMoveNormalizedType::from(&**t))
                .collect::<Vec<IotaMoveNormalizedType>>(),
        }
    }
}

impl<S: ToString> From<&NormalizedStruct<S>> for IotaMoveNormalizedStruct {
    fn from(struct_: &NormalizedStruct<S>) -> Self {
        Self {
            abilities: struct_.abilities.into(),
            type_parameters: struct_
                .type_parameters
                .iter()
                .copied()
                .map(IotaMoveStructTypeParameter::from)
                .collect::<Vec<IotaMoveStructTypeParameter>>(),
            fields: struct_
                .fields
                .iter()
                .map(|f| IotaMoveNormalizedField::from(&**f))
                .collect::<Vec<IotaMoveNormalizedField>>(),
        }
    }
}

impl<S: ToString> From<&NormalizedEnum<S>> for IotaMoveNormalizedEnum {
    fn from(value: &NormalizedEnum<S>) -> Self {
        Self {
            abilities: value.abilities.into(),
            type_parameters: value
                .type_parameters
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            variants: value
                .variants
                .iter()
                .map(|variant| {
                    (
                        variant.name.to_string(),
                        variant.fields.iter().map(Into::into).collect(),
                    )
                })
                .collect(),
        }
    }
}

impl From<DatatypeTyParameter> for IotaMoveStructTypeParameter {
    fn from(type_parameter: DatatypeTyParameter) -> Self {
        Self {
            constraints: type_parameter.constraints.into(),
            is_phantom: type_parameter.is_phantom,
        }
    }
}

impl<S: ToString> From<&NormalizedField<S>> for IotaMoveNormalizedField {
    fn from(normalized_field: &NormalizedField<S>) -> Self {
        Self {
            name: normalized_field.name.to_string(),
            type_: IotaMoveNormalizedType::from(&normalized_field.type_),
        }
    }
}

impl<S: ToString> From<&NormalizedType<S>> for IotaMoveNormalizedType {
    fn from(type_: &NormalizedType<S>) -> Self {
        match type_ {
            NormalizedType::Bool => IotaMoveNormalizedType::Bool,
            NormalizedType::U8 => IotaMoveNormalizedType::U8,
            NormalizedType::U16 => IotaMoveNormalizedType::U16,
            NormalizedType::U32 => IotaMoveNormalizedType::U32,
            NormalizedType::U64 => IotaMoveNormalizedType::U64,
            NormalizedType::U128 => IotaMoveNormalizedType::U128,
            NormalizedType::U256 => IotaMoveNormalizedType::U256,
            NormalizedType::Address => IotaMoveNormalizedType::Address,
            NormalizedType::Signer => IotaMoveNormalizedType::Signer,
            NormalizedType::Datatype(dt) => {
                let normalized::Datatype {
                    module,
                    name,
                    type_arguments,
                } = &**dt;
                IotaMoveNormalizedType::new_struct(
                    module.address.to_hex_literal(),
                    module.name.to_string(),
                    name.to_string(),
                    type_arguments
                        .iter()
                        .map(IotaMoveNormalizedType::from)
                        .collect::<Vec<IotaMoveNormalizedType>>(),
                )
            }
            NormalizedType::Vector(v) => {
                IotaMoveNormalizedType::Vector(Box::new(IotaMoveNormalizedType::from(&**v)))
            }
            NormalizedType::TypeParameter(t) => IotaMoveNormalizedType::TypeParameter(*t),
            NormalizedType::Reference(false, r) => {
                IotaMoveNormalizedType::Reference(Box::new(IotaMoveNormalizedType::from(&**r)))
            }
            NormalizedType::Reference(true, mr) => IotaMoveNormalizedType::MutableReference(
                Box::new(IotaMoveNormalizedType::from(&**mr)),
            ),
        }
    }
}

impl From<AbilitySet> for IotaMoveAbilitySet {
    fn from(set: AbilitySet) -> IotaMoveAbilitySet {
        Self {
            abilities: set
                .into_iter()
                .map(|a| match a {
                    Ability::Copy => IotaMoveAbility::Copy,
                    Ability::Drop => IotaMoveAbility::Drop,
                    Ability::Key => IotaMoveAbility::Key,
                    Ability::Store => IotaMoveAbility::Store,
                })
                .collect::<Vec<IotaMoveAbility>>(),
        }
    }
}

impl IotaMoveNormalizedType {
    pub fn new_struct(
        address: String,
        module: String,
        name: String,
        type_arguments: Vec<IotaMoveNormalizedType>,
    ) -> Self {
        IotaMoveNormalizedType::Struct {
            inner: Box::new(IotaMoveNormalizedStructType {
                address,
                module,
                name,
                type_arguments,
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, JsonSchema, PartialEq)]
pub enum ObjectValueKind {
    ByImmutableReference,
    ByMutableReference,
    ByValue,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, JsonSchema, PartialEq)]
pub enum MoveFunctionArgType {
    Pure,
    Object(ObjectValueKind),
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq, EnumVariantOrder)]
#[serde(untagged, rename = "MoveValue")]
pub enum IotaMoveValue {
    // u64 and u128 are converted to String to avoid overflow
    Number(u32),
    Bool(bool),
    Address(IotaAddress),
    Vector(Vec<IotaMoveValue>),
    String(String),
    UID { id: ObjectID },
    Struct(IotaMoveStruct),
    Option(Box<Option<IotaMoveValue>>),
    Variant(IotaMoveVariant),
}

impl IotaMoveValue {
    /// Extract values from MoveValue without type information in json format
    pub fn to_json_value(self) -> Value {
        match self {
            IotaMoveValue::Struct(move_struct) => move_struct.to_json_value(),
            IotaMoveValue::Vector(values) => IotaMoveStruct::Runtime(values).to_json_value(),
            IotaMoveValue::Number(v) => json!(v),
            IotaMoveValue::Bool(v) => json!(v),
            IotaMoveValue::Address(v) => json!(v),
            IotaMoveValue::String(v) => json!(v),
            IotaMoveValue::UID { id } => json!({ "id": id }),
            IotaMoveValue::Option(v) => json!(v),
            IotaMoveValue::Variant(v) => v.to_json_value(),
        }
    }
}

impl Display for IotaMoveValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut writer = String::new();
        match self {
            IotaMoveValue::Number(value) => write!(writer, "{value}")?,
            IotaMoveValue::Bool(value) => write!(writer, "{value}")?,
            IotaMoveValue::Address(value) => write!(writer, "{value}")?,
            IotaMoveValue::String(value) => write!(writer, "{value}")?,
            IotaMoveValue::UID { id } => write!(writer, "{id}")?,
            IotaMoveValue::Struct(value) => write!(writer, "{value}")?,
            IotaMoveValue::Option(value) => write!(writer, "{value:?}")?,
            IotaMoveValue::Vector(vec) => {
                write!(
                    writer,
                    "{}",
                    vec.iter().map(|value| format!("{value}")).join(",\n")
                )?;
            }
            IotaMoveValue::Variant(value) => write!(writer, "{value}")?,
        }
        write!(f, "{}", writer.trim_end_matches('\n'))
    }
}

impl From<MoveValue> for IotaMoveValue {
    fn from(value: MoveValue) -> Self {
        match value {
            MoveValue::U8(value) => IotaMoveValue::Number(value.into()),
            MoveValue::U16(value) => IotaMoveValue::Number(value.into()),
            MoveValue::U32(value) => IotaMoveValue::Number(value),
            MoveValue::U64(value) => IotaMoveValue::String(format!("{value}")),
            MoveValue::U128(value) => IotaMoveValue::String(format!("{value}")),
            MoveValue::U256(value) => IotaMoveValue::String(format!("{value}")),
            MoveValue::Bool(value) => IotaMoveValue::Bool(value),
            MoveValue::Vector(values) => {
                IotaMoveValue::Vector(values.into_iter().map(|value| value.into()).collect())
            }
            MoveValue::Struct(value) => {
                // Best effort IOTA core type conversion
                let MoveStruct { type_, fields } = &value;
                if let Some(value) = try_convert_type(type_, fields) {
                    return value;
                }
                IotaMoveValue::Struct(value.into())
            }
            MoveValue::Signer(value) | MoveValue::Address(value) => {
                IotaMoveValue::Address(IotaAddress::from(ObjectID::from(value)))
            }
            MoveValue::Variant(MoveVariant {
                type_,
                variant_name,
                tag: _,
                fields,
            }) => IotaMoveValue::Variant(IotaMoveVariant {
                type_,
                variant: variant_name.to_string(),
                fields: fields
                    .into_iter()
                    .map(|(id, value)| (id.into_string(), value.into()))
                    .collect::<BTreeMap<_, _>>(),
            }),
        }
    }
}

fn to_bytearray(value: &[MoveValue]) -> Option<Vec<u8>> {
    if value.iter().all(|value| matches!(value, MoveValue::U8(_))) {
        let bytearray = value
            .iter()
            .flat_map(|value| {
                if let MoveValue::U8(u8) = value {
                    Some(*u8)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        Some(bytearray)
    } else {
        None
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(rename = "MoveVariant")]
pub struct IotaMoveVariant {
    #[schemars(with = "String")]
    #[serde(rename = "type")]
    #[serde_as(as = "IotaStructTag")]
    pub type_: StructTag,
    pub variant: String,
    pub fields: BTreeMap<String, IotaMoveValue>,
}

impl IotaMoveVariant {
    pub fn to_json_value(self) -> Value {
        // We only care about values here, assuming type information is known at the
        // client side.
        let fields = self
            .fields
            .into_iter()
            .map(|(key, value)| (key, value.to_json_value()))
            .collect::<BTreeMap<_, _>>();
        json!({
            "variant": self.variant,
            "fields": fields,
        })
    }
}

impl Display for IotaMoveVariant {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut writer = String::new();
        let IotaMoveVariant {
            type_,
            variant,
            fields,
        } = self;
        writeln!(writer)?;
        writeln!(writer, "  {}: {type_}", "type".bold().bright_black())?;
        writeln!(writer, "  {}: {variant}", "variant".bold().bright_black())?;
        for (name, value) in fields {
            let value = format!("{value}");
            let value = if value.starts_with('\n') {
                indent(&value, 2)
            } else {
                value
            };
            writeln!(writer, "  {}: {value}", name.bold().bright_black())?;
        }

        write!(f, "{}", writer.trim_end_matches('\n'))
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq, EnumVariantOrder)]
#[serde(untagged, rename = "MoveStruct")]
pub enum IotaMoveStruct {
    Runtime(Vec<IotaMoveValue>),
    WithTypes {
        #[schemars(with = "String")]
        #[serde(rename = "type")]
        #[serde_as(as = "IotaStructTag")]
        type_: StructTag,
        fields: BTreeMap<String, IotaMoveValue>,
    },
    WithFields(BTreeMap<String, IotaMoveValue>),
}

impl IotaMoveStruct {
    /// Extract values from MoveStruct without type information in json format
    pub fn to_json_value(self) -> Value {
        // Unwrap MoveStructs
        match self {
            IotaMoveStruct::Runtime(values) => {
                let values = values
                    .into_iter()
                    .map(|value| value.to_json_value())
                    .collect::<Vec<_>>();
                json!(values)
            }
            // We only care about values here, assuming struct type information is known at the
            // client side.
            IotaMoveStruct::WithTypes { type_: _, fields } | IotaMoveStruct::WithFields(fields) => {
                let fields = fields
                    .into_iter()
                    .map(|(key, value)| (key, value.to_json_value()))
                    .collect::<BTreeMap<_, _>>();
                json!(fields)
            }
        }
    }

    pub fn read_dynamic_field_value(&self, field_name: &str) -> Option<IotaMoveValue> {
        match self {
            IotaMoveStruct::WithFields(fields) => fields.get(field_name).cloned(),
            IotaMoveStruct::WithTypes { type_: _, fields } => fields.get(field_name).cloned(),
            _ => None,
        }
    }
}

impl Display for IotaMoveStruct {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut writer = String::new();
        match self {
            IotaMoveStruct::Runtime(_) => {}
            IotaMoveStruct::WithFields(fields) => {
                for (name, value) in fields {
                    writeln!(writer, "{}: {value}", name.bold().bright_black())?;
                }
            }
            IotaMoveStruct::WithTypes { type_, fields } => {
                writeln!(writer)?;
                writeln!(writer, "  {}: {type_}", "type".bold().bright_black())?;
                for (name, value) in fields {
                    let value = format!("{value}");
                    let value = if value.starts_with('\n') {
                        indent(&value, 2)
                    } else {
                        value
                    };
                    writeln!(writer, "  {}: {value}", name.bold().bright_black())?;
                }
            }
        }
        write!(f, "{}", writer.trim_end_matches('\n'))
    }
}

fn indent<T: Display>(d: &T, indent: usize) -> String {
    d.to_string()
        .lines()
        .map(|line| format!("{:indent$}{line}", ""))
        .join("\n")
}

fn try_convert_type(
    type_: &StructTag,
    fields: &[(Identifier, MoveValue)],
) -> Option<IotaMoveValue> {
    let struct_name = format!(
        "0x{}::{}::{}",
        type_.address.short_str_lossless(),
        type_.module,
        type_.name
    );
    let mut values = fields
        .iter()
        .map(|(id, value)| (id.to_string(), value))
        .collect::<BTreeMap<_, _>>();
    match struct_name.as_str() {
        "0x1::string::String" | "0x1::ascii::String" => {
            if let Some(MoveValue::Vector(bytes)) = values.remove("bytes") {
                return to_bytearray(bytes)
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .map(IotaMoveValue::String);
            }
        }
        "0x2::url::Url" => {
            return values.remove("url").cloned().map(IotaMoveValue::from);
        }
        "0x2::object::ID" => {
            return values.remove("bytes").cloned().map(IotaMoveValue::from);
        }
        "0x2::object::UID" => {
            let id = values.remove("id").cloned().map(IotaMoveValue::from);
            if let Some(IotaMoveValue::Address(address)) = id {
                return Some(IotaMoveValue::UID {
                    id: ObjectID::from(address),
                });
            }
        }
        "0x2::balance::Balance" => {
            return values.remove("value").cloned().map(IotaMoveValue::from);
        }
        "0x1::option::Option" => {
            if let Some(MoveValue::Vector(values)) = values.remove("vec") {
                return Some(IotaMoveValue::Option(Box::new(
                    // in Move option is modeled as vec of 1 element
                    values.first().cloned().map(IotaMoveValue::from),
                )));
            }
        }
        _ => return None,
    }
    warn!(
        fields =? fields,
        "failed to convert {struct_name} to IotaMoveValue"
    );
    None
}

impl From<MoveStruct> for IotaMoveStruct {
    fn from(move_struct: MoveStruct) -> Self {
        IotaMoveStruct::WithTypes {
            type_: move_struct.type_,
            fields: move_struct
                .fields
                .into_iter()
                .map(|(id, value)| (id.into_string(), value.into()))
                .collect(),
        }
    }
}

#[test]
fn enum_size() {
    assert_eq!(std::mem::size_of::<IotaMoveNormalizedType>(), 16);
}
