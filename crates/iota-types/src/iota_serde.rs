// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt,
    fmt::{Debug, Display, Formatter, Write},
    marker::PhantomData,
    ops::Deref,
    str::FromStr,
};

use iota_protocol_config::ProtocolVersion;
use serde::{
    self, Deserialize, Serialize,
    de::{Deserializer, Error},
    ser::{Error as SerError, Serializer},
};
use serde_with::{Bytes, DeserializeAs, DisplayFromStr, SerializeAs, serde_as};

use crate::{
    base_types::{IotaAddress, StructTag, TypeTag},
    parse_iota_struct_tag, parse_iota_type_tag,
};

#[inline]
pub(crate) fn to_custom_deser_error<'de, D, E>(e: E) -> D::Error
where
    E: Debug,
    D: Deserializer<'de>,
{
    Error::custom(format!("byte deserialization failed, cause by: {e:?}"))
}

#[inline]
pub(crate) fn to_custom_ser_error<S, E>(e: E) -> S::Error
where
    E: Debug,
    S: Serializer,
{
    S::Error::custom(format!("byte serialization failed, cause by: {e:?}"))
}

/// Use with serde_as to control serde for human-readable serialization and
/// deserialization `H` : serde_as SerializeAs/DeserializeAs delegation for
/// human readable in/output `R` : serde_as SerializeAs/DeserializeAs delegation
/// for non-human readable in/output
///
/// # Example:
///
/// ```text
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct Example(#[serde_as(as = "Readable<DisplayFromStr, _>")] [u8; 20]);
/// ```
///
/// The above example will delegate human-readable serde to `DisplayFromStr`
/// and array tuple (default) for non-human-readable serializer.
pub struct Readable<H, R> {
    human_readable: PhantomData<H>,
    non_human_readable: PhantomData<R>,
}

impl<T: ?Sized, H, R> SerializeAs<T> for Readable<H, R>
where
    H: SerializeAs<T>,
    R: SerializeAs<T>,
{
    fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            H::serialize_as(value, serializer)
        } else {
            R::serialize_as(value, serializer)
        }
    }
}

impl<'de, R, H, T> DeserializeAs<'de, T> for Readable<H, R>
where
    H: DeserializeAs<'de, T>,
    R: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            H::deserialize_as(deserializer)
        } else {
            R::deserialize_as(deserializer)
        }
    }
}

pub struct IotaStructTag;

impl SerializeAs<StructTag> for IotaStructTag {
    fn serialize_as<S>(value: &StructTag, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let f = to_iota_struct_tag_string(value).map_err(S::Error::custom)?;
        f.serialize(serializer)
    }
}

const IOTA_ADDRESSES: [IotaAddress; 7] = [
    IotaAddress::ZERO,
    IotaAddress::STD,
    IotaAddress::FRAMEWORK,
    IotaAddress::SYSTEM,
    IotaAddress::STARDUST,
    IotaAddress::SYSTEM_STATE,
    IotaAddress::CLOCK,
];
/// Serialize StructTag as a string, retaining the leading zeros in the address.
pub fn to_iota_struct_tag_string(value: &StructTag) -> Result<String, fmt::Error> {
    let mut f = String::new();
    let address = value.address();
    // trim leading zeros if address is in IOTA_ADDRESSES
    let address_str = if IOTA_ADDRESSES.contains(&address) {
        address.to_short_hex()
    } else {
        address.to_canonical_string(/* with_prefix */ true)
    };

    write!(f, "{}::{}::{}", address_str, value.module(), value.name())?;
    if let Some(first_ty) = value.type_params().first() {
        write!(f, "<")?;
        write!(f, "{}", to_iota_type_tag_string(first_ty)?)?;
        for ty in value.type_params().iter().skip(1) {
            write!(f, ", {}", to_iota_type_tag_string(ty)?)?;
        }
        write!(f, ">")?;
    }
    Ok(f)
}

fn to_iota_type_tag_string(value: &TypeTag) -> Result<String, fmt::Error> {
    match value {
        TypeTag::Vector(t) => Ok(format!("vector<{}>", to_iota_type_tag_string(t)?)),
        TypeTag::Struct(s) => to_iota_struct_tag_string(s),
        _ => Ok(value.to_string()),
    }
}

impl<'de> DeserializeAs<'de, StructTag> for IotaStructTag {
    fn deserialize_as<D>(deserializer: D) -> Result<StructTag, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_iota_struct_tag(&s).map_err(D::Error::custom)
    }
}

pub struct IotaTypeTag;

impl SerializeAs<TypeTag> for IotaTypeTag {
    fn serialize_as<S>(value: &TypeTag, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = to_iota_type_tag_string(value).map_err(S::Error::custom)?;
        s.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, TypeTag> for IotaTypeTag {
    fn deserialize_as<D>(deserializer: D) -> Result<TypeTag, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_iota_type_tag(&s).map_err(D::Error::custom)
    }
}

/// A marker for type tags that are serialized as strings. Normally, a
/// type tag is serialized as a string for readable formats, and as a byte array
/// for non-readable formats. This marker can be used to serialize a type tag as
/// a string even in non-readable formats.
pub struct TypeName;

impl SerializeAs<TypeTag> for TypeName {
    fn serialize_as<S>(value: &TypeTag, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = value.to_canonical_string(false);
        s.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, TypeTag> for TypeName {
    fn deserialize_as<D>(deserializer: D) -> Result<TypeTag, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_iota_type_tag(&s).map_err(D::Error::custom)
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy)]
pub struct BigInt<T>(#[serde_as(as = "DisplayFromStr")] T)
where
    T: Display + FromStr,
    <T as FromStr>::Err: Display;

impl<T> BigInt<T>
where
    T: Display + FromStr,
    <T as FromStr>::Err: Display,
{
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> SerializeAs<T> for BigInt<T>
where
    T: Display + FromStr + Copy,
    <T as FromStr>::Err: Display,
{
    fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        BigInt(*value).serialize(serializer)
    }
}

impl<'de, T> DeserializeAs<'de, T> for BigInt<T>
where
    T: Display + FromStr + Copy,
    <T as FromStr>::Err: Display,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(*BigInt::deserialize(deserializer)?)
    }
}

impl<T> From<T> for BigInt<T>
where
    T: Display + FromStr,
    <T as FromStr>::Err: Display,
{
    fn from(v: T) -> BigInt<T> {
        BigInt(v)
    }
}

impl<T> Deref for BigInt<T>
where
    T: Display + FromStr,
    <T as FromStr>::Err: Display,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Display for BigInt<T>
where
    T: Display + FromStr,
    <T as FromStr>::Err: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy)]
pub struct SequenceNumber(u64);

impl SerializeAs<crate::base_types::SequenceNumber> for SequenceNumber {
    fn serialize_as<S>(
        value: &crate::base_types::SequenceNumber,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = value.to_string();
        s.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, crate::base_types::SequenceNumber> for SequenceNumber {
    fn deserialize_as<D>(deserializer: D) -> Result<crate::base_types::SequenceNumber, D::Error>
    where
        D: Deserializer<'de>,
    {
        let b = BigInt::deserialize(deserializer)?;
        Ok(crate::base_types::SequenceNumber::from_u64(*b))
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy)]
#[serde(rename = "ProtocolVersion")]
pub struct AsProtocolVersion(u64);

impl SerializeAs<ProtocolVersion> for AsProtocolVersion {
    fn serialize_as<S>(value: &ProtocolVersion, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = value.as_u64().to_string();
        s.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, ProtocolVersion> for AsProtocolVersion {
    fn deserialize_as<D>(deserializer: D) -> Result<ProtocolVersion, D::Error>
    where
        D: Deserializer<'de>,
    {
        let b = BigInt::<u64>::deserialize(deserializer)?;
        Ok(ProtocolVersion::from(*b))
    }
}

/// Serializes and deserializes a RoaringBitmap with its own on-disk standard.
/// <https://github.com/RoaringBitmap/RoaringFormatSpec>
pub(crate) struct IotaBitmap;

impl SerializeAs<roaring::RoaringBitmap> for IotaBitmap {
    fn serialize_as<S>(source: &roaring::RoaringBitmap, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut bytes = vec![];

        source
            .serialize_into(&mut bytes)
            .map_err(to_custom_ser_error::<S, _>)?;
        Bytes::serialize_as(&bytes, serializer)
    }
}

impl<'de> DeserializeAs<'de, roaring::RoaringBitmap> for IotaBitmap {
    fn deserialize_as<D>(deserializer: D) -> Result<roaring::RoaringBitmap, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = Bytes::deserialize_as(deserializer)?;
        deserialize_iota_bitmap(&bytes).map_err(to_custom_deser_error::<'de, D, _>)
    }
}

// Deserializes a RoaringBitmap.
// NOTE: roaring 0.11+ validates container key ordering in deserialize_from(),
// so bitmaps with unsorted/duplicate container keys are already rejected at the
// deserialization level.
fn deserialize_iota_bitmap(bytes: &[u8]) -> std::io::Result<roaring::RoaringBitmap> {
    roaring::RoaringBitmap::deserialize_from(bytes)
}

#[cfg(test)]
mod test {
    use base64::Engine as _;

    use super::*;

    #[test]
    fn test_iota_bitmap_rejects_unsorted_keys() {
        // This base64 encodes a malformed roaring bitmap with unsorted/duplicate
        // container keys. roaring 0.11+ rejects such bitmaps at deserialization.
        let raw = "OjAAAAoAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWAAAAFoAAABcAAAAXgAAAGAAAABiAAAAZAAAAGYAAABoAAAAagAAAAEAAQABAAEAAQABAAEAAQABAAEA";
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw)
            .unwrap();

        assert!(deserialize_iota_bitmap(&bytes[..]).is_err());
    }

    #[test]
    fn test_iota_bitmap_valid_roundtrip() {
        // Build a valid bitmap, serialize it, then deserialize via our function.
        let mut original = roaring::RoaringBitmap::new();
        original.insert(1);
        original.insert(42);
        original.insert(100);

        let mut bytes = vec![];
        original.serialize_into(&mut bytes).unwrap();

        let result = deserialize_iota_bitmap(&bytes[..]).unwrap();
        assert_eq!(result.len(), 3);
        let values: Vec<u32> = result.iter().collect();
        assert_eq!(values, vec![1, 42, 100]);
    }
}
