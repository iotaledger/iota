// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::fmt;

use crate::{
    Transport,
    api::{
        constants, errors, helpers,
        packable::{Error as PackableError, Packable, Read, Unpackable, Write},
    },
};

#[derive(Debug)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Unpackable for Version {
    fn unpack<R: Read>(buf: &mut R) -> Result<Self, PackableError>
    where
        Self: Sized,
    {
        let major = u8::unpack(buf)?;
        let minor = u8::unpack(buf)?;
        let patch = u8::unpack(buf)?;

        // consume all extra bytes (app name)
        while u8::unpack(buf).is_ok() {
            // NOP
        }

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

struct VersionRequest {}
impl Packable for VersionRequest {
    fn packed_len(&self) -> usize {
        0 // No data to pack
    }

    fn pack<W: Write>(&self, _buf: &mut W) -> Result<(), PackableError> {
        Ok(()) // No data to pack
    }
}

pub fn exec<T: Transport>(transport: &T) -> Result<Version, errors::LedgerError> {
    helpers::send_with_blocks::<T, Version>(
        transport,
        constants::APDUInstructions::GetVersion,
        vec![Box::new(VersionRequest {})],
        None,
    )
}
