// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use fastcrypto::hash::{Digest, HashFunction, Sha256};
use ledger_transport::APDUCommand;

use crate::{
    Transport,
    api::{
        constants,
        errors::{self},
        packable::{Error as PackableError, Packable, PackableObject, Read, Unpackable, Write},
    },
};

/// Macro to create a vector of boxed packable objects
/// Usage: packable_vec![payload1, payload2, payload3]
#[macro_export]
macro_rules! packable_vec {
    ($($payload:expr),* $(,)?) => {
        vec![$(Box::new($payload) as Box<dyn $crate::api::packable::PackableObject>),*]
    };
}

#[derive(Default, Debug)]
pub(crate) struct PackedBIP32Path {
    data: Vec<u8>,
}

impl Packable for PackedBIP32Path {
    fn packed_len(&self) -> usize {
        self.data.len()
    }

    fn pack<W: Write>(&self, buf: &mut W) -> Result<(), PackableError> {
        buf.write_all(&self.data)?;
        Ok(())
    }
}

impl From<&bip32::DerivationPath> for PackedBIP32Path {
    fn from(path: &bip32::DerivationPath) -> Self {
        let mut data = Vec::with_capacity(path.len() * 4 + 1);
        data.push(path.len() as u8);
        for index in path.iter() {
            data.extend_from_slice(&index.0.to_le_bytes());
        }
        PackedBIP32Path { data }
    }
}

#[derive(Debug, Clone, Copy)]
enum LedgerToHost {
    ResultAccumulating = 0,
    ResultFinal = 1,
    GetChunk = 2,
    PutChunk = 3,
}

impl TryFrom<u8> for LedgerToHost {
    type Error = PackableError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(LedgerToHost::ResultAccumulating),
            1 => Ok(LedgerToHost::ResultFinal),
            2 => Ok(LedgerToHost::GetChunk),
            3 => Ok(LedgerToHost::PutChunk),
            _ => Err(PackableError::InvalidVariant),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum HostToLedger {
    Start = 0,
    GetChunkResponseSuccess = 1,
    GetChunkResponseFailure = 2,
    PutChunkResponse = 3,
    ResultAccumulatingResponse = 4,
}

impl HostToLedger {
    fn as_vec(self) -> Vec<u8> {
        vec![self as u8]
    }
}

#[derive(Debug)]
struct BlockResponse {
    instruction: LedgerToHost,
    payload: Vec<u8>,
}

impl BlockResponse {
    fn chunk_hash(&self) -> Result<Digest<32>, errors::LedgerError> {
        match self.instruction {
            LedgerToHost::GetChunk => {
                if self.payload.len() >= 32 {
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&self.payload[..32]);
                    return Ok(Digest::<32>::new(hash));
                }
                Err(errors::LedgerError::BlocksProtocolFailed)
            }
            LedgerToHost::PutChunk => Ok(Sha256::digest(&self.payload)),
            _ => Err(errors::LedgerError::BlocksProtocolFailed),
        }
    }
}

impl Unpackable for BlockResponse {
    fn unpack<R: Read>(buf: &mut R) -> Result<Self, PackableError> {
        let instruction =
            LedgerToHost::try_from(u8::unpack(buf)?).map_err(|_| PackableError::InvalidVariant)?;

        let mut payload = Vec::new();
        buf.read_to_end(&mut payload)?;
        Ok(Self {
            instruction,
            payload,
        })
    }
}

pub(crate) fn send_with_blocks<T: Transport, R: Unpackable>(
    transport: &T,
    ins: constants::APDUInstructions,
    payloads: Vec<Box<dyn PackableObject>>,
    extra_data: Option<HashMap<Digest<32>, Vec<u8>>>,
) -> Result<R, errors::LedgerError> {
    const CHUNK_SIZE: usize = 180;

    let mut data = extra_data.unwrap_or_default();
    let mut parameter_list: Vec<Digest<32>> = Vec::new();

    for payload in payloads {
        let packed = payload
            .pack_as_vec()
            .map_err(|_| errors::LedgerError::Serialization)?;
        let chunks: Vec<&[u8]> = packed.chunks(CHUNK_SIZE).collect();

        let mut last_hash: Digest<32> = Digest::<32>::new([0u8; 32]);
        for chunk in chunks.iter().rev() {
            let mut linked_chunk = Vec::with_capacity(32 + chunk.len());
            linked_chunk.extend(last_hash.to_vec());
            linked_chunk.extend_from_slice(chunk);

            last_hash = Sha256::digest(&linked_chunk);
            data.insert(last_hash, linked_chunk);
        }

        parameter_list.push(last_hash);
    }

    let mut initial_payload = vec![HostToLedger::Start as u8];
    for param in parameter_list {
        initial_payload.extend(&param.to_vec());
    }

    handle_blocks_protocol(transport, ins, initial_payload, data)
}

fn handle_blocks_protocol<T: Transport, U: Unpackable>(
    transport: &T,
    ins: constants::APDUInstructions,
    mut payload: Vec<u8>,
    mut data: HashMap<Digest<32>, Vec<u8>>,
) -> Result<U, errors::LedgerError> {
    let mut result = Vec::new();
    let ins = ins as u8;

    loop {
        let cmd = APDUCommand {
            cla: constants::APDU_CLA,
            ins,
            p1: constants::APDU_P1,
            p2: constants::APDU_P2,
            data: payload,
        };

        let rv = exec::<T, BlockResponse>(transport, cmd)?;

        match rv.instruction {
            LedgerToHost::ResultAccumulating => {
                result.extend(rv.payload);
                payload = HostToLedger::ResultAccumulatingResponse.as_vec();
            }
            LedgerToHost::ResultFinal => {
                result.extend(rv.payload);
                break;
            }
            LedgerToHost::GetChunk => {
                let key = rv.chunk_hash()?;
                payload = if let Some(chunk) = data.get(&key) {
                    let mut resp = HostToLedger::GetChunkResponseSuccess.as_vec();
                    resp.extend_from_slice(chunk);
                    resp
                } else {
                    vec![HostToLedger::GetChunkResponseFailure as u8]
                };
            }
            LedgerToHost::PutChunk => {
                data.insert(rv.chunk_hash()?, rv.payload);
                payload = HostToLedger::PutChunkResponse.as_vec();
            }
        }
    }

    let res = U::unpack(&mut &result[..]).map_err(|_| errors::LedgerError::Serialization)?;
    Ok(res)
}

pub(crate) fn exec<T: Transport, U: Unpackable>(
    transport: &T,
    cmd: APDUCommand<Vec<u8>>,
) -> Result<U, errors::LedgerError> {
    transport.exchange(&cmd).and_then(|resp| {
        let api_error = errors::LedgerError::get_error(resp.retcode());
        match api_error {
            None => {
                let res = U::unpack(&mut &resp.data()[..])
                    .map_err(|_| errors::LedgerError::Serialization)?;
                Ok(res)
            }
            Some(e) => Err(e),
        }
    })
}
