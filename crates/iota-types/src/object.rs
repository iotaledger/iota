// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    fmt::{Debug, Display, Formatter},
    mem::size_of,
    sync::Arc,
};

use iota_protocol_config::ProtocolConfig;
pub use iota_sdk_types::{MoveStruct as MoveObject, ObjectData as Data};
use iota_sdk_types::{ObjectId, Owner, StructTag, TypeTag, move_package::MovePackage};
use move_binary_format::CompiledModule;
use move_bytecode_utils::{layout::TypeLayoutBuilder, module_cache::GetModule};
use move_core_types::annotated_value::{MoveStruct, MoveStructLayout, MoveTypeLayout, MoveValue};
use serde::{Deserialize, Serialize};

use self::{balance_traversal::BalanceTraversal, bounded_visitor::BoundedVisitor};
use crate::{
    balance::Balance,
    base_types::{
        IotaAddress, MoveObjectType, ObjectDigest, ObjectRef, SequenceNumber, TransactionDigest,
    },
    coin::{Coin, CoinMetadata, TreasuryCap},
    crypto::{default_hash, deterministic_random_account_key},
    error::{
        ExecutionError, ExecutionErrorKind, IotaError, IotaResult, UserInputError, UserInputResult,
    },
    gas_coin::{GAS, GasCoin},
    iota_sdk_types_conversions::type_tag_sdk_to_core,
    layout_resolver::LayoutResolver,
    move_package::MovePackageExt,
    timelock::timelock::TimeLock,
};

mod balance_traversal;
pub mod bounded_visitor;
pub mod option_visitor;

pub const GAS_VALUE_FOR_TESTING: u64 = 300_000_000_000_000;
pub const OBJECT_START_VERSION: SequenceNumber = SequenceNumber::from_u64(1);

/// Index marking the end of the object's ID + the beginning of its version
pub const ID_END_INDEX: usize = ObjectId::LENGTH;

mod move_object_ext {
    pub trait Sealed {}
    impl Sealed for super::MoveObject {}
}

pub trait MoveObjectExt: Sized + move_object_ext::Sealed {
    fn new_from_execution(
        tag: StructTag,
        version: SequenceNumber,
        contents: Vec<u8>,
        protocol_config: &ProtocolConfig,
    ) -> Result<Self, ExecutionError>;
    fn new_from_execution_with_limit(
        tag: StructTag,
        version: SequenceNumber,
        contents: Vec<u8>,
        max_move_object_size: u64,
    ) -> Result<Self, ExecutionError>;
    fn new_gas_coin(version: SequenceNumber, id: ObjectId, value: u64) -> Self;
    fn new_coin(coin_type: TypeTag, version: SequenceNumber, id: ObjectId, value: u64) -> Self;
    fn get_coin_value_unchecked(&self) -> u64;
    fn set_coin_value_unchecked(&mut self, value: u64);
    fn set_clock_timestamp_ms_unchecked(&mut self, timestamp_ms: u64);
    fn update_contents(
        &mut self,
        new_contents: Vec<u8>,
        protocol_config: &ProtocolConfig,
    ) -> Result<(), ExecutionError>;
    fn update_contents_with_limit(
        &mut self,
        new_contents: Vec<u8>,
        max_move_object_size: u64,
    ) -> Result<(), ExecutionError>;
    fn increment_version_to(&mut self, next: SequenceNumber);
    fn decrement_version_to(&mut self, prev: SequenceNumber);
    fn get_layout(&self, resolver: &impl GetModule) -> Result<MoveStructLayout, IotaError>;
    fn get_struct_layout_from_struct_tag(
        struct_tag: StructTag,
        resolver: &impl GetModule,
    ) -> Result<MoveStructLayout, IotaError>;
    fn to_move_struct(&self, layout: &MoveStructLayout) -> Result<MoveStruct, IotaError>;
    fn object_size_for_gas_metering(&self) -> usize;
    fn get_total_iota(&self, layout_resolver: &mut dyn LayoutResolver) -> Result<u64, IotaError>;
    fn get_coin_balances(
        &self,
        layout_resolver: &mut dyn LayoutResolver,
    ) -> Result<BTreeMap<TypeTag, u64>, IotaError>;
}

impl MoveObjectExt for MoveObject {
    /// Creates a new Move object of type `tag` with BCS encoded bytes in
    /// `contents`.
    fn new_from_execution(
        tag: StructTag,
        version: SequenceNumber,
        contents: Vec<u8>,
        protocol_config: &ProtocolConfig,
    ) -> Result<Self, ExecutionError> {
        Self::new_from_execution_with_limit(
            tag,
            version,
            contents,
            protocol_config.max_move_object_size(),
        )
    }

    /// Creates a new Move object of type `tag` with BCS encoded bytes in
    /// `contents`. It allows to set a `max_move_object_size` for that.
    fn new_from_execution_with_limit(
        tag: StructTag,
        version: SequenceNumber,
        contents: Vec<u8>,
        max_move_object_size: u64,
    ) -> Result<Self, ExecutionError> {
        if contents.len() as u64 > max_move_object_size {
            return Err(ExecutionError::from_kind(
                ExecutionErrorKind::ObjectTooBig {
                    object_size: contents.len() as u64,
                    max_object_size: max_move_object_size,
                },
            ));
        }
        Self::new(tag.into(), version, contents).map_err(ExecutionError::invariant_violation)
    }

    fn new_gas_coin(version: SequenceNumber, id: ObjectId, value: u64) -> Self {
        // unwrap safe because coins are always smaller than the max object size

        Self::new_from_execution_with_limit(
            StructTag::new_gas_coin(),
            version,
            GasCoin::new(id, value).to_bcs_bytes(),
            256,
        )
        .unwrap()
    }

    fn new_coin(coin_type: TypeTag, version: SequenceNumber, id: ObjectId, value: u64) -> Self {
        // unwrap safe because coins are always smaller than the max object size

        Self::new_from_execution_with_limit(
            StructTag::new_coin(coin_type),
            version,
            Coin::new(id, value).to_bcs_bytes(),
            256,
        )
        .unwrap()
    }

    /// Return the `value: u64` field of a `Coin<T>` type.
    /// Useful for reading the coin without deserializing the object into a Move
    /// value. It is the caller's responsibility to check that `self` is a coin.
    /// This function may panic or do something unexpected otherwise.
    fn get_coin_value_unchecked(&self) -> u64 {
        debug_assert!(self.object_type().is_coin());
        // 32 bytes for object ID, 8 for balance
        debug_assert!(self.contents().len() == 40);

        // unwrap safe because we checked that it is a coin
        u64::from_le_bytes(<[u8; 8]>::try_from(&self.contents()[ID_END_INDEX..]).unwrap())
    }

    /// Update the `value: u64` field of a `Coin<T>` type.
    /// Useful for updating the coin without deserializing the object into a
    /// Move value. It is the caller's responsibility to check that `self` is a
    /// coin.
    /// This function may panic or do something unexpected otherwise.
    fn set_coin_value_unchecked(&mut self, value: u64) {
        debug_assert!(self.object_type().is_coin());
        // 32 bytes for object ID, 8 for balance
        debug_assert!(self.contents().len() == 40);

        let mut new_contents = self.contents().to_vec();
        new_contents[ID_END_INDEX..].copy_from_slice(&value.to_le_bytes());
        self.set_contents(new_contents).unwrap();
    }

    /// Update the `timestamp_ms: u64` field of the `Clock` type.
    /// Useful for updating the clock without deserializing the object into a
    /// Move value. It is the caller's responsibility to check that `self` is a
    /// `Clock`.
    /// This function may panic or do something unexpected otherwise.
    fn set_clock_timestamp_ms_unchecked(&mut self, timestamp_ms: u64) {
        debug_assert!(self.struct_tag().is_clock());
        // 32 bytes for object ID, 8 for timestamp
        debug_assert!(self.contents().len() == 40);

        let mut new_contents = self.contents().to_vec();
        new_contents[ID_END_INDEX..].copy_from_slice(&timestamp_ms.to_le_bytes());
        self.set_contents(new_contents).unwrap();
    }

    /// Update the contents of this object but does not increment its version
    fn update_contents(
        &mut self,
        new_contents: Vec<u8>,
        protocol_config: &ProtocolConfig,
    ) -> Result<(), ExecutionError> {
        self.update_contents_with_limit(new_contents, protocol_config.max_move_object_size())
    }

    fn update_contents_with_limit(
        &mut self,
        new_contents: Vec<u8>,
        max_move_object_size: u64,
    ) -> Result<(), ExecutionError> {
        if new_contents.len() as u64 > max_move_object_size {
            return Err(ExecutionError::from_kind(
                ExecutionErrorKind::ObjectTooBig {
                    object_size: new_contents.len() as u64,
                    max_object_size: max_move_object_size,
                },
            ));
        }

        #[cfg(debug_assertions)]
        let old_id = self.id();

        self.set_contents(new_contents)
            .map_err(ExecutionError::invariant_violation)?;

        // Update should not modify ID
        #[cfg(debug_assertions)]
        debug_assert_eq!(self.id(), old_id);

        Ok(())
    }

    /// Sets the version of this object to a new value which is assumed to be
    /// higher (and checked to be higher in debug).
    fn increment_version_to(&mut self, next: SequenceNumber) {
        debug_assert!(
            self.version() < next,
            "Not an increment: {} to {next}",
            self.version()
        );
        self.set_version(next);
    }

    /// Sets the version to a lower value (checked in debug).
    fn decrement_version_to(&mut self, prev: SequenceNumber) {
        debug_assert!(
            prev < self.version(),
            "Not a decrement: {} to {prev}",
            self.version()
        );
        self.set_version(prev);
    }

    /// Get a `MoveStructLayout` for `self`.
    /// The `resolver` value must contain the module that declares
    /// `self.object_type` and the (transitive) dependencies of
    /// `self.object_type` in order for this to succeed. Failure will result
    /// in an `ObjectSerializationError`
    fn get_layout(&self, resolver: &impl GetModule) -> Result<MoveStructLayout, IotaError> {
        Self::get_struct_layout_from_struct_tag(self.struct_tag().clone(), resolver)
    }

    fn get_struct_layout_from_struct_tag(
        struct_tag: StructTag,
        resolver: &impl GetModule,
    ) -> Result<MoveStructLayout, IotaError> {
        let type_ = TypeTag::Struct(Box::new(struct_tag));
        let layout = TypeLayoutBuilder::build_with_types(&type_tag_sdk_to_core(&type_), resolver)
            .map_err(|e| IotaError::ObjectSerialization {
            error: e.to_string(),
        })?;
        match layout {
            MoveTypeLayout::Struct(l) => Ok(*l),
            _ => unreachable!(
                "We called build_with_types on Struct type, should get a struct layout"
            ),
        }
    }

    /// Convert `self` to the JSON representation dictated by `layout`.
    fn to_move_struct(&self, layout: &MoveStructLayout) -> Result<MoveStruct, IotaError> {
        BoundedVisitor::deserialize_struct(self.contents(), layout).map_err(|e| {
            IotaError::ObjectSerialization {
                error: e.to_string(),
            }
        })
    }

    /// Approximate size of the object in bytes. This is used for gas metering.
    /// For the type tag field, we serialize it on the spot to get the accurate
    /// size. This should not be very expensive since the type tag is
    /// usually simple, and we only do this once per object being mutated.
    fn object_size_for_gas_metering(&self) -> usize {
        let serialized_type_tag_size =
            bcs::serialized_size(self.object_type()).expect("Serializing type tag should not fail");
        // + 8 for `version`
        self.contents().len() + serialized_type_tag_size + 8
    }

    /// Get the total amount of IOTA embedded in `self`. Intended for testing
    /// purposes
    fn get_total_iota(&self, layout_resolver: &mut dyn LayoutResolver) -> Result<u64, IotaError> {
        let balances = self.get_coin_balances(layout_resolver)?;
        Ok(balances.get(&GAS::type_tag()).copied().unwrap_or(0))
    }

    /// Get the total balances for all `Coin<T>` embedded in `self`.
    fn get_coin_balances(
        &self,
        layout_resolver: &mut dyn LayoutResolver,
    ) -> Result<BTreeMap<TypeTag, u64>, IotaError> {
        // Fast path without deserialization.
        if let Some(type_tag) = self.object_type().coin_type_opt() {
            let balance = self.get_coin_value_unchecked();
            Ok(if balance > 0 {
                BTreeMap::from([(type_tag.clone(), balance)])
            } else {
                BTreeMap::default()
            })
        } else {
            let layout = layout_resolver.get_annotated_layout(self.struct_tag())?;

            let mut traversal = BalanceTraversal::default();
            MoveValue::visit_deserialize(self.contents(), &layout.into_layout(), &mut traversal)
                .map_err(|e| IotaError::ObjectSerialization {
                    error: e.to_string(),
                })?;

            Ok(traversal.finish())
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Hash)]
#[serde(rename = "Object")]
pub struct ObjectInner {
    /// The meat of the object
    pub data: Data,
    /// The owner that unlocks this object
    pub owner: Owner,
    /// The digest of the transaction that created or last mutated this object
    pub previous_transaction: TransactionDigest,
    /// The amount of IOTA we would rebate if this object gets deleted.
    /// This number is re-calculated each time the object is mutated based on
    /// the present storage gas price.
    pub storage_rebate: u64,
}

#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Hash)]
#[serde(from = "ObjectInner")]
pub struct Object(Arc<ObjectInner>);

impl From<ObjectInner> for Object {
    fn from(inner: ObjectInner) -> Self {
        Self(Arc::new(inner))
    }
}

impl Object {
    pub fn into_inner(self) -> ObjectInner {
        match Arc::try_unwrap(self.0) {
            Ok(inner) => inner,
            Err(inner_arc) => (*inner_arc).clone(),
        }
    }

    pub fn as_inner(&self) -> &ObjectInner {
        &self.0
    }

    pub fn owner(&self) -> &Owner {
        &self.0.owner
    }

    pub fn new_from_genesis(
        data: Data,
        owner: Owner,
        previous_transaction: TransactionDigest,
    ) -> Self {
        ObjectInner {
            data,
            owner,
            previous_transaction,
            storage_rebate: 0,
        }
        .into()
    }

    /// Create a new Move object
    pub fn new_move(o: MoveObject, owner: Owner, previous_transaction: TransactionDigest) -> Self {
        ObjectInner {
            data: Data::Struct(o),
            owner,
            previous_transaction,
            storage_rebate: 0,
        }
        .into()
    }

    pub fn new_package_from_data(data: Data, previous_transaction: TransactionDigest) -> Self {
        ObjectInner {
            data,
            owner: Owner::Immutable,
            previous_transaction,
            storage_rebate: 0,
        }
        .into()
    }

    // Note: this will panic if `modules` is empty
    pub fn new_from_package(package: MovePackage, previous_transaction: TransactionDigest) -> Self {
        Self::new_package_from_data(Data::Package(package), previous_transaction)
    }

    pub fn new_package<'p>(
        modules: &[CompiledModule],
        previous_transaction: TransactionDigest,
        protocol_config: &ProtocolConfig,
        dependencies: impl IntoIterator<Item = &'p MovePackage>,
    ) -> Result<Self, ExecutionError> {
        Ok(Self::new_package_from_data(
            Data::Package(MovePackage::new_initial(
                modules,
                protocol_config,
                dependencies,
            )?),
            previous_transaction,
        ))
    }

    pub fn new_upgraded_package<'p>(
        previous_package: &MovePackage,
        new_package_id: ObjectId,
        modules: &[CompiledModule],
        previous_transaction: TransactionDigest,
        protocol_config: &ProtocolConfig,
        dependencies: impl IntoIterator<Item = &'p MovePackage>,
    ) -> Result<Self, ExecutionError> {
        Ok(Self::new_package_from_data(
            Data::Package(previous_package.new_upgraded(
                new_package_id,
                modules,
                protocol_config,
                dependencies,
            )?),
            previous_transaction,
        ))
    }

    pub fn new_package_for_testing(
        modules: &[CompiledModule],
        previous_transaction: TransactionDigest,
        dependencies: impl IntoIterator<Item = MovePackage>,
    ) -> Result<Self, ExecutionError> {
        let dependencies: Vec<_> = dependencies.into_iter().collect();
        let config = ProtocolConfig::get_for_max_version_UNSAFE();
        Self::new_package(modules, previous_transaction, &config, &dependencies)
    }

    /// Create a system package which is not subject to size limits. Panics if
    /// the object ID is not a known system package.
    pub fn new_system_package(
        modules: &[CompiledModule],
        version: SequenceNumber,
        dependencies: Vec<ObjectId>,
        previous_transaction: TransactionDigest,
    ) -> Self {
        let ret = Self::new_package_from_data(
            Data::Package(MovePackage::new_system(version, modules, dependencies)),
            previous_transaction,
        );

        #[cfg(not(msim))]
        assert!(ret.is_system_package());

        ret
    }
}

impl std::ops::Deref for Object {
    type Target = ObjectInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Object {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

impl ObjectInner {
    /// Returns true if the object is a system package.
    pub fn is_system_package(&self) -> bool {
        self.is_package() && self.id().is_system_package()
    }

    pub fn is_immutable(&self) -> bool {
        self.owner.is_immutable()
    }

    pub fn is_address_owned(&self) -> bool {
        self.owner.is_address()
    }

    pub fn is_child_object(&self) -> bool {
        self.owner.is_object()
    }

    pub fn is_shared(&self) -> bool {
        self.owner.is_shared()
    }

    pub fn get_single_owner(&self) -> Option<IotaAddress> {
        self.owner.address_or_object().copied()
    }

    // It's a common pattern to retrieve both the owner and object ID
    // together, if it's owned by a single owner.
    pub fn get_owner_and_id(&self) -> Option<(Owner, ObjectId)> {
        Some((self.owner, self.id()))
    }

    /// Return true if this object is a Move package, false if it is a Move
    /// value
    pub fn is_package(&self) -> bool {
        matches!(&self.data, Data::Package(_))
    }

    pub fn compute_object_reference(&self) -> ObjectRef {
        ObjectRef::new(self.id(), self.version(), self.digest())
    }

    pub fn digest(&self) -> ObjectDigest {
        ObjectDigest::new(default_hash(self))
    }

    pub fn id(&self) -> ObjectId {
        use Data::*;

        match &self.data {
            Struct(v) => v.id(),
            Package(m) => m.id(),
        }
    }

    pub fn version(&self) -> SequenceNumber {
        use Data::*;

        match &self.data {
            Struct(o) => o.version(),
            Package(p) => p.version(),
        }
    }

    pub fn type_(&self) -> Option<&MoveObjectType> {
        self.data.object_type()
    }

    pub fn struct_tag(&self) -> Option<StructTag> {
        self.data.struct_tag()
    }

    pub fn is_coin(&self) -> bool {
        if let Some(move_object) = self.data.as_struct_opt() {
            move_object.struct_tag().is_coin()
        } else {
            false
        }
    }

    pub fn is_gas_coin(&self) -> bool {
        if let Some(move_object) = self.data.as_struct_opt() {
            move_object.struct_tag().is_gas_coin()
        } else {
            false
        }
    }

    // TODO: use `MoveObj::get_balance_unsafe` instead.
    // context: https://github.com/iotaledger/iota/pull/10679#discussion_r1165877816
    pub fn as_coin_maybe(&self) -> Option<Coin> {
        if let Some(move_object) = self.data.as_struct_opt() {
            let coin: Coin = bcs::from_bytes(move_object.contents()).ok()?;
            Some(coin)
        } else {
            None
        }
    }

    pub fn as_timelock_balance_maybe(&self) -> Option<TimeLock<Balance>> {
        if let Some(move_object) = self.data.as_struct_opt() {
            Some(TimeLock::from_bcs_bytes(move_object.contents()).ok()?)
        } else {
            None
        }
    }

    pub fn coin_type_opt(&self) -> Option<&TypeTag> {
        if let Some(move_object) = self.data.as_struct_opt() {
            move_object.struct_tag().coin_type_opt()
        } else {
            None
        }
    }

    /// Return the `value: u64` field of a `Coin<T>` type.
    /// Useful for reading the coin without deserializing the object into a Move
    /// value It is the caller's responsibility to check that `self` is a
    /// coin--this function may panic or do something unexpected otherwise.
    pub fn get_coin_value_unchecked(&self) -> u64 {
        self.data
            .as_struct_opt()
            .unwrap()
            .get_coin_value_unchecked()
    }

    /// Approximate size of the object in bytes. This is used for gas metering.
    /// This will be slightly different from the serialized size, but
    /// we also don't want to serialize the object just to get the size.
    /// This approximation should be good enough for gas metering.
    pub fn object_size_for_gas_metering(&self) -> usize {
        let meta_data_size = size_of::<Owner>() + size_of::<TransactionDigest>() + size_of::<u64>();
        let data_size = match &self.data {
            Data::Struct(m) => m.object_size_for_gas_metering(),
            Data::Package(p) => p.size(),
        };
        meta_data_size + data_size
    }

    /// Change the owner of `self` to `new_owner`.
    pub fn transfer(&mut self, new_owner: IotaAddress) {
        self.owner = Owner::Address(new_owner);
    }

    /// Get a `MoveStructLayout` for `self`.
    /// The `resolver` value must contain the module that declares
    /// `self.object_type` and the (transitive) dependencies of
    /// `self.object_type` in order for this to succeed. Failure will result
    /// in an `ObjectSerializationError`
    pub fn get_layout(
        &self,
        resolver: &impl GetModule,
    ) -> Result<Option<MoveStructLayout>, IotaError> {
        match &self.data {
            Data::Struct(m) => Ok(Some(m.get_layout(resolver)?)),
            Data::Package(_) => Ok(None),
        }
    }

    /// Treat the object type as a Move struct with one type parameter,
    /// like this: `S<T>`.
    /// Returns the inner parameter type `T`.
    pub fn get_move_template_type(&self) -> IotaResult<TypeTag> {
        let move_struct = self.data.struct_tag().ok_or_else(|| IotaError::Type {
            error: "Object must be a Move object".to_owned(),
        })?;
        fp_ensure!(
            move_struct.type_params().len() == 1,
            IotaError::Type {
                error: "Move object struct must have one type parameter".to_owned()
            }
        );
        // Index access safe due to checks above.
        let type_tag = move_struct.type_params()[0].clone();
        Ok(type_tag)
    }

    pub fn to_rust<'de, T: Deserialize<'de>>(&'de self) -> Result<T, bcs::Error> {
        self.data
            .as_struct_opt()
            .ok_or_else(|| bcs::Error::Custom("Object is not a struct".to_string()))?
            .to_rust()
    }
}

// Testing-related APIs.
impl Object {
    /// Get the total amount of IOTA embedded in `self`, including both Move
    /// objects and the storage rebate
    pub fn get_total_iota(
        &self,
        layout_resolver: &mut dyn LayoutResolver,
    ) -> Result<u64, IotaError> {
        Ok(self.storage_rebate
            + match &self.data {
                Data::Struct(m) => m.get_total_iota(layout_resolver)?,
                Data::Package(_) => 0,
            })
    }

    pub fn immutable_with_id_for_testing(id: ObjectId) -> Self {
        let data = Data::Struct(
            MoveObject::new(
                StructTag::new_gas_coin().into(),
                OBJECT_START_VERSION,
                GasCoin::new(id, GAS_VALUE_FOR_TESTING).to_bcs_bytes(),
            )
            .unwrap(),
        );
        ObjectInner {
            owner: Owner::Immutable,
            data,
            previous_transaction: TransactionDigest::GENESIS_MARKER,
            storage_rebate: 0,
        }
        .into()
    }

    pub fn immutable_for_testing() -> Self {
        thread_local! {
            static IMMUTABLE_OBJECT_ID: ObjectId = ObjectId::random();
        }

        Self::immutable_with_id_for_testing(IMMUTABLE_OBJECT_ID.with(|id| *id))
    }

    /// Make a new random test shared object.
    pub fn shared_for_testing() -> Object {
        let id = ObjectId::random();
        let obj = MoveObject::new_gas_coin(OBJECT_START_VERSION, id, 10);
        let owner = Owner::Shared(obj.version());
        Object::new_move(obj, owner, TransactionDigest::GENESIS_MARKER)
    }

    pub fn with_id_owner_gas_for_testing(id: ObjectId, owner: IotaAddress, gas: u64) -> Self {
        let data = Data::Struct(
            MoveObject::new(
                StructTag::new_gas_coin().into(),
                OBJECT_START_VERSION,
                GasCoin::new(id, gas).to_bcs_bytes(),
            )
            .unwrap(),
        );
        ObjectInner {
            owner: Owner::Address(owner),
            data,
            previous_transaction: TransactionDigest::GENESIS_MARKER,
            storage_rebate: 0,
        }
        .into()
    }

    pub fn treasury_cap_for_testing(struct_tag: StructTag, treasury_cap: TreasuryCap) -> Self {
        let data = Data::Struct(
            MoveObject::new(
                StructTag::new_treasury_cap(struct_tag).into(),
                OBJECT_START_VERSION,
                bcs::to_bytes(&treasury_cap).expect("Failed to serialize"),
            )
            .unwrap(),
        );
        ObjectInner {
            owner: Owner::Immutable,
            data,
            previous_transaction: TransactionDigest::GENESIS_MARKER,
            storage_rebate: 0,
        }
        .into()
    }

    pub fn coin_metadata_for_testing(struct_tag: StructTag, metadata: CoinMetadata) -> Self {
        let data = Data::Struct(
            MoveObject::new(
                StructTag::new_coin_metadata(struct_tag).into(),
                OBJECT_START_VERSION,
                bcs::to_bytes(&metadata).expect("Failed to serialize"),
            )
            .unwrap(),
        );
        ObjectInner {
            owner: Owner::Immutable,
            data,
            previous_transaction: TransactionDigest::GENESIS_MARKER,
            storage_rebate: 0,
        }
        .into()
    }

    pub fn with_object_owner_for_testing(id: ObjectId, owner: ObjectId) -> Self {
        let data = Data::Struct(
            MoveObject::new(
                StructTag::new_gas_coin().into(),
                OBJECT_START_VERSION,
                GasCoin::new(id, GAS_VALUE_FOR_TESTING).to_bcs_bytes(),
            )
            .unwrap(),
        );
        ObjectInner {
            owner: Owner::Object(owner),
            data,
            previous_transaction: TransactionDigest::GENESIS_MARKER,
            storage_rebate: 0,
        }
        .into()
    }

    pub fn with_id_owner_for_testing(id: ObjectId, owner: IotaAddress) -> Self {
        // For testing, we provide sufficient gas by default.
        Self::with_id_owner_gas_for_testing(id, owner, GAS_VALUE_FOR_TESTING)
    }

    pub fn with_id_owner_version_for_testing(
        id: ObjectId,
        version: SequenceNumber,
        owner: Owner,
    ) -> Self {
        let data = Data::Struct(
            MoveObject::new(
                StructTag::new_gas_coin().into(),
                version,
                GasCoin::new(id, GAS_VALUE_FOR_TESTING).to_bcs_bytes(),
            )
            .unwrap(),
        );
        ObjectInner {
            owner,
            data,
            previous_transaction: TransactionDigest::GENESIS_MARKER,
            storage_rebate: 0,
        }
        .into()
    }

    pub fn with_owner_for_testing(owner: IotaAddress) -> Self {
        Self::with_id_owner_for_testing(ObjectId::random(), owner)
    }

    /// Generate a new gas coin worth `value` with a random object ID and owner
    /// For testing purposes only
    pub fn new_gas_with_balance_and_owner_for_testing(value: u64, owner: IotaAddress) -> Self {
        let obj = MoveObject::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), value);
        Object::new_move(
            obj,
            Owner::Address(owner),
            TransactionDigest::GENESIS_MARKER,
        )
    }

    /// Generate a new gas coin object with default balance and random owner.
    pub fn new_gas_for_testing() -> Self {
        let gas_object_id = ObjectId::random();
        let (owner, _) = deterministic_random_account_key();
        Object::with_id_owner_for_testing(gas_object_id, owner)
    }
}

/// Make a few test gas objects (all with the same random owner).
pub fn generate_test_gas_objects() -> Vec<Object> {
    thread_local! {
        static GAS_OBJECTS: Vec<Object> = (0..50)
            .map(|_| {
                let gas_object_id = ObjectId::random();
                let (owner, _) = deterministic_random_account_key();
                Object::with_id_owner_for_testing(gas_object_id, owner)
            })
            .collect();
    }

    GAS_OBJECTS.with(|v| v.clone())
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "status", content = "details")]
pub enum ObjectRead {
    NotExists(ObjectId),
    Exists(ObjectRef, Object, Option<MoveStructLayout>),
    Deleted(ObjectRef),
}

impl ObjectRead {
    /// Returns the object value if there is any, otherwise an Err if
    /// the object does not exist or is deleted.
    pub fn into_object(self) -> UserInputResult<Object> {
        match self {
            Self::Deleted(oref) => Err(UserInputError::ObjectDeleted { object_ref: oref }),
            Self::NotExists(id) => Err(UserInputError::ObjectNotFound {
                object_id: id,
                version: None,
            }),
            Self::Exists(_, o, _) => Ok(o),
        }
    }

    pub fn object(&self) -> UserInputResult<&Object> {
        match self {
            Self::Deleted(oref) => Err(UserInputError::ObjectDeleted { object_ref: *oref }),
            Self::NotExists(id) => Err(UserInputError::ObjectNotFound {
                object_id: *id,
                version: None,
            }),
            Self::Exists(_, o, _) => Ok(o),
        }
    }

    pub fn object_id(&self) -> ObjectId {
        match self {
            Self::Deleted(oref) => oref.object_id,
            Self::NotExists(id) => *id,
            Self::Exists(oref, _, _) => oref.object_id,
        }
    }
}

impl Display for ObjectRead {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deleted(oref) => {
                write!(f, "ObjectRead::Deleted ({oref:?})")
            }
            Self::NotExists(id) => {
                write!(f, "ObjectRead::NotExists ({id})")
            }
            Self::Exists(oref, _, _) => {
                write!(f, "ObjectRead::Exists ({oref:?})")
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "status", content = "details")]
pub enum PastObjectRead {
    /// The object does not exist
    ObjectNotExists(ObjectId),
    /// The object is found to be deleted with this version
    ObjectDeleted(ObjectRef),
    /// The object exists and is found with this version
    VersionFound(ObjectRef, Object, Option<MoveStructLayout>),
    /// The object exists but not found with this version
    VersionNotFound(ObjectId, SequenceNumber),
    /// The asked object version is higher than the latest
    VersionTooHigh {
        object_id: ObjectId,
        asked_version: SequenceNumber,
        latest_version: SequenceNumber,
    },
}

impl PastObjectRead {
    /// Returns the object value if there is any, otherwise an Err
    pub fn into_object(self) -> UserInputResult<Object> {
        match self {
            Self::ObjectDeleted(oref) => Err(UserInputError::ObjectDeleted { object_ref: oref }),
            Self::ObjectNotExists(id) => Err(UserInputError::ObjectNotFound {
                object_id: id,
                version: None,
            }),
            Self::VersionFound(_, o, _) => Ok(o),
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

impl Display for PastObjectRead {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ObjectDeleted(oref) => {
                write!(f, "PastObjectRead::ObjectDeleted ({oref:?})")
            }
            Self::ObjectNotExists(id) => {
                write!(f, "PastObjectRead::ObjectNotExists ({id})")
            }
            Self::VersionFound(oref, _, _) => {
                write!(f, "PastObjectRead::VersionFound ({oref:?})")
            }
            Self::VersionNotFound(object_id, version) => {
                write!(
                    f,
                    "PastObjectRead::VersionNotFound ({object_id}, asked sequence number {version:?})"
                )
            }
            Self::VersionTooHigh {
                object_id,
                asked_version,
                latest_version,
            } => {
                write!(
                    f,
                    "PastObjectRead::VersionTooHigh ({object_id}, asked sequence number {asked_version:?}, latest sequence number {latest_version:?})"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::ObjectId;

    use crate::{
        base_types::{IotaAddress, TransactionDigest},
        gas_coin::GasCoin,
        object::{MoveObjectExt, OBJECT_START_VERSION, Object, Owner},
    };

    // Ensure that object digest computation and bcs serialized format are not
    // inadvertently changed.
    #[test]
    fn test_object_digest_and_serialized_format() {
        let g =
            GasCoin::new_for_testing_with_id(ObjectId::ZERO, 123).to_object(OBJECT_START_VERSION);
        let o = Object::new_move(
            g,
            Owner::Address(IotaAddress::ZERO),
            TransactionDigest::ZERO,
        );
        let bytes = bcs::to_bytes(&o).unwrap();

        assert_eq!(
            bytes,
            [
                0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 123, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
        let objref = o.compute_object_reference();

        assert_eq!(objref.object_id, ObjectId::ZERO);
        assert_eq!(objref.version, 1);
        assert_eq!(
            objref.digest.to_string(),
            "Ba4YyVBcpc9jgX4PMLRoyt9dKLftYVSDvuKbtMr9f4NM"
        );
    }

    #[test]
    fn test_get_coin_value_unchecked() {
        fn test_for_value(v: u64) {
            let g = GasCoin::new_for_testing(v).to_object(OBJECT_START_VERSION);
            assert_eq!(g.get_coin_value_unchecked(), v);
            assert_eq!(GasCoin::try_from(&g).unwrap().value(), v);
        }

        test_for_value(0);
        test_for_value(1);
        test_for_value(8);
        test_for_value(9);
        test_for_value(u8::MAX as u64);
        test_for_value(u8::MAX as u64 + 1);
        test_for_value(u16::MAX as u64);
        test_for_value(u16::MAX as u64 + 1);
        test_for_value(u32::MAX as u64);
        test_for_value(u32::MAX as u64 + 1);
        test_for_value(u64::MAX);
    }

    #[test]
    fn test_set_coin_value_unchecked() {
        fn test_for_value(v: u64) {
            let mut g = GasCoin::new_for_testing(u64::MAX).to_object(OBJECT_START_VERSION);
            g.set_coin_value_unchecked(v);
            assert_eq!(g.get_coin_value_unchecked(), v);
            assert_eq!(GasCoin::try_from(&g).unwrap().value(), v);
            assert_eq!(g.version(), OBJECT_START_VERSION);
            assert_eq!(g.contents().len(), 40);
        }

        test_for_value(0);
        test_for_value(1);
        test_for_value(8);
        test_for_value(9);
        test_for_value(u8::MAX as u64);
        test_for_value(u8::MAX as u64 + 1);
        test_for_value(u16::MAX as u64);
        test_for_value(u16::MAX as u64 + 1);
        test_for_value(u32::MAX as u64);
        test_for_value(u32::MAX as u64 + 1);
        test_for_value(u64::MAX);
    }
}
