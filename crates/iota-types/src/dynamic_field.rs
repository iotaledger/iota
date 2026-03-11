// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt,
    fmt::{Display, Formatter},
};

use fastcrypto::{encoding::Base64, hash::HashFunction};
use iota_sdk_types::crypto::HashingIntentScope;
use move_core_types::{
    annotated_value::{MoveStruct, MoveValue, MoveVariant},
    ident_str,
    identifier::{IdentStr, Identifier},
    language_storage::{StructTag, TypeTag},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use serde_with::{DisplayFromStr, serde_as};

use crate::{
    IOTA_FRAMEWORK_ADDRESS, MoveTypeTagTrait, ObjectID, SequenceNumber,
    base_types::{IotaAddress, ObjectDigest},
    crypto::DefaultHash,
    error::{IotaError, IotaResult},
    id::UID,
    iota_serde::{IotaTypeTag, Readable},
    object::Object,
    storage::ObjectStore,
};

pub mod visitor;

const DYNAMIC_FIELD_MODULE_NAME: &IdentStr = ident_str!("dynamic_field");
const DYNAMIC_FIELD_FIELD_STRUCT_NAME: &IdentStr = ident_str!("Field");

const DYNAMIC_OBJECT_FIELD_MODULE_NAME: &IdentStr = ident_str!("dynamic_object_field");
const DYNAMIC_OBJECT_FIELD_WRAPPER_STRUCT_NAME: &IdentStr = ident_str!("Wrapper");

/// Rust version of the Move iota::dynamic_field::Field type
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Field<N, V> {
    pub id: UID,
    pub name: N,
    pub value: V,
}

/// Rust version of the Move iota::dynamic_object_field::Wrapper type
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct DOFWrapper<N> {
    pub name: N,
}

impl<N> MoveTypeTagTrait for DOFWrapper<N>
where
    N: MoveTypeTagTrait,
{
    fn get_type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(DynamicFieldInfo::dynamic_object_field_wrapper(
            N::get_type_tag(),
        )))
    }
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DynamicFieldInfo {
    pub name: DynamicFieldName,
    #[serde_as(as = "Readable<Base64, _>")]
    pub bcs_name: Vec<u8>,
    pub type_: DynamicFieldType,
    pub object_type: String,
    pub object_id: ObjectID,
    pub version: SequenceNumber,
    pub digest: ObjectDigest,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DynamicFieldName {
    #[schemars(with = "String")]
    #[serde_as(as = "Readable<IotaTypeTag, _>")]
    pub type_: TypeTag,
    // Bincode does not like serde_json::Value, rocksdb will not insert the value without
    // serializing value as string. TODO: investigate if this can be removed after switch to
    // BCS.
    #[schemars(with = "Value")]
    #[serde_as(as = "Readable<_, DisplayFromStr>")]
    pub value: Value,
}

impl Display for DynamicFieldName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.type_, self.value)
    }
}

#[derive(
    Copy, Clone, Serialize, Deserialize, JsonSchema, Ord, PartialOrd, Eq, PartialEq, Debug,
)]
pub enum DynamicFieldType {
    #[serde(rename_all = "camelCase")]
    DynamicField,
    DynamicObject,
}

impl Display for DynamicFieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DynamicFieldType::DynamicField => write!(f, "DynamicField"),
            DynamicFieldType::DynamicObject => write!(f, "DynamicObject"),
        }
    }
}

impl DynamicFieldInfo {
    pub fn is_dynamic_field(tag: &StructTag) -> bool {
        tag.address == IOTA_FRAMEWORK_ADDRESS
            && tag.module.as_ident_str() == DYNAMIC_FIELD_MODULE_NAME
            && tag.name.as_ident_str() == DYNAMIC_FIELD_FIELD_STRUCT_NAME
    }

    pub fn is_dynamic_object_field_wrapper(tag: &StructTag) -> bool {
        tag.address == IOTA_FRAMEWORK_ADDRESS
            && tag.module.as_ident_str() == DYNAMIC_OBJECT_FIELD_MODULE_NAME
            && tag.name.as_ident_str() == DYNAMIC_OBJECT_FIELD_WRAPPER_STRUCT_NAME
    }

    pub fn dynamic_field_type(key: TypeTag, value: TypeTag) -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            name: DYNAMIC_FIELD_FIELD_STRUCT_NAME.to_owned(),
            module: DYNAMIC_FIELD_MODULE_NAME.to_owned(),
            type_params: vec![key, value],
        }
    }

    pub fn dynamic_object_field_wrapper(key: TypeTag) -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: DYNAMIC_OBJECT_FIELD_MODULE_NAME.to_owned(),
            name: DYNAMIC_OBJECT_FIELD_WRAPPER_STRUCT_NAME.to_owned(),
            type_params: vec![key],
        }
    }

    pub fn try_extract_field_name(
        tag: &StructTag,
        type_: &DynamicFieldType,
    ) -> IotaResult<TypeTag> {
        match (type_, tag.type_params.first()) {
            (DynamicFieldType::DynamicField, Some(name_type)) => Ok(name_type.clone()),
            (DynamicFieldType::DynamicObject, Some(TypeTag::Struct(s))) => Ok(s
                .type_params
                .first()
                .ok_or_else(|| IotaError::ObjectDeserialization {
                    error: format!("Error extracting dynamic object name from object: {tag}"),
                })?
                .clone()),
            _ => Err(IotaError::ObjectDeserialization {
                error: format!("Error extracting dynamic object name from object: {tag}"),
            }),
        }
    }

    pub fn try_extract_field_value(tag: &StructTag) -> IotaResult<TypeTag> {
        match tag.type_params.last() {
            Some(value_type) => Ok(value_type.clone()),
            None => Err(IotaError::ObjectDeserialization {
                error: format!("Error extracting dynamic object value from object: {tag}"),
            }),
        }
    }

    pub fn parse_move_object(
        move_struct: &MoveStruct,
    ) -> IotaResult<(MoveValue, DynamicFieldType, ObjectID)> {
        let name = extract_field_from_move_struct(move_struct, "name").ok_or_else(|| {
            IotaError::ObjectDeserialization {
                error: "Cannot extract [name] field from iota::dynamic_field::Field".to_string(),
            }
        })?;

        let value = extract_field_from_move_struct(move_struct, "value").ok_or_else(|| {
            IotaError::ObjectDeserialization {
                error: "Cannot extract [value] field from iota::dynamic_field::Field".to_string(),
            }
        })?;

        Ok(if is_dynamic_object(move_struct) {
            let name = match name {
                MoveValue::Struct(name_struct) => {
                    extract_field_from_move_struct(name_struct, "name")
                }
                _ => None,
            }
            .ok_or_else(|| IotaError::ObjectDeserialization {
                error: "Cannot extract [name] field from iota::dynamic_object_field::Wrapper."
                    .to_string(),
            })?;
            // ID extracted from the wrapper object
            let object_id =
                extract_id_value(value).ok_or_else(|| IotaError::ObjectDeserialization {
                    error: format!(
                        "Cannot extract dynamic object's object id from \
                        iota::dynamic_field::Field, {value:?}"
                    ),
                })?;
            (name.clone(), DynamicFieldType::DynamicObject, object_id)
        } else {
            // ID of the Field object
            let object_id =
                extract_object_id(move_struct).ok_or_else(|| IotaError::ObjectDeserialization {
                    error: format!(
                        "Cannot extract dynamic object's object id from \
                        iota::dynamic_field::Field, {move_struct:?}",
                    ),
                })?;
            (name.clone(), DynamicFieldType::DynamicField, object_id)
        })
    }
}

pub fn extract_field_from_move_struct<'a>(
    move_struct: &'a MoveStruct,
    field_name: &str,
) -> Option<&'a MoveValue> {
    move_struct.fields.iter().find_map(|(id, value)| {
        if id.to_string() == field_name {
            Some(value)
        } else {
            None
        }
    })
}

fn extract_object_id(value: &MoveStruct) -> Option<ObjectID> {
    // id:UID is the first value in an object
    let uid_value = &value.fields.first()?.1;

    // id is the first value in UID
    let id_value = match uid_value {
        MoveValue::Struct(MoveStruct { fields, .. }) => &fields.first()?.1,
        _ => return None,
    };
    extract_id_value(id_value)
}

pub fn extract_id_value(id_value: &MoveValue) -> Option<ObjectID> {
    // the id struct has a single bytes field
    let id_bytes_value = match id_value {
        MoveValue::Struct(MoveStruct { fields, .. }) => &fields.first()?.1,
        _ => return None,
    };
    // the bytes field should be an address
    match id_bytes_value {
        MoveValue::Address(addr) => Some(ObjectID::from(*addr)),
        _ => None,
    }
}

pub fn is_dynamic_object(move_struct: &MoveStruct) -> bool {
    matches!(
        &move_struct.type_.type_params[0],
        TypeTag::Struct(tag) if DynamicFieldInfo::is_dynamic_object_field_wrapper(tag)
    )
}

pub fn derive_dynamic_field_id<T>(
    parent: T,
    key_type_tag: &TypeTag,
    key_bytes: &[u8],
) -> Result<ObjectID, bcs::Error>
where
    T: Into<IotaAddress>,
{
    let parent: IotaAddress = parent.into();
    let k_tag_bytes = bcs::to_bytes(key_type_tag)?;
    tracing::trace!(
        "Deriving dynamic field ID for parent={:?}, key={:?}, key_type_tag={:?}",
        parent,
        key_bytes,
        key_type_tag,
    );

    // hash(parent || len(key) || key || key_type_tag)
    let mut hasher = DefaultHash::default();
    hasher.update([HashingIntentScope::ChildObjectId as u8]);
    hasher.update(parent);
    hasher.update(key_bytes.len().to_le_bytes());
    hasher.update(key_bytes);
    hasher.update(k_tag_bytes);
    let hash = hasher.finalize();

    // truncate into an ObjectID and return
    // OK to access slice because digest should never be shorter than
    // ObjectID::LENGTH.
    let id = ObjectID::try_from(&hash.as_ref()[0..ObjectID::LENGTH]).unwrap();
    tracing::trace!("derive_dynamic_field_id result: {:?}", id);
    Ok(id)
}

/// Given a parent object ID (e.g. a table), and a `key`, retrieve the
/// corresponding dynamic field object from the `object_store`. The key type `K`
/// must implement `MoveTypeTagTrait` which has an associated function that
/// returns the Move type tag. Note that this function returns the Field object
/// itself, not the value in the field.
pub fn get_dynamic_field_object_from_store<K>(
    object_store: &dyn ObjectStore,
    parent_id: ObjectID,
    key: &K,
) -> Result<Object, IotaError>
where
    K: MoveTypeTagTrait + Serialize + DeserializeOwned + fmt::Debug,
{
    let id = derive_dynamic_field_id(parent_id, &K::get_type_tag(), &bcs::to_bytes(key).unwrap())
        .map_err(|err| IotaError::DynamicFieldRead(err.to_string()))?;
    let object = object_store.try_get_object(&id)?.ok_or_else(|| {
        IotaError::DynamicFieldRead(format!(
            "Dynamic field with key={key:?} and ID={id:?} not found on parent {parent_id:?}"
        ))
    })?;
    Ok(object)
}

/// Similar to `get_dynamic_field_object_from_store`, but returns the value in
/// the field instead of the Field object itself.
pub fn get_dynamic_field_from_store<K, V>(
    object_store: &dyn ObjectStore,
    parent_id: ObjectID,
    key: &K,
) -> Result<V, IotaError>
where
    K: MoveTypeTagTrait + Serialize + DeserializeOwned + fmt::Debug,
    V: Serialize + DeserializeOwned,
{
    let object = get_dynamic_field_object_from_store(object_store, parent_id, key)?;
    let move_object = object.data.try_as_move().ok_or_else(|| {
        IotaError::DynamicFieldRead(format!(
            "Dynamic field {:?} is not a Move object",
            object.id()
        ))
    })?;
    Ok(bcs::from_bytes::<Field<K, V>>(move_object.contents())
        .map_err(|err| IotaError::DynamicFieldRead(err.to_string()))?
        .value)
}

/// Convert an annotated `MoveValue` directly to a `serde_json::Value`.
///
/// This replicates the combined `IotaMoveValue::from(val).to_json_value()`
/// pipeline from `iota-json-rpc-types` so that `iota-core` can produce JSON
/// representations of dynamic field names and event payloads without depending
/// on JSON-RPC types.
///
/// # Keeping in sync
///
/// The `iota-json-rpc-types` crate has an equivalent conversion split across
/// two functions:
///   - `IotaMoveValue::from(MoveValue)` in `iota_move.rs`
///   - `IotaMoveValue::to_json_value()`  in `iota_move.rs`
///
/// Both implementations MUST produce identical JSON for any given `MoveValue`.
/// The well-known type handlers (`try_convert_struct_to_json` here vs the
/// match arms in `IotaMoveValue::from`) must handle the same set of types:
///   `0x1::string::String`, `0x1::ascii::String`, `0x2::url::Url`,
///   `0x2::object::ID`, `0x2::object::UID`, `0x2::balance::Balance`,
///   `0x1::option::Option`.
///
/// TODO: Consider unifying these implementations so there is a single source
/// of truth. The `IotaMoveValue` intermediate type serves the JSON-RPC schema,
/// but the JSON output should ideally be derived from one shared function.
pub fn move_value_to_json(value: MoveValue) -> Value {
    match value {
        MoveValue::U8(v) => json!(u32::from(v)),
        MoveValue::U16(v) => json!(u32::from(v)),
        MoveValue::U32(v) => json!(v),
        MoveValue::U64(v) => json!(format!("{v}")),
        MoveValue::U128(v) => json!(format!("{v}")),
        MoveValue::U256(v) => json!(format!("{v}")),
        MoveValue::Bool(v) => json!(v),
        MoveValue::Vector(values) => {
            let arr: Vec<Value> = values.into_iter().map(move_value_to_json).collect();
            json!(arr)
        }
        MoveValue::Struct(value) => {
            if let Some(v) = try_convert_struct_to_json(&value.type_, &value.fields) {
                return v;
            }
            move_struct_to_json(value)
        }
        MoveValue::Signer(addr) | MoveValue::Address(addr) => {
            json!(IotaAddress::from(ObjectID::from(addr)))
        }
        MoveValue::Variant(MoveVariant {
            type_: _,
            variant_name,
            tag: _,
            fields,
        }) => {
            let fields_map: std::collections::BTreeMap<String, Value> = fields
                .into_iter()
                .map(|(id, v)| (id.into_string(), move_value_to_json(v)))
                .collect();
            json!({
                "variant": variant_name.into_string(),
                "fields": fields_map,
            })
        }
    }
}

fn move_struct_to_json(s: MoveStruct) -> Value {
    let fields_map: std::collections::BTreeMap<String, Value> = s
        .fields
        .into_iter()
        .map(|(id, v)| (id.into_string(), move_value_to_json(v)))
        .collect();
    json!(fields_map)
}

/// Attempt to convert well-known IOTA/Move framework types to a simpler JSON
/// representation (strings, unwrapped scalars, etc.).
fn try_convert_struct_to_json(
    type_: &StructTag,
    fields: &[(Identifier, MoveValue)],
) -> Option<Value> {
    let struct_name = format!(
        "0x{}::{}::{}",
        type_.address.short_str_lossless(),
        type_.module,
        type_.name
    );
    let mut values: std::collections::BTreeMap<String, &MoveValue> = fields
        .iter()
        .map(|(id, value)| (id.to_string(), value))
        .collect();
    match struct_name.as_str() {
        "0x1::string::String" | "0x1::ascii::String" => {
            if let Some(MoveValue::Vector(bytes)) = values.remove("bytes") {
                return to_bytearray_json(bytes)
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .map(|s| json!(s));
            }
        }
        "0x2::url::Url" => {
            return values.remove("url").cloned().map(move_value_to_json);
        }
        "0x2::object::ID" => {
            return values.remove("bytes").cloned().map(move_value_to_json);
        }
        "0x2::object::UID" => {
            if let Some(id_value) = values.remove("id") {
                if let Some(object_id) = extract_id_value(id_value) {
                    return Some(json!({ "id": object_id }));
                }
            }
        }
        "0x2::balance::Balance" => {
            return values.remove("value").cloned().map(move_value_to_json);
        }
        "0x1::option::Option" => {
            if let Some(MoveValue::Vector(values)) = values.remove("vec") {
                let opt = values.first().cloned().map(move_value_to_json);
                return Some(json!(opt));
            }
        }
        _ => return None,
    }
    tracing::debug!(
        struct_name,
        "failed to convert well-known type to simplified JSON"
    );
    None
}

fn to_bytearray_json(value: &[MoveValue]) -> Option<Vec<u8>> {
    if value.iter().all(|v| matches!(v, MoveValue::U8(_))) {
        Some(
            value
                .iter()
                .filter_map(|v| {
                    if let MoveValue::U8(u) = v {
                        Some(*u)
                    } else {
                        None
                    }
                })
                .collect(),
        )
    } else {
        None
    }
}
