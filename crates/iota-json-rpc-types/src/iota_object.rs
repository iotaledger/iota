// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fmt,
    fmt::{Display, Formatter, Write},
};

use anyhow::{anyhow, bail};
use colored::Colorize;
use fastcrypto::encoding::Base64;
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    base_types::{
        Identifier, IotaAddress, ObjectDigest, ObjectID, ObjectInfo, ObjectRef, ObjectType,
        SequenceNumber, StructTag, TransactionDigest,
    },
    error::{
        ExecutionError, IotaError, IotaObjectResponseError, IotaResult, UserInputError,
        UserInputResult,
    },
    gas_coin::GasCoin,
    messages_checkpoint::CheckpointSequenceNumber,
    move_package::{MovePackage, TypeOrigin, UpgradeInfo},
    object::{Data, MoveObject, MoveObjectExt, Object, ObjectInner, ObjectRead, Owner},
};
use move_bytecode_utils::module_cache::GetModule;
use move_core_types::annotated_value::{MoveStructLayout, MoveValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::{DeserializeAs, DisplayFromStr, SerializeAs, serde_as};

use crate::{
    IotaMoveStruct, IotaMoveValue, IotaObjectResponseError as IotaObjectResponseErrorSchema, Page,
    iota_owner::OwnerSchema,
    iota_primitives::{
        Base58 as Base58Schema, Base64 as Base64Schema, Identifier as IdentifierSchema,
        IotaAddress as IotaAddressSchema, ObjectID as ObjectIDSchema,
        SequenceNumberString as SequenceNumberStringSchema,
        SequenceNumberU64 as SequenceNumberU64Schema, StructTag as StructTagSchema,
    },
};

#[serde_as]
#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, PartialEq, Eq)]
pub struct IotaObjectResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<IotaObjectData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IotaObjectResponseErrorSchema>")]
    #[serde_as(as = "Option<IotaObjectResponseErrorSchema>")]
    pub error: Option<IotaObjectResponseError>,
}

impl IotaObjectResponse {
    pub fn new(data: Option<IotaObjectData>, error: Option<IotaObjectResponseError>) -> Self {
        Self { data, error }
    }

    pub fn new_with_data(data: IotaObjectData) -> Self {
        Self {
            data: Some(data),
            error: None,
        }
    }

    pub fn new_with_error(error: IotaObjectResponseError) -> Self {
        Self {
            data: None,
            error: Some(error),
        }
    }

    pub fn try_from_object_read_and_options(
        object_read: ObjectRead,
        options: &IotaObjectDataOptions,
    ) -> anyhow::Result<Self> {
        match object_read {
            ObjectRead::NotExists(id) => Ok(IotaObjectResponse::new_with_error(
                IotaObjectResponseError::NotExists { object_id: id },
            )),
            ObjectRead::Exists(object_ref, o, layout) => Ok(IotaObjectResponse::new_with_data(
                IotaObjectData::new(object_ref, o, layout, options, None)?,
            )),
            ObjectRead::Deleted(object_ref) => Ok(IotaObjectResponse::new_with_error(
                IotaObjectResponseError::Deleted {
                    object_id: object_ref.object_id,
                    version: object_ref.version,
                    digest: object_ref.digest,
                },
            )),
        }
    }
}

impl Ord for IotaObjectResponse {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.data, &other.data) {
            (Some(data), Some(data_2)) => {
                if data.object_id.cmp(&data_2.object_id).eq(&Ordering::Greater) {
                    return Ordering::Greater;
                } else if data.object_id.cmp(&data_2.object_id).eq(&Ordering::Less) {
                    return Ordering::Less;
                }
                Ordering::Equal
            }
            // In this ordering those with data will come before IotaObjectResponses that are
            // errors.
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            // IotaObjectResponses that are errors are just considered equal.
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for IotaObjectResponse {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl IotaObjectResponse {
    pub fn move_object_bcs(&self) -> Option<&Vec<u8>> {
        match &self.data {
            Some(IotaObjectData {
                bcs: Some(IotaRawData::MoveObject(obj)),
                ..
            }) => Some(&obj.bcs_bytes),
            _ => None,
        }
    }

    pub fn owner(&self) -> Option<Owner> {
        if let Some(data) = &self.data {
            return data.owner;
        }
        None
    }

    pub fn object_id(&self) -> Result<ObjectID, anyhow::Error> {
        Ok(match (&self.data, &self.error) {
            (Some(obj_data), None) => obj_data.object_id,
            (None, Some(IotaObjectResponseError::NotExists { object_id })) => *object_id,
            (
                None,
                Some(IotaObjectResponseError::Deleted {
                    object_id,
                    version: _,
                    digest: _,
                }),
            ) => *object_id,
            _ => bail!(
                "Could not get object_id, something went wrong with IotaObjectResponse construction."
            ),
        })
    }

    pub fn object_ref_if_exists(&self) -> Option<ObjectRef> {
        match (&self.data, &self.error) {
            (Some(obj_data), None) => Some(obj_data.object_ref()),
            _ => None,
        }
    }
}

impl TryFrom<IotaObjectResponse> for ObjectInfo {
    type Error = anyhow::Error;

    fn try_from(value: IotaObjectResponse) -> Result<Self, Self::Error> {
        let IotaObjectData {
            object_id,
            version,
            digest,
            type_,
            owner,
            previous_transaction,
            ..
        } = value.into_object()?;

        Ok(ObjectInfo {
            object_id,
            version,
            digest,
            type_: type_.ok_or_else(|| anyhow!("Object type not found for object."))?,
            owner: owner.ok_or_else(|| anyhow!("Owner not found for object."))?,
            previous_transaction: previous_transaction
                .ok_or_else(|| anyhow!("Transaction digest not found for object."))?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Eq, PartialEq)]
pub struct DisplayFieldsResponse {
    pub data: Option<BTreeMap<String, String>>,
    #[schemars(with = "Option<IotaObjectResponseErrorSchema>")]
    pub error: Option<IotaObjectResponseError>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Eq, PartialEq)]
#[serde(rename_all = "camelCase", rename = "ObjectData")]
pub struct IotaObjectData {
    #[schemars(with = "ObjectIDSchema")]
    pub object_id: ObjectID,
    /// Object version.
    #[serde_as(as = "SequenceNumberStringSchema")]
    #[schemars(with = "SequenceNumberStringSchema")]
    pub version: SequenceNumber,
    /// Base64 string representing the object digest
    #[schemars(with = "Base58Schema")]
    pub digest: ObjectDigest,
    /// The type of the object. Default to be None unless
    /// IotaObjectDataOptions.showType is set to true
    #[schemars(with = "Option<String>")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<ObjectType>,
    // Default to be None because otherwise it will be repeated for the getOwnedObjects endpoint
    /// The owner of this object. Default to be None unless
    /// IotaObjectDataOptions.showOwner is set to true
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<OwnerSchema>")]
    #[serde_as(as = "Option<OwnerSchema>")]
    pub owner: Option<Owner>,
    /// The digest of the transaction that created or last mutated this object.
    /// Default to be None unless IotaObjectDataOptions.
    /// showPreviousTransaction is set to true
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Base58Schema>")]
    pub previous_transaction: Option<TransactionDigest>,
    /// The amount of IOTA we would rebate if this object gets deleted.
    /// This number is re-calculated each time the object is mutated based on
    /// the present storage gas price.
    #[schemars(with = "Option<String>")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_rebate: Option<u64>,
    /// The Display metadata for frontend UI rendering, default to be None
    /// unless IotaObjectDataOptions.showContent is set to true This can also
    /// be None if the struct type does not have Display defined
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<DisplayFieldsResponse>,
    /// Move object content or package content, default to be None unless
    /// IotaObjectDataOptions.showContent is set to true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<IotaParsedData>,
    /// Move object content or package content in BCS, default to be None unless
    /// IotaObjectDataOptions.showBcs is set to true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bcs: Option<IotaRawData>,
}

impl IotaObjectData {
    pub fn new(
        object_ref: ObjectRef,
        obj: Object,
        layout: impl Into<Option<MoveStructLayout>>,
        options: &IotaObjectDataOptions,
        display_fields: impl Into<Option<DisplayFieldsResponse>>,
    ) -> anyhow::Result<Self> {
        let layout = layout.into();
        let display_fields = display_fields.into();
        let show_display = options.show_display;
        let IotaObjectDataOptions {
            show_type,
            show_owner,
            show_previous_transaction,
            show_content,
            show_bcs,
            show_storage_rebate,
            ..
        } = options;

        let ObjectRef {
            object_id,
            version,
            digest,
        } = object_ref;
        let type_ = if *show_type {
            Some(Into::<ObjectType>::into(&obj))
        } else {
            None
        };

        let bcs: Option<IotaRawData> = if *show_bcs {
            let data = match obj.data.clone() {
                Data::Struct(m) => {
                    let layout = layout.clone().ok_or_else(|| {
                        anyhow!("Layout is required to convert Move object to json")
                    })?;
                    IotaRawData::try_from_object(m, layout)?
                }
                Data::Package(p) => IotaRawData::try_from_package(p)
                    .map_err(|e| anyhow!("Error getting raw data from package: {e:#?}"))?,
            };
            Some(data)
        } else {
            None
        };

        let obj = obj.into_inner();

        let content: Option<IotaParsedData> = if *show_content {
            let data = match obj.data {
                Data::Struct(m) => {
                    let layout = layout.ok_or_else(|| {
                        anyhow!("Layout is required to convert Move object to json")
                    })?;
                    IotaParsedData::try_from_object(m, layout)?
                }
                Data::Package(p) => IotaParsedData::try_from_package(p)?,
            };
            Some(data)
        } else {
            None
        };

        Ok(IotaObjectData {
            object_id,
            version,
            digest,
            type_,
            owner: if *show_owner { Some(obj.owner) } else { None },
            storage_rebate: if *show_storage_rebate {
                Some(obj.storage_rebate)
            } else {
                None
            },
            previous_transaction: if *show_previous_transaction {
                Some(obj.previous_transaction)
            } else {
                None
            },
            content,
            bcs,
            display: if show_display { display_fields } else { None },
        })
    }

    pub fn object_ref(&self) -> ObjectRef {
        ObjectRef::new(self.object_id, self.version, self.digest)
    }

    pub fn object_type(&self) -> anyhow::Result<ObjectType> {
        self.type_
            .as_ref()
            .ok_or_else(|| anyhow!("type is missing for object {:?}", self.object_id))
            .cloned()
    }

    pub fn is_gas_coin(&self) -> bool {
        match self.type_.as_ref() {
            Some(ObjectType::Struct(ty)) if ty.is_gas_coin() => true,
            Some(_) => false,
            None => false,
        }
    }
}

impl Display for IotaObjectData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let type_ = if let Some(type_) = &self.type_ {
            type_.to_string()
        } else {
            "Unknown Type".into()
        };
        let mut writer = String::new();
        writeln!(
            writer,
            "{}",
            format!("----- {type_} ({}[{}]) -----", self.object_id, self.version).bold()
        )?;
        if let Some(owner) = self.owner {
            writeln!(writer, "{}: {owner}", "Owner".bold().bright_black())?;
        }

        writeln!(
            writer,
            "{}: {}",
            "Version".bold().bright_black(),
            self.version
        )?;
        if let Some(storage_rebate) = self.storage_rebate {
            writeln!(
                writer,
                "{}: {storage_rebate}",
                "Storage Rebate".bold().bright_black(),
            )?;
        }

        if let Some(previous_transaction) = self.previous_transaction {
            writeln!(
                writer,
                "{}: {previous_transaction:?}",
                "Previous Transaction".bold().bright_black(),
            )?;
        }
        if let Some(content) = self.content.as_ref() {
            writeln!(writer, "{}", "----- Data -----".bold())?;
            write!(writer, "{content}")?;
        }

        write!(f, "{writer}")
    }
}

impl TryFrom<&IotaObjectData> for GasCoin {
    type Error = anyhow::Error;
    fn try_from(object: &IotaObjectData) -> Result<Self, Self::Error> {
        match &object
            .content
            .as_ref()
            .ok_or_else(|| anyhow!("Expect object content to not be empty"))?
        {
            IotaParsedData::MoveObject(o) => {
                if o.type_.is_gas_coin() {
                    return GasCoin::try_from(&o.fields);
                }
            }
            IotaParsedData::Package(_) => {}
        }

        bail!("Gas object type is not a gas coin: {:?}", object.type_)
    }
}

impl TryFrom<&IotaMoveStruct> for GasCoin {
    type Error = anyhow::Error;
    fn try_from(move_struct: &IotaMoveStruct) -> Result<Self, Self::Error> {
        match move_struct {
            IotaMoveStruct::WithFields(fields) | IotaMoveStruct::WithTypes { type_: _, fields } => {
                if let Some(IotaMoveValue::String(balance)) = fields.get("balance") {
                    if let Ok(balance) = balance.parse::<u64>() {
                        if let Some(IotaMoveValue::UID { id }) = fields.get("id") {
                            return Ok(GasCoin::new(*id, balance));
                        }
                    }
                }
            }
            _ => {}
        }
        bail!("Struct is not a gas coin: {move_struct:?}")
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Eq, PartialEq, Default)]
#[serde(rename_all = "camelCase", rename = "ObjectDataOptions", default)]
pub struct IotaObjectDataOptions {
    /// Whether to show the type of the object. Default to be False
    pub show_type: bool,
    /// Whether to show the owner of the object. Default to be False
    pub show_owner: bool,
    /// Whether to show the previous transaction digest of the object. Default
    /// to be False
    pub show_previous_transaction: bool,
    /// Whether to show the Display metadata of the object for frontend
    /// rendering. Default to be False
    pub show_display: bool,
    /// Whether to show the content(i.e., package content or Move struct
    /// content) of the object. Default to be False
    pub show_content: bool,
    /// Whether to show the content in BCS format. Default to be False
    pub show_bcs: bool,
    /// Whether to show the storage rebate of the object. Default to be False
    pub show_storage_rebate: bool,
}

impl IotaObjectDataOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// return BCS data and all other metadata such as storage rebate
    pub fn bcs_lossless() -> Self {
        Self {
            show_bcs: true,
            show_type: true,
            show_owner: true,
            show_previous_transaction: true,
            show_display: false,
            show_content: false,
            show_storage_rebate: true,
        }
    }

    /// return full content except bcs
    pub fn full_content() -> Self {
        Self {
            show_bcs: false,
            show_type: true,
            show_owner: true,
            show_previous_transaction: true,
            show_display: false,
            show_content: true,
            show_storage_rebate: true,
        }
    }

    pub fn with_content(mut self) -> Self {
        self.show_content = true;
        self
    }

    pub fn with_owner(mut self) -> Self {
        self.show_owner = true;
        self
    }

    pub fn with_type(mut self) -> Self {
        self.show_type = true;
        self
    }

    pub fn with_display(mut self) -> Self {
        self.show_display = true;
        self
    }

    pub fn with_bcs(mut self) -> Self {
        self.show_bcs = true;
        self
    }

    pub fn with_previous_transaction(mut self) -> Self {
        self.show_previous_transaction = true;
        self
    }

    pub fn is_not_in_object_info(&self) -> bool {
        self.show_bcs || self.show_content || self.show_display || self.show_storage_rebate
    }
}

impl TryFrom<(ObjectRead, IotaObjectDataOptions)> for IotaObjectResponse {
    type Error = anyhow::Error;

    fn try_from(
        (object_read, options): (ObjectRead, IotaObjectDataOptions),
    ) -> Result<Self, Self::Error> {
        Self::try_from_object_read_and_options(object_read, &options)
    }
}

impl TryFrom<(ObjectInfo, IotaObjectDataOptions)> for IotaObjectResponse {
    type Error = anyhow::Error;

    fn try_from(
        (object_info, options): (ObjectInfo, IotaObjectDataOptions),
    ) -> Result<Self, Self::Error> {
        let IotaObjectDataOptions {
            show_type,
            show_owner,
            show_previous_transaction,
            ..
        } = options;

        Ok(Self::new_with_data(IotaObjectData {
            object_id: object_info.object_id,
            version: object_info.version,
            digest: object_info.digest,
            type_: show_type.then_some(object_info.type_),
            owner: show_owner.then_some(object_info.owner),
            previous_transaction: show_previous_transaction
                .then_some(object_info.previous_transaction),
            storage_rebate: None,
            display: None,
            content: None,
            bcs: None,
        }))
    }
}

impl IotaObjectResponse {
    /// Returns a reference to the object if there is any, otherwise an Err if
    /// the object does not exist or is deleted.
    pub fn object(&self) -> Result<&IotaObjectData, IotaObjectResponseError> {
        if let Some(data) = &self.data {
            Ok(data)
        } else if let Some(error) = &self.error {
            Err(error.clone())
        } else {
            // We really shouldn't reach this code block since either data, or error field
            // should always be filled.
            Err(IotaObjectResponseError::Unknown)
        }
    }

    /// Returns the object value if there is any, otherwise an Err if
    /// the object does not exist or is deleted.
    pub fn into_object(self) -> Result<IotaObjectData, IotaObjectResponseError> {
        match self.object() {
            Ok(data) => Ok(data.clone()),
            Err(error) => Err(error),
        }
    }
}

impl TryInto<Object> for IotaObjectData {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Object, Self::Error> {
        let protocol_config = ProtocolConfig::get_for_min_version();
        let data = match self.bcs {
            Some(IotaRawData::MoveObject(o)) => Data::Struct({
                MoveObject::new_from_execution(
                    o.type_().clone(),
                    o.version,
                    o.bcs_bytes,
                    &protocol_config,
                )?
            }),
            Some(IotaRawData::Package(p)) => Data::Package(MovePackage::new(
                p.id,
                self.version,
                p.module_map
                    .iter()
                    .map(|(k, v)| (Identifier::new_unchecked(k), v.clone()))
                    .collect(),
                protocol_config.max_move_package_size(),
                p.type_origin_table.into_iter().collect(),
                p.linkage_table.into_iter().collect(),
            )?),
            _ => Err(anyhow!(
                "BCS data is required to convert IotaObjectData to Object"
            ))?,
        };
        Ok(ObjectInner {
            data,
            owner: self
                .owner
                .ok_or_else(|| anyhow!("Owner is required to convert IotaObjectData to Object"))?,
            previous_transaction: self.previous_transaction.ok_or_else(|| {
                anyhow!("previous_transaction is required to convert IotaObjectData to Object")
            })?,
            storage_rebate: self.storage_rebate.ok_or_else(|| {
                anyhow!("storage_rebate is required to convert IotaObjectData to Object")
            })?,
        }
        .into())
    }
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", rename = "ObjectRef")]
pub struct ObjectRefSchema {
    /// Hex code as string representing the object id
    #[schemars(with = "ObjectIDSchema")]
    pub object_id: ObjectID,
    /// Object version.
    #[schemars(with = "SequenceNumberU64Schema")]
    pub version: SequenceNumber,
    /// Base64 string representing the object digest
    #[schemars(with = "Base58Schema")]
    pub digest: ObjectDigest,
}

impl SerializeAs<ObjectRef> for ObjectRefSchema {
    fn serialize_as<S>(source: &ObjectRef, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let iota_object_ref: ObjectRefSchema = (*source).into();
        iota_object_ref.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, ObjectRef> for ObjectRefSchema {
    fn deserialize_as<D>(deserializer: D) -> Result<ObjectRef, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let iota_object_ref = ObjectRefSchema::deserialize(deserializer)?;
        Ok(iota_object_ref.into())
    }
}

impl From<ObjectRef> for ObjectRefSchema {
    fn from(oref: ObjectRef) -> Self {
        Self {
            object_id: oref.object_id,
            version: oref.version,
            digest: oref.digest,
        }
    }
}

impl From<ObjectRefSchema> for ObjectRef {
    fn from(oref: ObjectRefSchema) -> Self {
        ObjectRef::new(oref.object_id, oref.version, oref.digest)
    }
}

pub trait IotaData: Sized {
    type ObjectType;
    type PackageType;
    fn try_from_object(object: MoveObject, layout: MoveStructLayout)
    -> Result<Self, anyhow::Error>;
    fn try_from_package(package: MovePackage) -> Result<Self, anyhow::Error>;
    fn try_as_move(&self) -> Option<&Self::ObjectType>;
    fn try_into_move(self) -> Option<Self::ObjectType>;
    fn try_as_package(&self) -> Option<&Self::PackageType>;
    fn type_(&self) -> Option<&StructTag>;
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(tag = "dataType", rename_all = "camelCase", rename = "RawData")]
pub enum IotaRawData {
    // Manually handle generic schema generation
    MoveObject(IotaRawMoveObject),
    Package(IotaRawMovePackage),
}

impl IotaData for IotaRawData {
    type ObjectType = IotaRawMoveObject;
    type PackageType = IotaRawMovePackage;

    fn try_from_object(object: MoveObject, _: MoveStructLayout) -> Result<Self, anyhow::Error> {
        Ok(Self::MoveObject(object.into()))
    }

    fn try_from_package(package: MovePackage) -> Result<Self, anyhow::Error> {
        Ok(Self::Package(package.into()))
    }

    fn try_as_move(&self) -> Option<&Self::ObjectType> {
        match self {
            Self::MoveObject(o) => Some(o),
            Self::Package(_) => None,
        }
    }

    fn try_into_move(self) -> Option<Self::ObjectType> {
        match self {
            Self::MoveObject(o) => Some(o),
            Self::Package(_) => None,
        }
    }

    fn try_as_package(&self) -> Option<&Self::PackageType> {
        match self {
            Self::MoveObject(_) => None,
            Self::Package(p) => Some(p),
        }
    }

    fn type_(&self) -> Option<&StructTag> {
        match self {
            Self::MoveObject(o) => Some(&o.type_),
            Self::Package(_) => None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(tag = "dataType", rename_all = "camelCase", rename = "Data")]
pub enum IotaParsedData {
    // Manually handle generic schema generation
    MoveObject(Box<IotaParsedMoveObject>),
    Package(IotaMovePackage),
}

impl IotaData for IotaParsedData {
    type ObjectType = IotaParsedMoveObject;
    type PackageType = IotaMovePackage;

    fn try_from_object(
        object: MoveObject,
        layout: MoveStructLayout,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self::MoveObject(Box::new(
            IotaParsedMoveObject::try_from_layout(object, layout)?,
        )))
    }

    fn try_from_package(package: MovePackage) -> Result<Self, anyhow::Error> {
        let mut disassembled = BTreeMap::new();
        for bytecode in package.serialized_module_map().values() {
            // this function is only from JSON RPC - it is OK to deserialize with max Move
            // binary version
            let module = move_binary_format::CompiledModule::deserialize_with_defaults(bytecode)
                .map_err(|error| IotaError::ModuleDeserializationFailure {
                    error: error.to_string(),
                })?;
            let d = move_disassembler::disassembler::Disassembler::from_module(
                &module,
                move_ir_types::location::Spanned::unsafe_no_loc(()).loc,
            )
            .map_err(|e| IotaError::ObjectSerialization {
                error: e.to_string(),
            })?;
            let bytecode_str = d
                .disassemble()
                .map_err(|e| IotaError::ObjectSerialization {
                    error: e.to_string(),
                })?;
            disassembled.insert(module.name().to_string(), Value::String(bytecode_str));
        }

        Ok(Self::Package(IotaMovePackage { disassembled }))
    }

    fn try_as_move(&self) -> Option<&Self::ObjectType> {
        match self {
            Self::MoveObject(o) => Some(o),
            Self::Package(_) => None,
        }
    }

    fn try_into_move(self) -> Option<Self::ObjectType> {
        match self {
            Self::MoveObject(o) => Some(*o),
            Self::Package(_) => None,
        }
    }

    fn try_as_package(&self) -> Option<&Self::PackageType> {
        match self {
            Self::MoveObject(_) => None,
            Self::Package(p) => Some(p),
        }
    }

    fn type_(&self) -> Option<&StructTag> {
        match self {
            Self::MoveObject(o) => Some(&o.type_),
            Self::Package(_) => None,
        }
    }
}

impl Display for IotaParsedData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut writer = String::new();
        match self {
            IotaParsedData::MoveObject(o) => {
                writeln!(writer, "{}: {}", "type".bold().bright_black(), o.type_)?;
                write!(writer, "{}", &o.fields)?;
            }
            IotaParsedData::Package(p) => {
                write!(
                    writer,
                    "{}: {:?}",
                    "Modules".bold().bright_black(),
                    p.disassembled.keys()
                )?;
            }
        }
        write!(f, "{writer}")
    }
}

impl IotaParsedData {
    pub fn try_from_object_read(object_read: ObjectRead) -> Result<Self, anyhow::Error> {
        match object_read {
            ObjectRead::NotExists(id) => Err(anyhow::anyhow!("Object {id} does not exist")),
            ObjectRead::Exists(_object_ref, o, layout) => {
                let data = match o.into_inner().data {
                    Data::Struct(m) => {
                        let layout = layout.ok_or_else(|| {
                            anyhow!("Layout is required to convert Move object to json")
                        })?;
                        IotaParsedData::try_from_object(m, layout)?
                    }
                    Data::Package(p) => IotaParsedData::try_from_package(p)?,
                };
                Ok(data)
            }
            ObjectRead::Deleted(object_ref) => Err(anyhow::anyhow!(
                "Object {} was deleted at version {} with digest {}",
                object_ref.object_id,
                object_ref.version,
                object_ref.digest
            )),
        }
    }
}

pub trait IotaMoveObject: Sized {
    fn try_from_layout(object: MoveObject, layout: MoveStructLayout)
    -> Result<Self, anyhow::Error>;

    fn try_from(o: MoveObject, resolver: &impl GetModule) -> Result<Self, anyhow::Error> {
        let layout = o.get_layout(resolver)?;
        Self::try_from_layout(o, layout)
    }

    fn type_(&self) -> &StructTag;
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(rename = "MoveObject", rename_all = "camelCase")]
pub struct IotaParsedMoveObject {
    #[serde(rename = "type")]
    #[schemars(with = "StructTagSchema")]
    #[serde_as(as = "StructTagSchema")]
    pub type_: StructTag,
    pub fields: IotaMoveStruct,
}

impl IotaMoveObject for IotaParsedMoveObject {
    fn try_from_layout(
        object: MoveObject,
        layout: MoveStructLayout,
    ) -> Result<Self, anyhow::Error> {
        let move_struct = object.to_move_struct(&layout)?.into();

        Ok(
            if let IotaMoveStruct::WithTypes { type_, fields } = move_struct {
                IotaParsedMoveObject {
                    type_,
                    fields: IotaMoveStruct::WithFields(fields),
                }
            } else {
                IotaParsedMoveObject {
                    type_: object.struct_tag().clone(),
                    fields: move_struct,
                }
            },
        )
    }

    fn type_(&self) -> &StructTag {
        &self.type_
    }
}

impl IotaParsedMoveObject {
    pub fn try_from_object_read(object_read: ObjectRead) -> Result<Self, anyhow::Error> {
        let parsed_data = IotaParsedData::try_from_object_read(object_read)?;
        match parsed_data {
            IotaParsedData::MoveObject(o) => Ok(*o),
            IotaParsedData::Package(_) => Err(anyhow::anyhow!("Object is not a Move object")),
        }
    }

    pub fn read_dynamic_field_value(&self, field_name: &str) -> Option<IotaMoveValue> {
        match &self.fields {
            IotaMoveStruct::WithFields(fields) => fields.get(field_name).cloned(),
            IotaMoveStruct::WithTypes { fields, .. } => fields.get(field_name).cloned(),
            _ => None,
        }
    }
}

pub fn type_and_fields_from_move_event_data(
    event_data: MoveValue,
) -> IotaResult<(StructTag, serde_json::Value)> {
    match event_data.into() {
        IotaMoveValue::Struct(move_struct) => match &move_struct {
            IotaMoveStruct::WithTypes { type_, .. } => {
                Ok((type_.clone(), move_struct.clone().to_json_value()))
            }
            _ => Err(IotaError::ObjectDeserialization {
                error: "Found non-type IotaMoveStruct in MoveValue event".to_string(),
            }),
        },
        IotaMoveValue::Variant(v) => Ok((v.type_.clone(), v.to_json_value())),
        IotaMoveValue::Vector(_)
        | IotaMoveValue::Number(_)
        | IotaMoveValue::Bool(_)
        | IotaMoveValue::Address(_)
        | IotaMoveValue::String(_)
        | IotaMoveValue::UID { .. }
        | IotaMoveValue::Option(_) => Err(IotaError::ObjectDeserialization {
            error: "Invalid MoveValue event type -- this should not be possible".to_string(),
        }),
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(rename = "RawMoveObject", rename_all = "camelCase")]
pub struct IotaRawMoveObject {
    #[serde(rename = "type")]
    #[schemars(with = "StructTagSchema")]
    #[serde_as(as = "StructTagSchema")]
    pub type_: StructTag,
    #[schemars(with = "SequenceNumberU64Schema")]
    pub version: SequenceNumber,
    #[serde_as(as = "Base64")]
    #[schemars(with = "Base64Schema")]
    pub bcs_bytes: Vec<u8>,
}

impl From<MoveObject> for IotaRawMoveObject {
    fn from(o: MoveObject) -> Self {
        Self {
            type_: o.struct_tag().clone(),
            version: o.version(),
            bcs_bytes: o.into_contents(),
        }
    }
}

impl IotaMoveObject for IotaRawMoveObject {
    fn try_from_layout(
        object: MoveObject,
        _layout: MoveStructLayout,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            type_: object.struct_tag().clone(),
            version: object.version(),
            bcs_bytes: object.into_contents(),
        })
    }

    fn type_(&self) -> &StructTag {
        &self.type_
    }
}

impl IotaRawMoveObject {
    pub fn deserialize<'a, T: Deserialize<'a>>(&'a self) -> Result<T, anyhow::Error> {
        Ok(bcs::from_bytes(self.bcs_bytes.as_slice())?)
    }
}

/// Store the origin of a data type where it first appeared in the version
/// chain.
///
/// A data type is identified by the name of the module and the name of the
/// struct/enum in combination.
///
/// # Undefined behavior
///
/// Directly modifying any field is undefined behavior. The fields are only
/// public for read-only access.
#[serde_as]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, JsonSchema)]
#[schemars(rename = "TypeOrigin")]
pub struct IotaTypeOrigin {
    /// The name of the module the data type resides in.
    #[schemars(with = "IdentifierSchema")]
    pub module_name: Identifier,
    /// The name of the data type.
    ///
    /// Here this either refers to an enum or a struct identifier.
    // `struct_name` alias to support backwards compatibility with the old name
    #[serde(alias = "struct_name")]
    #[schemars(with = "IdentifierSchema")]
    pub datatype_name: Identifier,
    /// `Storage ID` of the package, where the given type first appeared.
    #[schemars(with = "ObjectIDSchema")]
    pub package: ObjectID,
}

impl From<TypeOrigin> for IotaTypeOrigin {
    fn from(origin: TypeOrigin) -> Self {
        Self {
            module_name: origin.module_name,
            datatype_name: origin.datatype_name,
            package: origin.package,
        }
    }
}

impl From<IotaTypeOrigin> for TypeOrigin {
    fn from(origin: IotaTypeOrigin) -> Self {
        Self {
            module_name: origin.module_name,
            datatype_name: origin.datatype_name,
            package: origin.package,
        }
    }
}

/// Value for the [MovePackage]'s linkage_table.
///
/// # Undefined behavior
///
/// Directly modifying any field is undefined behavior. The fields are only
/// public for read-only access.
#[serde_as]
#[derive(JsonSchema)]
#[schemars(rename = "UpgradeInfo")]
pub struct IotaUpgradeInfo {
    /// `Storage ID`/`Package ID` of the referred package.
    #[schemars(with = "ObjectIDSchema")]
    pub upgraded_id: ObjectID,
    /// The version of the package at `upgraded_id`.
    #[schemars(with = "SequenceNumberU64Schema")]
    pub upgraded_version: SequenceNumber,
}

impl From<UpgradeInfo> for IotaUpgradeInfo {
    fn from(info: UpgradeInfo) -> Self {
        Self {
            upgraded_id: info.upgraded_id,
            upgraded_version: info.upgraded_version,
        }
    }
}

impl From<IotaUpgradeInfo> for UpgradeInfo {
    fn from(info: IotaUpgradeInfo) -> Self {
        Self {
            upgraded_id: info.upgraded_id,
            upgraded_version: info.upgraded_version,
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(rename = "RawMovePackage", rename_all = "camelCase")]
pub struct IotaRawMovePackage {
    #[schemars(with = "ObjectIDSchema")]
    pub id: ObjectID,
    #[schemars(with = "SequenceNumberU64Schema")]
    pub version: SequenceNumber,
    #[schemars(with = "BTreeMap<String, Base64Schema>")]
    #[serde_as(as = "BTreeMap<_, Base64>")]
    pub module_map: BTreeMap<String, Vec<u8>>,
    #[schemars(with = "Vec<IotaTypeOrigin>")]
    pub type_origin_table: Vec<TypeOrigin>,
    #[schemars(with = "BTreeMap<ObjectIDSchema, IotaUpgradeInfo>")]
    pub linkage_table: BTreeMap<ObjectID, UpgradeInfo>,
}

impl From<MovePackage> for IotaRawMovePackage {
    fn from(p: MovePackage) -> Self {
        Self {
            id: p.id(),
            version: p.version(),
            module_map: p
                .serialized_module_map()
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            type_origin_table: p.type_origin_table().clone(),
            linkage_table: p.linkage_table().clone(),
        }
    }
}

impl IotaRawMovePackage {
    pub fn to_move_package(
        &self,
        max_move_package_size: u64,
    ) -> Result<MovePackage, ExecutionError> {
        Ok(MovePackage::new(
            self.id,
            self.version,
            self.module_map
                .iter()
                .map(|(k, v)| (Identifier::new_unchecked(k), v.clone()))
                .collect(),
            max_move_package_size,
            self.type_origin_table.clone(),
            self.linkage_table.clone(),
        )?)
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, PartialEq, Eq)]
#[serde(tag = "status", content = "details", rename = "ObjectRead")]
#[expect(clippy::large_enum_variant)]
pub enum IotaPastObjectResponse {
    /// The object exists and is found with this version
    VersionFound(IotaObjectData),
    /// The object does not exist
    ObjectNotExists(#[schemars(with = "ObjectIDSchema")] ObjectID),
    /// The object is found to be deleted with this version
    ObjectDeleted(
        #[schemars(with = "ObjectRefSchema")]
        #[serde_as(as = "ObjectRefSchema")]
        ObjectRef,
    ),
    /// The object exists but not found with this version
    VersionNotFound(
        #[schemars(with = "ObjectIDSchema")] ObjectID,
        #[schemars(with = "SequenceNumberU64Schema")] SequenceNumber,
    ),
    /// The asked object version is higher than the latest
    VersionTooHigh {
        #[schemars(with = "ObjectIDSchema")]
        object_id: ObjectID,
        #[schemars(with = "SequenceNumberU64Schema")]
        asked_version: SequenceNumber,
        #[schemars(with = "SequenceNumberU64Schema")]
        latest_version: SequenceNumber,
    },
}

impl IotaPastObjectResponse {
    /// Returns a reference to the object if there is any, otherwise an Err
    pub fn object(&self) -> UserInputResult<&IotaObjectData> {
        match &self {
            Self::ObjectDeleted(oref) => Err(UserInputError::ObjectDeleted { object_ref: *oref }),
            Self::ObjectNotExists(id) => Err(UserInputError::ObjectNotFound {
                object_id: *id,
                version: None,
            }),
            Self::VersionFound(o) => Ok(o),
            Self::VersionNotFound(id, seq_num) => Err(UserInputError::ObjectNotFound {
                object_id: *id,
                version: Some(*seq_num),
            }),
            Self::VersionTooHigh {
                object_id,
                asked_version,
                latest_version,
            } => Err(UserInputError::ObjectSequenceNumberTooHigh {
                object_id: *object_id,
                asked_version: *asked_version,
                latest_version: *latest_version,
            }),
        }
    }

    /// Returns the object value if there is any, otherwise an Err
    pub fn into_object(self) -> UserInputResult<IotaObjectData> {
        match self {
            Self::ObjectDeleted(oref) => Err(UserInputError::ObjectDeleted { object_ref: oref }),
            Self::ObjectNotExists(id) => Err(UserInputError::ObjectNotFound {
                object_id: id,
                version: None,
            }),
            Self::VersionFound(o) => Ok(o),
            Self::VersionNotFound(object_id, version) => Err(UserInputError::ObjectNotFound {
                object_id,
                version: Some(version),
            }),
            Self::VersionTooHigh {
                object_id,
                asked_version,
                latest_version,
            } => Err(UserInputError::ObjectSequenceNumberTooHigh {
                object_id,
                asked_version,
                latest_version,
            }),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(rename = "MovePackage", rename_all = "camelCase")]
pub struct IotaMovePackage {
    pub disassembled: BTreeMap<String, Value>,
}

pub type QueryObjectsPage = Page<IotaObjectResponse, CheckpointedObjectID>;
pub type ObjectsPage = Page<IotaObjectResponse, ObjectID>;

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Copy, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointedObjectID {
    #[schemars(with = "ObjectIDSchema")]
    pub object_id: ObjectID,
    #[schemars(with = "Option<String>")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_checkpoint: Option<CheckpointSequenceNumber>,
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(rename = "GetPastObjectRequest", rename_all = "camelCase")]
pub struct IotaGetPastObjectRequest {
    /// the ID of the queried object
    #[schemars(with = "ObjectIDSchema")]
    pub object_id: ObjectID,
    /// the version of the queried object.
    #[schemars(with = "SequenceNumberStringSchema")]
    #[serde_as(as = "SequenceNumberStringSchema")]
    pub version: SequenceNumber,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum IotaObjectDataFilter {
    MatchAll(Vec<IotaObjectDataFilter>),
    MatchAny(Vec<IotaObjectDataFilter>),
    MatchNone(Vec<IotaObjectDataFilter>),
    /// Query by type a specified Package.
    Package(#[schemars(with = "ObjectIDSchema")] ObjectID),
    /// Query by type a specified Move module.
    MoveModule {
        /// the Move package ID
        #[schemars(with = "ObjectIDSchema")]
        package: ObjectID,
        /// the module name
        #[schemars(with = "IdentifierSchema")]
        module: Identifier,
    },
    /// Query by type
    StructType(
        #[schemars(with = "StructTagSchema")]
        #[serde_as(as = "StructTagSchema")]
        StructTag,
    ),
    AddressOwner(#[schemars(with = "IotaAddressSchema")] IotaAddress),
    ObjectOwner(#[schemars(with = "ObjectIDSchema")] ObjectID),
    ObjectId(#[schemars(with = "ObjectIDSchema")] ObjectID),
    // allow querying for multiple object ids
    ObjectIds(#[schemars(with = "Vec<ObjectIDSchema>")] Vec<ObjectID>),
    Version(
        #[serde_as(as = "DisplayFromStr")]
        #[schemars(with = "String")]
        u64,
    ),
}

impl IotaObjectDataFilter {
    pub fn gas_coin() -> Self {
        Self::StructType(StructTag::new_gas_coin())
    }

    pub fn and(self, other: Self) -> Self {
        Self::MatchAll(vec![self, other])
    }
    pub fn or(self, other: Self) -> Self {
        Self::MatchAny(vec![self, other])
    }
    pub fn not(self, other: Self) -> Self {
        Self::MatchNone(vec![self, other])
    }

    pub fn matches(&self, object: &ObjectInfo) -> bool {
        match self {
            IotaObjectDataFilter::MatchAll(filters) => !filters.iter().any(|f| !f.matches(object)),
            IotaObjectDataFilter::MatchAny(filters) => filters.iter().any(|f| f.matches(object)),
            IotaObjectDataFilter::MatchNone(filters) => !filters.iter().any(|f| f.matches(object)),
            IotaObjectDataFilter::StructType(s) => {
                let obj_tag: StructTag = match &object.type_ {
                    ObjectType::Package => return false,
                    ObjectType::Struct(s) => s.clone().into(),
                };
                // If people do not provide type_params, we will match all type_params
                // e.g. `0x2::coin::Coin` can match `0x2::coin::Coin<0x2::iota::IOTA>`
                if !s.type_params().is_empty() && s.type_params() != obj_tag.type_params() {
                    false
                } else {
                    obj_tag.address() == s.address()
                        && obj_tag.module() == s.module()
                        && obj_tag.name() == s.name()
                }
            }
            IotaObjectDataFilter::MoveModule { package, module } => {
                matches!(&object.type_, ObjectType::Struct(s) if &ObjectID::from(s.address()) == package
                        && s.module() == module)
            }
            IotaObjectDataFilter::Package(p) => {
                matches!(&object.type_, ObjectType::Struct(s) if &ObjectID::from(s.address()) == p)
            }
            IotaObjectDataFilter::AddressOwner(a) => {
                matches!(object.owner, Owner::Address(addr) if &addr == a)
            }
            IotaObjectDataFilter::ObjectOwner(o) => {
                matches!(object.owner, Owner::Object(addr) if &addr == o)
            }
            IotaObjectDataFilter::ObjectId(id) => &object.object_id == id,
            IotaObjectDataFilter::ObjectIds(ids) => ids.contains(&object.object_id),
            IotaObjectDataFilter::Version(v) => object.version == *v,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", rename = "ObjectResponseQuery", default)]
pub struct IotaObjectResponseQuery {
    /// If None, no filter will be applied
    pub filter: Option<IotaObjectDataFilter>,
    /// config which fields to include in the response, by default only digest
    /// is included
    pub options: Option<IotaObjectDataOptions>,
}

impl IotaObjectResponseQuery {
    pub fn new(
        filter: Option<IotaObjectDataFilter>,
        options: Option<IotaObjectDataOptions>,
    ) -> Self {
        Self { filter, options }
    }

    pub fn new_with_filter(filter: IotaObjectDataFilter) -> Self {
        Self {
            filter: Some(filter),
            options: None,
        }
    }

    pub fn new_with_options(options: IotaObjectDataOptions) -> Self {
        Self {
            filter: None,
            options: Some(options),
        }
    }
}
