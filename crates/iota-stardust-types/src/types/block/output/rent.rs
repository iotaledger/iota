// Copyright 2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::mem::size_of;

use packable::{
    Packable,
    error::{UnpackError, UnpackErrorExt},
    packer::Packer,
    unpacker::Unpacker,
};

use crate::types::block::{
    BlockId, Error,
    address::{Address, Ed25519Address},
    output::{
        BasicOutputBuilder, NativeTokens, Output, OutputId,
        unlock_condition::{
            AddressUnlockCondition, ExpirationUnlockCondition, StorageDepositReturnUnlockCondition,
        },
    },
    payload::milestone::MilestoneIndex,
};

const DEFAULT_BYTE_COST: u32 = 100;
const DEFAULT_BYTE_COST_FACTOR_KEY: u8 = 10;
const DEFAULT_BYTE_COST_FACTOR_DATA: u8 = 1;

type ConfirmationUnixTimestamp = u32;

/// Specifies the current parameters for the byte cost computation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct RentStructure {
    /// Cost in tokens per virtual byte.
    v_byte_cost: u32,
    /// The weight factor used for key fields in the outputs.
    v_byte_factor_key: u8,
    /// The weight factor used for data fields in the outputs.
    v_byte_factor_data: u8,
}

impl Default for RentStructure {
    fn default() -> Self {
        Self {
            v_byte_cost: DEFAULT_BYTE_COST,
            v_byte_factor_key: DEFAULT_BYTE_COST_FACTOR_KEY,
            v_byte_factor_data: DEFAULT_BYTE_COST_FACTOR_DATA,
        }
    }
}

impl RentStructure {
    /// Creates a new [`RentStructure`].
    pub fn new(byte_cost: u32, byte_factor_key: u8, byte_factor_data: u8) -> Self {
        Self {
            v_byte_cost: byte_cost,
            v_byte_factor_key: byte_factor_key,
            v_byte_factor_data: byte_factor_data,
        }
    }

    /// Sets the byte cost for the storage deposit.
    pub fn with_byte_cost(mut self, byte_cost: u32) -> Self {
        self.v_byte_cost = byte_cost;
        self
    }

    /// Sets the virtual byte weight for the key fields.
    pub fn with_byte_factor_key(mut self, byte_factor_key: u8) -> Self {
        self.v_byte_factor_key = byte_factor_key;
        self
    }

    /// Sets the virtual byte weight for the data fields.
    pub fn with_byte_factor_data(mut self, byte_factor_data: u8) -> Self {
        self.v_byte_factor_data = byte_factor_data;
        self
    }

    /// Returns the byte cost of the [`RentStructure`].
    pub fn byte_cost(&self) -> u32 {
        self.v_byte_cost
    }

    /// Returns the byte factor key of the [`RentStructure`].
    pub fn byte_factor_key(&self) -> u8 {
        self.v_byte_factor_key
    }

    /// Returns the byte factor data of the [`RentStructure`].
    pub fn byte_factor_data(&self) -> u8 {
        self.v_byte_factor_data
    }

    /// Returns the byte offset of the [`RentStructure`].
    pub fn byte_offset(&self) -> u32 {
        size_of::<OutputId>() as u32 * self.v_byte_factor_key as u32
            + size_of::<BlockId>() as u32 * self.v_byte_factor_data as u32
            + size_of::<MilestoneIndex>() as u32 * self.v_byte_factor_data as u32
            + size_of::<ConfirmationUnixTimestamp>() as u32 * self.v_byte_factor_data as u32
    }
}

impl Packable for RentStructure {
    type UnpackError = Error;
    type UnpackVisitor = ();

    fn pack<P: Packer>(&self, packer: &mut P) -> Result<(), P::Error> {
        self.v_byte_cost.pack(packer)?;
        self.v_byte_factor_data.pack(packer)?;
        self.v_byte_factor_key.pack(packer)?;

        Ok(())
    }

    fn unpack<U: Unpacker, const VERIFY: bool>(
        unpacker: &mut U,
        visitor: &Self::UnpackVisitor,
    ) -> Result<Self, UnpackError<Self::UnpackError, U::Error>> {
        let v_byte_cost = u32::unpack::<_, VERIFY>(unpacker, visitor).coerce()?;
        let v_byte_factor_data = u8::unpack::<_, VERIFY>(unpacker, visitor).coerce()?;
        let v_byte_factor_key = u8::unpack::<_, VERIFY>(unpacker, visitor).coerce()?;

        Ok(Self {
            v_byte_cost,
            v_byte_factor_key,
            v_byte_factor_data,
        })
    }
}

/// A trait to facilitate the computation of the byte cost of block outputs,
/// which is central to dust protection.
pub trait Rent {
    /// Different fields in a type lead to different storage requirements for
    /// the ledger state.
    fn weighted_bytes(&self, config: &RentStructure) -> u64;

    /// Computes the rent cost given a [`RentStructure`].
    fn rent_cost(&self, config: &RentStructure) -> u64 {
        config.v_byte_cost as u64 * (self.weighted_bytes(config) + config.byte_offset() as u64)
    }
}

impl<T: Rent, const N: usize> Rent for [T; N] {
    fn weighted_bytes(&self, config: &RentStructure) -> u64 {
        self.iter().map(|elem| elem.weighted_bytes(config)).sum()
    }
}

pub struct MinimumStorageDepositBasicOutput {
    config: RentStructure,
    token_supply: u64,
    builder: BasicOutputBuilder,
}

impl MinimumStorageDepositBasicOutput {
    pub fn new(config: RentStructure, token_supply: u64) -> Self {
        Self {
            config,
            token_supply,
            builder: BasicOutputBuilder::new_with_amount(Output::AMOUNT_MIN).add_unlock_condition(
                AddressUnlockCondition::new(Address::from(Ed25519Address::from(
                    [0; Ed25519Address::LENGTH],
                ))),
            ),
        }
    }

    pub fn with_native_tokens(mut self, native_tokens: impl Into<Option<NativeTokens>>) -> Self {
        if let Some(native_tokens) = native_tokens.into() {
            self.builder = self.builder.with_native_tokens(native_tokens);
        }
        self
    }

    pub fn with_storage_deposit_return(mut self) -> Result<Self, Error> {
        self.builder = self
            .builder
            .add_unlock_condition(StorageDepositReturnUnlockCondition::new(
                Address::from(Ed25519Address::from([0; Ed25519Address::LENGTH])),
                Output::AMOUNT_MIN,
                self.token_supply,
            )?);
        Ok(self)
    }

    pub fn with_expiration(mut self) -> Result<Self, Error> {
        self.builder = self
            .builder
            .add_unlock_condition(ExpirationUnlockCondition::new(
                Address::from(Ed25519Address::from([0; Ed25519Address::LENGTH])),
                1,
            )?);
        Ok(self)
    }

    pub fn finish(self) -> Result<u64, Error> {
        Ok(self
            .builder
            .finish_output(self.token_supply)?
            .rent_cost(&self.config))
    }
}
