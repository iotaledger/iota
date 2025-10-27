// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{
    Transport,
    api::{
        constants, errors, helpers,
        helpers::PackedBIP32Path,
        packable::{Error as PackableError, Packable, Read, Unpackable, Write},
    },
    packable_vec,
};

#[derive(Debug)]
pub struct SignatureBytes {
    pub bytes: Vec<u8>,
}

impl Unpackable for SignatureBytes {
    fn unpack<R: Read>(buf: &mut R) -> Result<Self, PackableError>
    where
        Self: Sized,
    {
        let mut bytes = Vec::new();
        buf.read_to_end(&mut bytes)?;
        Ok(Self { bytes })
    }
}

struct TransactionData {
    transaction: Vec<u8>,
}

impl Packable for TransactionData {
    fn packed_len(&self) -> usize {
        0_u32.packed_len() + self.transaction.len()
    }

    fn pack<W: Write>(&self, buf: &mut W) -> Result<(), PackableError> {
        (self.transaction.len() as u32).pack(buf)?;
        buf.write_all(&self.transaction)?;
        Ok(())
    }
}

struct TransactionObjects {
    objects: Vec<Vec<u8>>,
}

impl Packable for TransactionObjects {
    fn packed_len(&self) -> usize {
        0_u32.packed_len()
            + self
                .objects
                .iter()
                .map(|o| 0u32.packed_len() + o.len())
                .sum::<usize>()
    }

    fn pack<W: Write>(&self, buf: &mut W) -> Result<(), PackableError> {
        // Pack the number of objects
        (self.objects.len() as u32).pack(buf)?;

        // Pack each object
        for object in &self.objects {
            // Pack the length of the object
            (object.len() as u32).pack(buf)?;
            // Write the object data
            buf.write_all(object)?;
        }
        Ok(())
    }
}

pub fn exec<T: Transport>(
    transport: &T,
    path: &bip32::DerivationPath,
    transaction: Vec<u8>,
    objects: Vec<Vec<u8>>,
) -> Result<SignatureBytes, errors::LedgerError> {
    let payloads = if objects.is_empty() {
        packable_vec![TransactionData { transaction }, PackedBIP32Path::from(path)]
    } else {
        packable_vec![
            TransactionData { transaction },
            PackedBIP32Path::from(path),
            TransactionObjects { objects }
        ]
    };

    helpers::send_with_blocks(
        transport,
        constants::APDUInstructions::SignTransaction,
        payloads,
        None,
    )
}
