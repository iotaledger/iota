// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::ed25519::ED25519_PUBLIC_KEY_LENGTH;
use iota_types::{
    base_types::IotaAddress,
    crypto::{PublicKey, SignatureScheme},
};

use crate::{
    Transport,
    api::{
        constants::APDUInstructions,
        errors, helpers,
        packable::{Error as PackableError, Read, Unpackable},
    },
    packable_vec,
};

pub struct PublicKeyResult {
    pub public_key: PublicKey,
    pub address: IotaAddress,
}

impl Unpackable for PublicKeyResult {
    fn unpack<R: Read>(buf: &mut R) -> Result<Self, PackableError>
    where
        Self: Sized,
    {
        if u8::unpack(buf)? != ED25519_PUBLIC_KEY_LENGTH as u8 {
            return Err(PackableError::InvalidAnnouncedLen);
        }
        let mut key = [0_u8; ED25519_PUBLIC_KEY_LENGTH];
        buf.read_exact(&mut key)?;
        let public_key = PublicKey::try_from_bytes(SignatureScheme::ED25519, &key)
            .map_err(|_| PackableError::InvalidData)?;

        if u8::unpack(buf)? != 32 {
            return Err(PackableError::InvalidAnnouncedLen);
        }
        let mut address_buffer = [0_u8; 32];
        buf.read_exact(&mut address_buffer)?;
        let address =
            IotaAddress::from_bytes(address_buffer).map_err(|_| PackableError::InvalidData)?;

        Ok(Self {
            public_key,
            address,
        })
    }
}

pub fn exec<T: Transport>(
    transport: &T,
    bip32: &bip32::DerivationPath,
    show: bool,
) -> Result<PublicKeyResult, errors::LedgerError> {
    let payload: helpers::PackedBIP32Path = bip32.into();
    let ins = if show {
        APDUInstructions::VerifyAddress
    } else {
        APDUInstructions::GetPublicKey
    };
    helpers::send_with_blocks(transport, ins, packable_vec![payload], None)
}
