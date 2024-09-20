// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types for use with these tools.

use iota_json::IotaJsonValue;
use iota_json_rpc_types::{IotaData as _, IotaObjectResponse, IotaTypeTag};
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    coin::Coin,
    digests::ObjectDigest,
    gas_coin::GasCoin as IotaGasCoin,
    TypeTag,
};
use move_core_types::{
    account_address::AccountAddress, ident_str, identifier::IdentStr, language_storage::StructTag,
    u256::U256,
};
use serde::Serialize;

use crate::ClientError;

/// A gas coin.
#[derive(Debug, Clone)]
pub struct GasCoin {
    /// The underlying coin.
    pub coin: Coin,
    /// The object ID.
    pub object_id: ObjectID,
    /// The latest version of the object.
    pub version: SequenceNumber,
    /// The object digest.
    pub digest: ObjectDigest,
}

impl GasCoin {
    /// Get the object reference information.
    pub fn object_ref(&self) -> ObjectRef {
        (self.object_id, self.version, self.digest)
    }
}

impl TryFrom<IotaObjectResponse> for GasCoin {
    type Error = ClientError;

    fn try_from(value: IotaObjectResponse) -> Result<Self, Self::Error> {
        let obj = value.into_object()?;
        let gas = obj
            .bcs
            .as_ref()
            .ok_or_else(|| ClientError::MissingField("bcs"))?
            .try_as_move()
            .ok_or_else(|| ClientError::ParseMoveObject)?
            .deserialize::<IotaGasCoin>()?;
        Ok(Self {
            coin: gas.0,
            object_id: obj.object_id,
            version: obj.version,
            digest: obj.digest,
        })
    }
}

/// A trait for defining a custom move struct in rust code.
/// NOTE: Ideally this type is derived.
pub trait CustomMoveType {
    /// The generic types of this custom type. The empty expression () indicates
    /// that there are no generics.
    type Generics: MoveTypes;
    /// The package address. If this is None, the package provided to the call
    /// will be used.
    const PACKAGE: Option<AccountAddress> = None;
    /// The name of the module in the move package in which this type is
    /// defined.
    const MODULE: &'static IdentStr;
    /// The name of this type in the move package.
    const TYPE_NAME: &'static IdentStr;

    /// Get the type tag from this custom move type.
    fn type_tag(package_id: ObjectID) -> TypeTag {
        TypeTag::Struct(Box::new(StructTag {
            address: Self::PACKAGE.unwrap_or_else(|| AccountAddress::from(package_id)),
            module: Self::MODULE.into(),
            name: Self::TYPE_NAME.into(),
            type_params: Self::Generics::type_tags(package_id),
        }))
    }
}

/// A parameter type.
pub enum ParamType {
    /// An object, referenced by ID.
    Object(ObjectID),
    /// A bcs serialized value.
    Pure(Vec<u8>),
}

/// A trait which defines how types are serialized for move calls.
pub trait MoveParam {
    /// Get the serialized argument.
    fn iota_arg(&self) -> anyhow::Result<IotaJsonValue>;

    /// Get the param type.
    fn param(&self) -> anyhow::Result<ParamType>;
}

/// A trait which defines the iota tag of the type.
pub trait MoveType {
    /// Return the type tag.
    fn type_tag(package_id: ObjectID) -> TypeTag;
}

/// A trait which defines multiple params for use with tuples.
pub trait MoveParams {
    /// Get the aui args.
    fn iota_args(&self) -> anyhow::Result<Vec<IotaJsonValue>> {
        let mut values = Vec::new();
        self.push_iota_args(&mut values)?;
        Ok(values)
    }

    /// Push the iota args onto the list.
    fn push_iota_args(&self, values: &mut Vec<IotaJsonValue>) -> anyhow::Result<()>;
}

/// A trait which defines multiple types for use with tuples.
pub trait MoveTypes {
    /// Get the type tags.
    fn type_tags(package_id: ObjectID) -> Vec<TypeTag> {
        let mut tags = Vec::new();
        Self::push_type_tags(package_id, &mut tags);
        tags
    }

    /// Push the type tags onto the list.
    fn push_type_tags(package_id: ObjectID, tags: &mut Vec<TypeTag>);

    /// Convenience function for getting iota type tags.
    fn iota_type_args(package_id: ObjectID) -> Vec<IotaTypeTag> {
        Self::type_tags(package_id)
            .into_iter()
            .map(IotaTypeTag::from)
            .collect()
    }
}

macro_rules! impl_move_types_tuple {
    ($($tup:ident.$idx:tt),+$(,)?) => {
        impl<$($tup),+> MoveTypes for ($($tup),+)
        where $($tup: MoveTypes),+
        {
            fn push_type_tags(package_id: ObjectID, tags: &mut Vec<TypeTag>) {
                $(
                    $tup::push_type_tags(package_id, tags);
                )+
            }
        }

        impl<$($tup),+> MoveParams for ($($tup),+)
        where $($tup: MoveParams),+
        {
            fn push_iota_args(&self, values: &mut Vec<IotaJsonValue>) -> anyhow::Result<()> {
                $(
                    self.$idx.push_iota_args(values)?;
                )+
                Ok(())
            }
        }
    };
}
impl_move_types_tuple!(T1.0, T2.1);
impl_move_types_tuple!(T1.0, T2.1, T3.2);
impl_move_types_tuple!(T1.0, T2.1, T3.2, T4.3);
impl_move_types_tuple!(T1.0, T2.1, T3.2, T4.3, T5.4);

impl MoveTypes for () {
    fn push_type_tags(_: ObjectID, _: &mut Vec<TypeTag>) {}
}

impl<T: MoveType> MoveTypes for T {
    fn push_type_tags(package_id: ObjectID, tags: &mut Vec<TypeTag>) {
        tags.push(Self::type_tag(package_id))
    }
}

impl<T: MoveParam> MoveParams for T {
    fn push_iota_args(&self, values: &mut Vec<IotaJsonValue>) -> anyhow::Result<()> {
        values.push(self.iota_arg()?);
        Ok(())
    }
}

macro_rules! impl_simple_move_type {
    ($rust_ty:ident, $move_ty:ident) => {
        impl MoveType for $rust_ty {
            fn type_tag(_: ObjectID) -> TypeTag {
                TypeTag::$move_ty
            }
        }

        impl MoveParam for $rust_ty {
            fn iota_arg(&self) -> anyhow::Result<IotaJsonValue> {
                IotaJsonValue::new(serde_json::Value::String(self.to_string()))
            }

            fn param(&self) -> anyhow::Result<ParamType> {
                Ok(ParamType::Pure(bcs::to_bytes(self)?))
            }
        }
    };
}
impl_simple_move_type!(bool, Bool);
impl_simple_move_type!(u8, U8);
impl_simple_move_type!(u16, U16);
impl_simple_move_type!(u32, U32);
impl_simple_move_type!(u64, U64);
impl_simple_move_type!(u128, U128);
impl_simple_move_type!(U256, U256);
impl_simple_move_type!(IotaAddress, Address);

impl<T: MoveType> MoveType for Vec<T> {
    fn type_tag(package_id: ObjectID) -> TypeTag {
        TypeTag::Vector(Box::new(T::type_tag(package_id)))
    }
}

impl<T: CustomMoveType> MoveType for T {
    fn type_tag(package_id: ObjectID) -> TypeTag {
        <T as CustomMoveType>::type_tag(package_id)
    }
}

impl MoveParam for ObjectID {
    fn iota_arg(&self) -> anyhow::Result<IotaJsonValue> {
        Ok(IotaJsonValue::from_object_id(*self))
    }

    fn param(&self) -> anyhow::Result<ParamType> {
        Ok(ParamType::Object(*self))
    }
}

impl MoveParam for () {
    fn iota_arg(&self) -> anyhow::Result<IotaJsonValue> {
        IotaJsonValue::new(serde_json::Value::Null)
    }

    fn param(&self) -> anyhow::Result<ParamType> {
        Ok(ParamType::Pure(Vec::new()))
    }
}

impl<T: Serialize> MoveParam for Vec<T> {
    fn iota_arg(&self) -> anyhow::Result<IotaJsonValue> {
        IotaJsonValue::new(serde_json::to_value(self)?)
    }

    fn param(&self) -> anyhow::Result<ParamType> {
        Ok(ParamType::Pure(bcs::to_bytes(self)?))
    }
}

impl<T: Serialize> MoveParam for Box<T> {
    fn iota_arg(&self) -> anyhow::Result<IotaJsonValue> {
        IotaJsonValue::new(serde_json::to_value(self)?)
    }

    fn param(&self) -> anyhow::Result<ParamType> {
        Ok(ParamType::Pure(bcs::to_bytes(self)?))
    }
}

impl<T: MoveParam> MoveParam for &T {
    fn iota_arg(&self) -> anyhow::Result<IotaJsonValue> {
        (*self).iota_arg()
    }

    fn param(&self) -> anyhow::Result<ParamType> {
        (*self).param()
    }
}

impl<T: MoveTypes> CustomMoveType for Option<T> {
    type Generics = T;

    const PACKAGE: Option<AccountAddress> = Some(AccountAddress::ONE);
    const MODULE: &'static IdentStr = ident_str!("option");
    const TYPE_NAME: &'static IdentStr = ident_str!("Option");
}

/// Helper macro for wrapping possible type trees into Options.
#[macro_export]
macro_rules! opt {
    ($Some:tt) => {
        Some($Some)
    };
    () => {
        None
    };
}

/// Defines a custom move type.
#[macro_export]
macro_rules! move_type {
    (@impl $(($Package:tt))? $Module:tt $Name:ident $(($($Generics:tt)*) ($($Bounds:tt)*))?) => {
        impl $(<$($Bounds)*>)? CustomMoveType for $Name $(<$($Generics)*>)? {
            type Generics = ($($($Generics)*)?);

            const PACKAGE: Option<move_core_types::account_address::AccountAddress> = $crate::opt!($($Package)?);
            const MODULE: &'static move_core_types::identifier::IdentStr = move_core_types::ident_str!(core::stringify!($Module));
            const TYPE_NAME: &'static move_core_types::identifier::IdentStr = move_core_types::ident_str!(core::stringify!($Name));
        }
    };
    (@split $(($Package:tt))? $Module:tt $Name:ident <$($Generic:tt $(: $Bound1:tt $(+ $Bounds:tt)*)?),+>) => {
        move_type!(@impl $(($Package))? $Module $Name ($($Generic),+) ($($Generic $(: $Bound1 $(+ $Bounds)*)?),+));
    };
    (@generics $(($Package:tt))? $Module:tt $Name:ident <$($Generic:tt $(: $Bound1:tt $(+ $Bounds:tt)*)?),+> $($Rest:tt)*) => {
        move_type!(@split $(($Package))? $Module $Name <$($Generic $(: $Bound1 $(+ $Bounds)*)?),+>);
    };
    (@generics $(($Package:tt))? $Module:tt $Name:ident $($Rest:tt)*) => {
        move_type!(@impl $(($Package))? $Module $Name);
    };
    ($(#[$Meta:meta])* $Vis:vis struct $Package:tt :: $Module:tt :: $Name:ident $($Rest:tt)*) => {
        $(#[$Meta])*
        $Vis struct $Name $($Rest)*

        move_type!(@generics ($Package:tt) $Module $Name $($Rest)*);
    };
    ($(#[$Meta:meta])* $Vis:vis struct $Module:tt :: $Name:ident $($Rest:tt)*) => {
        $(#[$Meta])*
        $Vis struct $Name $($Rest)*

        move_type!(@generics $Module $Name $($Rest)*);
    };
}
