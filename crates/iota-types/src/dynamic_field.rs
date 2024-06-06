// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt,
    fmt::{Display, Formatter},
};

use fastcrypto::{encoding::Base58, hash::HashFunction};
use move_core_types::{
    annotated_value::{MoveStruct, MoveValue},
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use serde_with::{serde_as, DisplayFromStr};
use shared_crypto::intent::HashingIntentScope;

use crate::{
    base_types::{IotaAddress, ObjectDigest},
    crypto::DefaultHash,
    error::{IotaError, IotaResult},
    id::UID,
    iota_serde::{IotaTypeTag, Readable},
    object::Object,
    storage::ObjectStore,
    MoveTypeTagTrait, ObjectID, SequenceNumber, IOTA_FRAMEWORK_ADDRESS,
};

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

#[serde_as]
#[derive(Clone, Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DynamicFieldInfo {
    pub name: DynamicFieldName,
    #[schemars(with = "Base58")]
    #[serde_as(as = "Readable<Base58, _>")]
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

#[derive(Clone, Serialize, Deserialize, JsonSchema, Ord, PartialOrd, Eq, PartialEq, Debug)]
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
                .ok_or_else(|| IotaError::ObjectDeserializationError {
                    error: format!("Error extracting dynamic object name from object: {tag}"),
                })?
                .clone()),
            _ => Err(IotaError::ObjectDeserializationError {
                error: format!("Error extracting dynamic object name from object: {tag}"),
            }),
        }
    }

    pub fn try_extract_field_value(tag: &StructTag) -> IotaResult<TypeTag> {
        match tag.type_params.last() {
            Some(value_type) => Ok(value_type.clone()),
            None => Err(IotaError::ObjectDeserializationError {
                error: format!("Error extracting dynamic object value from object: {tag}"),
            }),
        }
    }

    pub fn parse_move_object(
        move_struct: &MoveStruct,
    ) -> IotaResult<(MoveValue, DynamicFieldType, ObjectID)> {
        let name = extract_field_from_move_struct(move_struct, "name").ok_or_else(|| {
            IotaError::ObjectDeserializationError {
                error: "Cannot extract [name] field from iota::dynamic_field::Field".to_string(),
            }
        })?;

        let value = extract_field_from_move_struct(move_struct, "value").ok_or_else(|| {
            IotaError::ObjectDeserializationError {
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
            .ok_or_else(|| IotaError::ObjectDeserializationError {
                error: "Cannot extract [name] field from iota::dynamic_object_field::Wrapper."
                    .to_string(),
            })?;
            // ID extracted from the wrapper object
            let object_id =
                extract_id_value(value).ok_or_else(|| IotaError::ObjectDeserializationError {
                    error: format!(
                        "Cannot extract dynamic object's object id from \
                        iota::dynamic_field::Field, {value:?}"
                    ),
                })?;
            (name.clone(), DynamicFieldType::DynamicObject, object_id)
        } else {
            // ID of the Field object
            let object_id = extract_object_id(move_struct).ok_or_else(|| {
                IotaError::ObjectDeserializationError {
                    error: format!(
                        "Cannot extract dynamic object's object id from \
                        iota::dynamic_field::Field, {move_struct:?}",
                    ),
                }
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

fn extract_id_value(id_value: &MoveValue) -> Option<ObjectID> {
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
        .map_err(|err| IotaError::DynamicFieldReadError(err.to_string()))?;
    let object = object_store.get_object(&id)?.ok_or_else(|| {
        IotaError::DynamicFieldReadError(format!(
            "Dynamic field with key={:?} and ID={:?} not found on parent {:?}",
            key, id, parent_id
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
        IotaError::DynamicFieldReadError(format!(
            "Dynamic field {:?} is not a Move object",
            object.id()
        ))
    })?;
    Ok(bcs::from_bytes::<Field<K, V>>(move_object.contents())
        .map_err(|err| IotaError::DynamicFieldReadError(err.to_string()))?
        .value)
}
