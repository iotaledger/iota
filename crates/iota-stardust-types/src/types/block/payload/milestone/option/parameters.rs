// Copyright 2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Module describing the parameters milestone option.

use alloc::boxed::Box;
use core::ops::RangeInclusive;

use packable::{Packable, bounded::BoundedU16, prefix::BoxedSlicePrefix};

use crate::types::block::{Error, payload::milestone::MilestoneIndex};

pub(crate) type BinaryParametersLength = BoundedU16<
    { *ParametersMilestoneOption::BINARY_PARAMETERS_LENGTH_RANGE.start() },
    { *ParametersMilestoneOption::BINARY_PARAMETERS_LENGTH_RANGE.end() },
>;

///
#[derive(Clone, Debug, Eq, PartialEq, Packable)]
#[packable(unpack_error = Error)]
pub struct ParametersMilestoneOption {
    // The milestone index at which these protocol parameters become active.
    target_milestone_index: MilestoneIndex,
    // The protocol version.
    protocol_version: u8,
    // The protocol parameters in binary form.
    #[packable(unpack_error_with = |err| Error::InvalidBinaryParametersLength(err.into_prefix_err().into()))]
    binary_parameters: BoxedSlicePrefix<u8, BinaryParametersLength>,
}

impl ParametersMilestoneOption {
    /// The milestone option kind of a [`ParametersMilestoneOption`].
    pub const KIND: u8 = 1;
    /// Valid lengths for binary parameters.
    pub const BINARY_PARAMETERS_LENGTH_RANGE: RangeInclusive<u16> = 0..=8192;

    /// Creates a new [`ParametersMilestoneOption`].
    pub fn new(
        target_milestone_index: MilestoneIndex,
        protocol_version: u8,
        binary_parameters: impl Into<Box<[u8]>>,
    ) -> Result<Self, Error> {
        Ok(Self {
            target_milestone_index,
            protocol_version,
            binary_parameters: binary_parameters
                .into()
                .try_into()
                .map_err(Error::InvalidBinaryParametersLength)?,
        })
    }

    /// Returns the target milestone index of a [`ParametersMilestoneOption`].
    pub fn target_milestone_index(&self) -> MilestoneIndex {
        self.target_milestone_index
    }

    /// Returns the protocol version of a [`ParametersMilestoneOption`].
    pub fn protocol_version(&self) -> u8 {
        self.protocol_version
    }

    /// Returns the binary parameters of a [`ParametersMilestoneOption`].
    pub fn binary_parameters(&self) -> &[u8] {
        &self.binary_parameters
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::{types::block::Error, utils::serde::prefix_hex_bytes};

    ///
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ParametersMilestoneOptionDto {
        #[serde(rename = "type")]
        pub kind: u8,
        pub target_milestone_index: u32,
        pub protocol_version: u8,
        #[serde(rename = "params")]
        #[serde(
            skip_serializing_if = "<[_]>::is_empty",
            default,
            with = "prefix_hex_bytes"
        )]
        pub binary_parameters: Box<[u8]>,
    }

    impl From<&ParametersMilestoneOption> for ParametersMilestoneOptionDto {
        fn from(value: &ParametersMilestoneOption) -> Self {
            Self {
                kind: ParametersMilestoneOption::KIND,
                target_milestone_index: *value.target_milestone_index(),
                protocol_version: value.protocol_version(),
                binary_parameters: value.binary_parameters().into(),
            }
        }
    }

    impl TryFrom<ParametersMilestoneOptionDto> for ParametersMilestoneOption {
        type Error = Error;

        fn try_from(value: ParametersMilestoneOptionDto) -> Result<Self, Self::Error> {
            Self::new(
                value.target_milestone_index.into(),
                value.protocol_version,
                value.binary_parameters,
            )
        }
    }
}
