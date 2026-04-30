// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    convert::{TryFrom, TryInto},
    fmt,
    str::FromStr,
};

use anyhow::anyhow;
use fastcrypto::hash::HashFunction;
use iota_protocol_config::ProtocolConfig;
pub use iota_sdk_types::{Identifier, StructTag, TypeTag};
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{
    account_address::AccountAddress, annotated_value as A, ident_str, identifier::IdentStr,
    language_storage::ModuleId,
};
use serde::{
    Deserialize, Serialize, Serializer,
    ser::{Error, SerializeSeq},
};

use crate::{
    MOVE_STDLIB_ADDRESS,
    account_abstraction::authenticator_function::AuthenticatorFunctionRefV1,
    crypto::{
        AuthorityPublicKeyBytes, DefaultHash, IotaPublicKey, IotaSignature, PublicKey,
        SignatureScheme,
    },
    dynamic_field::{DynamicFieldInfo, DynamicFieldType},
    effects::{TransactionEffects, TransactionEffectsAPI},
    epoch_data::EpochData,
    error::{ExecutionError, ExecutionErrorKind, IotaError, IotaResult},
    gas_coin::GAS,
    id::RESOLVED_IOTA_ID,
    iota_sdk_types_conversions::struct_tag_sdk_to_core,
    iota_serde::to_iota_struct_tag_string,
    messages_checkpoint::CheckpointTimestamp,
    multisig::MultiSigPublicKey,
    object::{Object, Owner},
    parse_iota_struct_tag,
    signature::GenericSignature,
    stardust::output::{AliasOutput, BasicOutput, Nft, NftOutput},
    timelock::timelock::{self},
    transaction::{Transaction, VerifiedTransaction},
};
pub use crate::{
    committee::EpochId,
    digests::{ObjectDigest, TransactionDigest, TransactionEffectsDigest},
};

#[cfg(test)]
#[path = "unit_tests/base_types_tests.rs"]
mod base_types_tests;

pub use iota_sdk_types::{
    ObjectId as ObjectID, ObjectReference as ObjectRef, Version as SequenceNumber,
};

pub type TxSequenceNumber = u64;

pub type VersionNumber = SequenceNumber;

/// The round number.
pub type CommitRound = u64;

pub type AuthorityName = AuthorityPublicKeyBytes;

pub trait ConciseableName<'a> {
    type ConciseTypeRef: std::fmt::Debug;
    type ConciseType: std::fmt::Debug;

    fn concise(&'a self) -> Self::ConciseTypeRef;
    fn concise_owned(&self) -> Self::ConciseType;
}

pub type VersionDigest = (SequenceNumber, ObjectDigest);

pub fn random_object_ref() -> ObjectRef {
    ObjectRef::new(
        ObjectID::random(),
        SequenceNumber::default(),
        ObjectDigest::new([0; 32]),
    )
}

/// Wrapper around StructTag with a space-efficient representation for common
/// types like coins The StructTag for a gas coin is 84 bytes, so using 1 byte
/// instead is a win. The inner representation is private to prevent incorrectly
/// constructing an `Other` instead of one of the specialized variants, e.g.
/// `Other(StructTag::new_gas_coin())` instead of `GasCoin`
#[derive(Eq, PartialEq, PartialOrd, Ord, Debug, Clone, Deserialize, Serialize, Hash)]
pub struct MoveObjectType(MoveObjectType_);

/// Even though it is declared public, it is the "private", internal
/// representation for `MoveObjectType`
#[derive(Eq, PartialEq, PartialOrd, Ord, Debug, Clone, Deserialize, Serialize, Hash)]
pub enum MoveObjectType_ {
    /// A type that is not `0x2::coin::Coin<T>`
    Other(Box<StructTag>),
    /// An IOTA coin (i.e., `0x2::coin::Coin<0x2::iota::IOTA>`)
    GasCoin,
    /// A record of a staked IOTA coin (i.e., `0x3::staking_pool::StakedIota`)
    StakedIota,
    /// A non-IOTA coin type (i.e., `0x2::coin::Coin<T> where T !=
    /// 0x2::iota::IOTA`)
    Coin(TypeTag),
    // NOTE: if adding a new type here, and there are existing on-chain objects of that
    // type with Other(_), that is ok, but you must hand-roll PartialEq/Eq/Ord/maybe Hash
    // to make sure the new type and Other(_) are interpreted consistently.
}

impl MoveObjectType {
    pub fn gas_coin() -> Self {
        Self(MoveObjectType_::GasCoin)
    }

    pub fn coin(coin_type: TypeTag) -> Self {
        Self(if GAS::is_gas_type(&coin_type) {
            MoveObjectType_::GasCoin
        } else {
            MoveObjectType_::Coin(coin_type)
        })
    }

    pub fn staked_iota() -> Self {
        Self(MoveObjectType_::StakedIota)
    }

    pub fn timelocked_iota_balance() -> Self {
        Self(MoveObjectType_::Other(Box::new(StructTag::new_time_lock(
            StructTag::new_balance(StructTag::new_gas()),
        ))))
    }

    pub fn timelocked_staked_iota() -> Self {
        Self(MoveObjectType_::Other(Box::new(
            StructTag::new_timelocked_staked_iota(),
        )))
    }

    pub fn stardust_nft() -> Self {
        Self(MoveObjectType_::Other(Box::new(Nft::tag())))
    }

    pub fn address(&self) -> IotaAddress {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::Coin(_) => IotaAddress::FRAMEWORK,
            MoveObjectType_::StakedIota => IotaAddress::SYSTEM,
            MoveObjectType_::Other(s) => s.address(),
        }
    }

    pub fn module(&self) -> Identifier {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::Coin(_) => Identifier::COIN_MODULE,
            MoveObjectType_::StakedIota => Identifier::STAKING_POOL_MODULE,
            MoveObjectType_::Other(s) => s.module().clone(),
        }
    }

    pub fn name(&self) -> Identifier {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::Coin(_) => Identifier::COIN,
            MoveObjectType_::StakedIota => Identifier::STAKED_IOTA,
            MoveObjectType_::Other(s) => s.name().clone(),
        }
    }

    pub fn type_params(&self) -> Vec<TypeTag> {
        match &self.0 {
            MoveObjectType_::GasCoin => vec![GAS::type_tag()],
            MoveObjectType_::StakedIota => vec![],
            MoveObjectType_::Coin(inner) => vec![inner.clone()],
            MoveObjectType_::Other(s) => s.type_params().to_vec(),
        }
    }

    pub fn into_type_params(self) -> Vec<TypeTag> {
        match self.0 {
            MoveObjectType_::GasCoin => vec![GAS::type_tag()],
            MoveObjectType_::StakedIota => vec![],
            MoveObjectType_::Coin(inner) => vec![inner],
            MoveObjectType_::Other(s) => s.type_params().to_vec(),
        }
    }

    pub fn coin_type_maybe(&self) -> Option<TypeTag> {
        match &self.0 {
            MoveObjectType_::GasCoin => Some(GAS::type_tag()),
            MoveObjectType_::Coin(inner) => Some(inner.clone()),
            MoveObjectType_::StakedIota => None,
            MoveObjectType_::Other(_) => None,
        }
    }

    pub fn module_id(&self) -> ModuleId {
        ModuleId::new(
            AccountAddress::new(self.address().into_bytes()),
            move_core_types::identifier::Identifier::new(self.module().as_str()).unwrap(),
        )
    }

    pub fn size_for_gas_metering(&self) -> usize {
        // unwraps safe because a `StructTag` cannot fail to serialize
        match &self.0 {
            MoveObjectType_::GasCoin => 1,
            MoveObjectType_::StakedIota => 1,
            MoveObjectType_::Coin(inner) => bcs::serialized_size(inner).unwrap() + 1,
            MoveObjectType_::Other(s) => bcs::serialized_size(s).unwrap() + 1,
        }
    }

    /// Return true if `self` is `0x2::coin::Coin<T>` for some T (note: T can be
    /// IOTA)
    pub fn is_coin(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::Coin(_) => true,
            MoveObjectType_::StakedIota | MoveObjectType_::Other(_) => false,
        }
    }

    /// Return true if `self` is 0x2::coin::Coin<0x2::iota::IOTA>
    pub fn is_gas_coin(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin => true,
            MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) | MoveObjectType_::Other(_) => {
                false
            }
        }
    }

    /// Return true if `self` is `0x2::coin::Coin<t>`
    pub fn is_coin_t(&self, t: &TypeTag) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin => GAS::is_gas_type(t),
            MoveObjectType_::Coin(c) => t == c,
            MoveObjectType_::StakedIota | MoveObjectType_::Other(_) => false,
        }
    }

    pub fn is_staked_iota(&self) -> bool {
        match &self.0 {
            MoveObjectType_::StakedIota => true,
            MoveObjectType_::GasCoin | MoveObjectType_::Coin(_) | MoveObjectType_::Other(_) => {
                false
            }
        }
    }

    pub fn is_coin_metadata(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => s.is_coin_metadata(),
        }
    }

    pub fn is_coin_manager(&self) -> bool {
        matches!(&self.0, MoveObjectType_::Other(struct_tag) if struct_tag.is_coin_manager())
    }

    pub fn is_treasury_cap(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => s.is_treasury_cap(),
        }
    }

    pub fn is_regulated_coin_metadata(&self) -> bool {
        self.address() == IotaAddress::FRAMEWORK
            && self.module() == Identifier::COIN_MODULE
            && self.name() == Identifier::from_static("RegulatedCoinMetadata")
    }

    pub fn is_coin_deny_cap_v1(&self) -> bool {
        self.address() == IotaAddress::FRAMEWORK
            && self.module() == Identifier::COIN_MODULE
            && self.name() == Identifier::from_static("DenyCapV1")
    }

    pub fn is_dynamic_field(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => DynamicFieldInfo::is_dynamic_field(s),
        }
    }

    pub fn is_timelock(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => s.is_time_lock(),
        }
    }

    pub fn is_timelocked_balance(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => timelock::is_timelocked_balance(s),
        }
    }

    pub fn is_timelocked_staked_iota(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => s.is_timelocked_staked_iota(),
        }
    }

    pub fn is_alias_output(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => AliasOutput::is_alias_output(s),
        }
    }

    pub fn is_basic_output(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => BasicOutput::is_basic_output(s),
        }
    }

    pub fn is_nft_output(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => NftOutput::is_nft_output(s),
        }
    }

    pub fn is_authenticator_function_ref_v1(&self) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                false
            }
            MoveObjectType_::Other(s) => {
                AuthenticatorFunctionRefV1::is_authenticator_function_ref_v1(s)
            }
        }
    }

    pub fn try_extract_field_name(&self, type_: &DynamicFieldType) -> IotaResult<TypeTag> {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                Err(IotaError::ObjectDeserialization {
                    error: "Error extracting dynamic object name from Coin object".to_string(),
                })
            }
            MoveObjectType_::Other(s) => DynamicFieldInfo::try_extract_field_name(s, type_),
        }
    }

    pub fn try_extract_field_value(&self) -> IotaResult<TypeTag> {
        match &self.0 {
            MoveObjectType_::GasCoin | MoveObjectType_::StakedIota | MoveObjectType_::Coin(_) => {
                Err(IotaError::ObjectDeserialization {
                    error: "Error extracting dynamic object value from Coin object".to_string(),
                })
            }
            MoveObjectType_::Other(s) => DynamicFieldInfo::try_extract_field_value(s),
        }
    }

    pub fn is(&self, s: &StructTag) -> bool {
        match &self.0 {
            MoveObjectType_::GasCoin => s.is_gas_coin(),
            MoveObjectType_::StakedIota => s.is_staked_iota(),
            MoveObjectType_::Coin(inner) => s.is_coin() && inner == &s.type_params()[0],
            MoveObjectType_::Other(o) => s == o.as_ref(),
        }
    }

    pub fn other(&self) -> Option<&StructTag> {
        if let MoveObjectType_::Other(s) = &self.0 {
            Some(s)
        } else {
            None
        }
    }

    /// Returns the string representation of this object's type using the
    /// canonical display.
    pub fn to_canonical_string(&self, with_prefix: bool) -> String {
        StructTag::from(self.clone()).to_canonical_string(with_prefix)
    }
}

impl From<&StructTag> for MoveObjectType {
    fn from(s: &StructTag) -> Self {
        Self(if s.is_gas_coin() {
            MoveObjectType_::GasCoin
        } else if s.is_coin() {
            let [type_param] = s.type_params() else {
                unreachable!("a coin has exactly one type parameter");
            };
            MoveObjectType_::Coin(type_param.clone())
        } else if s.is_staked_iota() {
            MoveObjectType_::StakedIota
        } else {
            MoveObjectType_::Other(Box::new(s.clone()))
        })
    }
}

impl From<StructTag> for MoveObjectType {
    fn from(s: StructTag) -> Self {
        Self(if s.is_gas_coin() {
            MoveObjectType_::GasCoin
        } else if s.is_coin() {
            let Some(type_param) = s.into_parts().3.into_iter().next() else {
                unreachable!("a coin has exactly one type parameter");
            };
            MoveObjectType_::Coin(type_param)
        } else if s.is_staked_iota() {
            MoveObjectType_::StakedIota
        } else {
            MoveObjectType_::Other(Box::new(s))
        })
    }
}

impl From<MoveObjectType> for StructTag {
    fn from(t: MoveObjectType) -> Self {
        match t.0 {
            MoveObjectType_::GasCoin => StructTag::new_gas_coin(),
            MoveObjectType_::StakedIota => StructTag::new_staked_iota(),
            MoveObjectType_::Coin(inner) => StructTag::new_coin(inner),
            MoveObjectType_::Other(s) => *s,
        }
    }
}

impl From<MoveObjectType> for TypeTag {
    fn from(o: MoveObjectType) -> TypeTag {
        let s: StructTag = o.into();
        TypeTag::Struct(Box::new(s))
    }
}

/// Whether this type is valid as a primitive (pure) transaction input.
pub fn is_primitive_type_tag(t: &TypeTag) -> bool {
    use TypeTag as T;

    match t {
        T::Bool | T::U8 | T::U16 | T::U32 | T::U64 | T::U128 | T::U256 | T::Address => true,
        T::Vector(inner) => is_primitive_type_tag(inner),
        T::Struct(st) => {
            let resolved_struct = (
                &AccountAddress::new(st.address().into_bytes()),
                move_core_types::identifier::IdentStr::new(st.module().as_str()).unwrap(),
                move_core_types::identifier::IdentStr::new(st.name().as_str()).unwrap(),
            );
            // is id or..
            if resolved_struct == RESOLVED_IOTA_ID {
                return true;
            }
            // is utf8 string
            if resolved_struct == RESOLVED_UTF8_STR {
                return true;
            }
            // is ascii string
            if resolved_struct == RESOLVED_ASCII_STR {
                return true;
            }
            // is option of a primitive
            resolved_struct == RESOLVED_STD_OPTION
                && st.type_params().len() == 1
                && is_primitive_type_tag(&st.type_params()[0])
        }
        T::Signer => false,
    }
}

/// Type of an IOTA object
#[derive(Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum ObjectType {
    /// Move package containing one or more bytecode modules
    Package,
    /// A Move struct of the given type
    Struct(MoveObjectType),
}

impl From<&Object> for ObjectType {
    fn from(o: &Object) -> Self {
        o.data
            .type_()
            .map(|t| ObjectType::Struct(t.clone()))
            .unwrap_or(ObjectType::Package)
    }
}

impl TryFrom<ObjectType> for StructTag {
    type Error = anyhow::Error;

    fn try_from(o: ObjectType) -> Result<Self, anyhow::Error> {
        match o {
            ObjectType::Package => Err(anyhow!("Cannot create StructTag from Package")),
            ObjectType::Struct(move_object_type) => Ok(move_object_type.into()),
        }
    }
}

impl FromStr for ObjectType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.to_lowercase() == PACKAGE {
            Ok(ObjectType::Package)
        } else {
            let tag = parse_iota_struct_tag(s)?;
            Ok(ObjectType::Struct(MoveObjectType::from(tag)))
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct ObjectInfo {
    pub object_id: ObjectID,
    pub version: SequenceNumber,
    pub digest: ObjectDigest,
    pub type_: ObjectType,
    pub owner: Owner,
    pub previous_transaction: TransactionDigest,
}

impl ObjectInfo {
    pub fn new(oref: &ObjectRef, o: &Object) -> Self {
        Self {
            object_id: oref.object_id,
            version: oref.version,
            digest: oref.digest,
            type_: o.into(),
            owner: o.owner,
            previous_transaction: o.previous_transaction,
        }
    }

    pub fn from_object(object: &Object) -> Self {
        Self {
            object_id: object.id(),
            version: object.version(),
            digest: object.digest(),
            type_: object.into(),
            owner: object.owner,
            previous_transaction: object.previous_transaction,
        }
    }
}
const PACKAGE: &str = "package";
impl ObjectType {
    pub fn is_gas_coin(&self) -> bool {
        matches!(self, ObjectType::Struct(s) if s.is_gas_coin())
    }

    pub fn is_coin(&self) -> bool {
        matches!(self, ObjectType::Struct(s) if s.is_coin())
    }

    /// Return true if `self` is `0x2::coin::Coin<t>`
    pub fn is_coin_t(&self, t: &TypeTag) -> bool {
        matches!(self, ObjectType::Struct(s) if s.is_coin_t(t))
    }

    pub fn is_package(&self) -> bool {
        matches!(self, ObjectType::Package)
    }
}

impl From<ObjectInfo> for ObjectRef {
    fn from(info: ObjectInfo) -> Self {
        ObjectRef::new(info.object_id, info.version, info.digest)
    }
}

impl From<&ObjectInfo> for ObjectRef {
    fn from(info: &ObjectInfo) -> Self {
        ObjectRef::new(info.object_id, info.version, info.digest)
    }
}

pub const IOTA_ADDRESS_LENGTH: usize = ObjectID::LENGTH;

pub use iota_sdk_types::Address as IotaAddress;

pub fn address_from_iota_pub_key<T: IotaPublicKey>(pk: &T) -> IotaAddress {
    let mut hasher = DefaultHash::default();
    T::SIGNATURE_SCHEME.update_hasher_with_flag(&mut hasher);
    hasher.update(pk);
    let g_arr = hasher.finalize();
    IotaAddress::new(g_arr.digest)
}

impl From<&PublicKey> for IotaAddress {
    fn from(pk: &PublicKey) -> Self {
        let mut hasher = DefaultHash::default();
        pk.scheme().update_hasher_with_flag(&mut hasher);
        hasher.update(pk);
        let g_arr = hasher.finalize();
        IotaAddress::new(g_arr.digest)
    }
}

impl From<&MultiSigPublicKey> for IotaAddress {
    /// Derive a IotaAddress from [struct MultiSigPublicKey]. A MultiSig address
    /// is defined as the 32-byte Blake2b hash of serializing the flag, the
    /// threshold, concatenation of all n flag, public keys and
    /// its weight. `flag_MultiSig || threshold || flag_1 || pk_1 || weight_1
    /// || ... || flag_n || pk_n || weight_n`.
    fn from(multisig_pk: &MultiSigPublicKey) -> Self {
        let mut hasher = DefaultHash::default();
        hasher.update([SignatureScheme::MultiSig.flag()]);
        hasher.update(multisig_pk.threshold().to_le_bytes());
        multisig_pk.pubkeys().iter().for_each(|(pk, w)| {
            pk.scheme().update_hasher_with_flag(&mut hasher);
            hasher.update(pk.as_ref());
            hasher.update(w.to_le_bytes());
        });
        IotaAddress::new(hasher.finalize().digest)
    }
}

impl TryFrom<&GenericSignature> for IotaAddress {
    type Error = IotaError;
    /// Derive a IotaAddress from a serialized signature in IOTA
    /// [GenericSignature].
    fn try_from(sig: &GenericSignature) -> IotaResult<Self> {
        match sig {
            GenericSignature::Signature(sig) => {
                let scheme = sig.scheme();
                let pub_key_bytes = sig.public_key_bytes();
                let pub_key = PublicKey::try_from_bytes(scheme, pub_key_bytes).map_err(|_| {
                    IotaError::InvalidSignature {
                        error: "Cannot parse pubkey".to_string(),
                    }
                })?;
                Ok(IotaAddress::from(&pub_key))
            }
            GenericSignature::MultiSig(ms) => Ok(ms.get_pk().into()),
            #[allow(deprecated)]
            GenericSignature::ZkLoginAuthenticatorDeprecated(_) => {
                Err(IotaError::UnsupportedFeature {
                    error: "zkLogin is not supported".to_string(),
                })
            }
            GenericSignature::PasskeyAuthenticator(s) => Ok(IotaAddress::from(&s.get_pk()?)),
            GenericSignature::MoveAuthenticator(move_authenticator) => move_authenticator.address(),
        }
    }
}

/// Generate a fake IotaAddress with repeated one byte.
pub fn dbg_addr(name: u8) -> IotaAddress {
    let addr = [name; IOTA_ADDRESS_LENGTH];
    IotaAddress::new(addr)
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Serialize, Deserialize, Debug)]
pub struct ExecutionDigests {
    pub transaction: TransactionDigest,
    pub effects: TransactionEffectsDigest,
}

impl ExecutionDigests {
    pub fn new(transaction: TransactionDigest, effects: TransactionEffectsDigest) -> Self {
        Self {
            transaction,
            effects,
        }
    }

    pub fn random() -> Self {
        Self {
            transaction: TransactionDigest::random(),
            effects: TransactionEffectsDigest::random(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct ExecutionData {
    pub transaction: Transaction,
    pub effects: TransactionEffects,
}

impl ExecutionData {
    pub fn new(transaction: Transaction, effects: TransactionEffects) -> ExecutionData {
        debug_assert_eq!(transaction.digest(), effects.transaction_digest());
        Self {
            transaction,
            effects,
        }
    }

    pub fn digests(&self) -> ExecutionDigests {
        self.effects.execution_digests()
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct VerifiedExecutionData {
    pub transaction: VerifiedTransaction,
    pub effects: TransactionEffects,
}

impl VerifiedExecutionData {
    pub fn new(transaction: VerifiedTransaction, effects: TransactionEffects) -> Self {
        debug_assert_eq!(transaction.digest(), effects.transaction_digest());
        Self {
            transaction,
            effects,
        }
    }

    pub fn new_unchecked(data: ExecutionData) -> Self {
        Self {
            transaction: VerifiedTransaction::new_unchecked(data.transaction),
            effects: data.effects,
        }
    }

    pub fn into_inner(self) -> ExecutionData {
        ExecutionData {
            transaction: self.transaction.into_inner(),
            effects: self.effects,
        }
    }

    pub fn digests(&self) -> ExecutionDigests {
        self.effects.execution_digests()
    }
}

pub const RESOLVED_STD_OPTION: (&AccountAddress, &IdentStr, &IdentStr) = (
    &MOVE_STDLIB_ADDRESS,
    ident_str!("option"),
    ident_str!("Option"),
);

pub const RESOLVED_ASCII_STR: (&AccountAddress, &IdentStr, &IdentStr) = (
    &MOVE_STDLIB_ADDRESS,
    ident_str!("ascii"),
    ident_str!("String"),
);

pub const RESOLVED_UTF8_STR: (&AccountAddress, &IdentStr, &IdentStr) = (
    &MOVE_STDLIB_ADDRESS,
    ident_str!("string"),
    ident_str!("String"),
);

pub fn move_ascii_str_layout() -> A::MoveStructLayout {
    A::MoveStructLayout {
        type_: struct_tag_sdk_to_core(&StructTag::new_ascii_string()),
        fields: vec![A::MoveFieldLayout::new(
            ident_str!("bytes").into(),
            A::MoveTypeLayout::Vector(Box::new(A::MoveTypeLayout::U8)),
        )],
    }
}

pub fn move_utf8_str_layout() -> A::MoveStructLayout {
    A::MoveStructLayout {
        type_: struct_tag_sdk_to_core(&StructTag::new_string()),
        fields: vec![A::MoveFieldLayout::new(
            ident_str!("bytes").into(),
            A::MoveTypeLayout::Vector(Box::new(A::MoveTypeLayout::U8)),
        )],
    }
}

pub fn url_layout() -> A::MoveStructLayout {
    A::MoveStructLayout {
        type_: struct_tag_sdk_to_core(&StructTag::new_url()),
        fields: vec![A::MoveFieldLayout::new(
            ident_str!("url").to_owned(),
            A::MoveTypeLayout::Struct(Box::new(move_ascii_str_layout())),
        )],
    }
}

// The Rust representation of the Move `TxContext`.
// This struct must be kept in sync with the Move `TxContext` definition.
// Moving forward we are going to zero all fields of the Move `TxContext`
// and use native functions to retrieve info about the transaction.
// However we cannot remove the Move type and so this struct is going to
// be the Rust equivalent to the Move `TxContext` for legacy usages.
//
// `TxContext` in Rust (see below) is going to be purely used in Rust and can
// evolve as needed without worrying about any compatibility with Move.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MoveLegacyTxContext {
    // Signer/sender of the transaction
    sender: AccountAddress,
    // Digest of the current transaction
    digest: Vec<u8>,
    // The current epoch number
    epoch: EpochId,
    // Timestamp that the epoch started at
    epoch_timestamp_ms: CheckpointTimestamp,
    // Number of `ObjectID`'s generated during execution of the current transaction
    ids_created: u64,
}

impl From<&TxContext> for MoveLegacyTxContext {
    fn from(tx_context: &TxContext) -> Self {
        Self {
            sender: tx_context.sender,
            digest: tx_context.digest.clone(),
            epoch: tx_context.epoch,
            epoch_timestamp_ms: tx_context.epoch_timestamp_ms,
            ids_created: tx_context.ids_created,
        }
    }
}

// Information about the transaction context.
// This struct is not related to Move and can evolve as needed/required.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TxContext {
    /// Signer/sender of the transaction
    sender: AccountAddress,
    /// Digest of the current transaction
    digest: Vec<u8>,
    /// The current epoch number
    epoch: EpochId,
    /// Timestamp that the epoch started at
    epoch_timestamp_ms: CheckpointTimestamp,
    /// Number of `ObjectID`'s generated during execution of the current
    /// transaction
    ids_created: u64,
    // Reference gas price
    rgp: u64,
    /// Gas price passed to transaction as input
    gas_price: u64,
    /// Gas budget passed to transaction as input
    gas_budget: u64,
    /// Address of the sponsor if any (gas owner != sender)
    sponsor: Option<AccountAddress>,
    /// Whether the `TxContext` is native or not (i.e., Move reads values via
    /// native functions instead of struct fields).
    is_native: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum TxContextKind {
    // No TxContext
    None,
    // &mut TxContext
    Mutable,
    // &TxContext
    Immutable,
}

impl TxContext {
    pub fn new(
        sender: &IotaAddress,
        digest: &TransactionDigest,
        epoch_data: &EpochData,
        rgp: u64,
        gas_price: u64,
        gas_budget: u64,
        sponsor: Option<IotaAddress>,
        protocol_config: &ProtocolConfig,
    ) -> Self {
        Self::new_from_components(
            sender,
            digest,
            &epoch_data.epoch_id(),
            epoch_data.epoch_start_timestamp(),
            rgp,
            gas_price,
            gas_budget,
            sponsor,
            protocol_config,
        )
    }

    pub fn new_from_components(
        sender: &IotaAddress,
        digest: &TransactionDigest,
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        rgp: u64,
        gas_price: u64,
        gas_budget: u64,
        sponsor: Option<IotaAddress>,
        protocol_config: &ProtocolConfig,
    ) -> Self {
        Self {
            sender: AccountAddress::new(sender.into_bytes()),
            digest: digest.into_inner().to_vec(),
            epoch: *epoch_id,
            epoch_timestamp_ms,
            ids_created: 0,
            rgp,
            gas_price,
            gas_budget,
            sponsor: sponsor.map(|s| AccountAddress::new(s.into_bytes())),
            is_native: protocol_config.move_native_tx_context(),
        }
    }

    /// Returns whether the type signature is &mut TxContext, &TxContext, or
    /// none of the above.
    pub fn kind(view: &CompiledModule, s: &SignatureToken) -> TxContextKind {
        use SignatureToken as S;
        let (kind, s) = match s {
            S::MutableReference(s) => (TxContextKind::Mutable, s),
            S::Reference(s) => (TxContextKind::Immutable, s),
            _ => return TxContextKind::None,
        };

        let S::Datatype(idx) = &**s else {
            return TxContextKind::None;
        };

        let (module_addr, module_name, struct_name) = resolve_struct(view, *idx);
        let is_tx_context_type = module_name.as_str() == Identifier::TX_CONTEXT_MODULE.as_str()
            && module_addr.as_ref() == IotaAddress::FRAMEWORK.as_bytes()
            && struct_name.as_str() == Identifier::TX_CONTEXT.as_str();

        if is_tx_context_type {
            kind
        } else {
            TxContextKind::None
        }
    }

    pub fn epoch(&self) -> EpochId {
        self.epoch
    }

    pub fn epoch_timestamp_ms(&self) -> u64 {
        self.epoch_timestamp_ms
    }

    /// Return the transaction digest, to include in new objects
    pub fn digest(&self) -> TransactionDigest {
        TransactionDigest::new(self.digest.clone().try_into().unwrap())
    }

    pub fn sponsor(&self) -> Option<IotaAddress> {
        self.sponsor.map(|a| IotaAddress::from(a.into_bytes()))
    }

    pub fn rgp(&self) -> u64 {
        self.rgp
    }

    pub fn gas_price(&self) -> u64 {
        self.gas_price
    }

    pub fn gas_budget(&self) -> u64 {
        self.gas_budget
    }

    pub fn ids_created(&self) -> u64 {
        self.ids_created
    }

    /// Derive a globally unique object ID by hashing self.digest |
    /// self.ids_created
    pub fn fresh_id(&mut self) -> ObjectID {
        let id = ObjectID::derive_id(self.digest(), self.ids_created);
        self.ids_created += 1;
        id
    }

    pub fn sender(&self) -> IotaAddress {
        IotaAddress::new(self.sender.into_bytes())
    }

    pub fn to_vec(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }

    /// Serialize this context as a `MoveLegacyTxContext`. When `is_native` is
    /// true, all fields except digest are zeroed (Move reads actual values via
    /// native functions). When false, actual field values are used.
    pub fn to_bcs_legacy_context(&self) -> Vec<u8> {
        let move_context: MoveLegacyTxContext = if self.is_native {
            let tx_context = &TxContext {
                sender: AccountAddress::ZERO,
                digest: vec![],
                epoch: 0,
                epoch_timestamp_ms: 0,
                ids_created: 0,
                rgp: 0,
                gas_price: 0,
                gas_budget: 0,
                sponsor: None,
                is_native: true,
            };
            tx_context.into()
        } else {
            self.into()
        };
        bcs::to_bytes(&move_context).unwrap()
    }

    /// Updates state of the context instance. It's intended to use
    /// when mutable context is passed over some boundary via
    /// serialize/deserialize and this is the reason why this method
    /// consumes the other context.
    pub fn update_state(&mut self, other: MoveLegacyTxContext) -> Result<(), ExecutionError> {
        if !self.is_native {
            if self.sender != other.sender
                || self.digest != other.digest
                || other.ids_created < self.ids_created
            {
                return Err(ExecutionError::new_with_source(
                    ExecutionErrorKind::InvariantViolation,
                    "Immutable fields for TxContext changed",
                ));
            }
            self.ids_created = other.ids_created;
        }
        Ok(())
    }

    /// Replace all fields. Used by Move test-only native functions.
    pub fn replace(
        &mut self,
        sender: AccountAddress,
        tx_hash: Vec<u8>,
        epoch: u64,
        epoch_timestamp_ms: u64,
        ids_created: u64,
        rgp: u64,
        gas_price: u64,
        gas_budget: u64,
        sponsor: Option<AccountAddress>,
    ) {
        self.sender = sender;
        self.digest = tx_hash;
        self.epoch = epoch;
        self.epoch_timestamp_ms = epoch_timestamp_ms;
        self.ids_created = ids_created;
        self.rgp = rgp;
        self.gas_price = gas_price;
        self.gas_budget = gas_budget;
        self.sponsor = sponsor;
    }

    // Generate a random TxContext for testing.
    pub fn random_for_testing_only() -> Self {
        Self::new(
            &IotaAddress::random(),
            &TransactionDigest::random(),
            &EpochData::new_test(),
            0,
            0,
            0,
            None,
            &ProtocolConfig::get_for_max_version_UNSAFE(),
        )
    }
}

/// Generate a fake ObjectID with repeated one byte.
pub fn dbg_object_id(name: u8) -> ObjectID {
    ObjectID::new([name; ObjectID::LENGTH])
}

#[derive(PartialEq, Eq, Clone, Debug, thiserror::Error)]
pub enum ObjectIDParseError {
    #[error("ObjectID hex literal must start with 0x")]
    HexLiteralPrefixMissing,

    #[error("Could not convert from bytes slice")]
    TryFromSlice,
}

impl fmt::Display for MoveObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        let s: StructTag = self.clone().into();
        write!(
            f,
            "{}",
            to_iota_struct_tag_string(&s).map_err(fmt::Error::custom)?
        )
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectType::Package => write!(f, "{PACKAGE}"),
            ObjectType::Struct(t) => write!(f, "{t}"),
        }
    }
}

// SizeOneVec is a wrapper around Vec<T> that enforces the size of the vec to be
// 1. This seems pointless, but it allows us to have fields in protocol messages
// that are current enforced to be of size 1, but might later allow other sizes,
// and to have that constraint enforced in the serialization/deserialization
// layer, instead of requiring manual input validation.
#[derive(Debug, Deserialize, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[serde(try_from = "Vec<T>")]
pub struct SizeOneVec<T> {
    e: T,
}

impl<T> SizeOneVec<T> {
    pub fn new(e: T) -> Self {
        Self { e }
    }

    pub fn element(&self) -> &T {
        &self.e
    }

    pub fn element_mut(&mut self) -> &mut T {
        &mut self.e
    }

    pub fn into_inner(self) -> T {
        self.e
    }

    pub fn iter(&self) -> std::iter::Once<&T> {
        std::iter::once(&self.e)
    }
}

impl<T> Serialize for SizeOneVec<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(1))?;
        seq.serialize_element(&self.e)?;
        seq.end()
    }
}

impl<T> TryFrom<Vec<T>> for SizeOneVec<T> {
    type Error = anyhow::Error;

    fn try_from(mut v: Vec<T>) -> Result<Self, Self::Error> {
        if v.len() != 1 {
            Err(anyhow!("Expected a vec of size 1"))
        } else {
            Ok(SizeOneVec {
                e: v.pop().unwrap(),
            })
        }
    }
}

#[test]
fn test_size_one_vec_is_transparent() {
    let regular = vec![42u8];
    let size_one = SizeOneVec::new(42u8);

    // Vec -> SizeOneVec serialization is transparent
    let regular_ser = bcs::to_bytes(&regular).unwrap();
    let size_one_deser = bcs::from_bytes::<SizeOneVec<u8>>(&regular_ser).unwrap();
    assert_eq!(size_one, size_one_deser);

    // other direction works too
    let size_one_ser = bcs::to_bytes(&SizeOneVec::new(43u8)).unwrap();
    let regular_deser = bcs::from_bytes::<Vec<u8>>(&size_one_ser).unwrap();
    assert_eq!(regular_deser, vec![43u8]);

    // we get a deserialize error when deserializing a vec with size != 1
    let empty_ser = bcs::to_bytes(&Vec::<u8>::new()).unwrap();
    bcs::from_bytes::<SizeOneVec<u8>>(&empty_ser).unwrap_err();

    let size_greater_than_one_ser = bcs::to_bytes(&vec![1u8, 2u8]).unwrap();
    bcs::from_bytes::<SizeOneVec<u8>>(&size_greater_than_one_ser).unwrap_err();
}
