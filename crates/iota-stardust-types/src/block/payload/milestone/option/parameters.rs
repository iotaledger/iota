// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Module describing the parameters milestone option.

use core::ops::RangeInclusive;

use packable::{Packable, bounded::BoundedU16, prefix::BoxedSlicePrefix};

use crate::block::Error;

pub(crate) type BinaryParametersLength = BoundedU16<
    { *ParametersMilestoneOption::BINARY_PARAMETERS_LENGTH_RANGE.start() },
    { *ParametersMilestoneOption::BINARY_PARAMETERS_LENGTH_RANGE.end() },
>;

/// A [`ParametersMilestoneOption`] defines changing protocol parameters
/// starting from a target milestone index.
#[derive(Clone, Debug, Eq, PartialEq, Packable)]
#[packable(unpack_error = Error)]
pub struct ParametersMilestoneOption {
    target_milestone_index: u32,
    protocol_version: u8,
    #[packable(unpack_error_with = |err| Error::InvalidBinaryParametersLength(err.into_prefix_err()))]
    binary_parameters: BoxedSlicePrefix<u8, BinaryParametersLength>,
}

impl ParametersMilestoneOption {
    /// The milestone option kind of a [`ParametersMilestoneOption`].
    pub const KIND: u8 = 1;
    /// Valid lengths for binary parameters.
    pub const BINARY_PARAMETERS_LENGTH_RANGE: RangeInclusive<u16> = 0..=8192;

    /// Creates a new [`ParametersMilestoneOption`].
    pub fn new(
        target_milestone_index: u32,
        protocol_version: u8,
        binary_parameters: impl Into<Box<[u8]>>,
    ) -> Result<Self, Error> {
        let binary_parameters_box = binary_parameters.into();
        let len = binary_parameters_box.len();
        if !Self::BINARY_PARAMETERS_LENGTH_RANGE.contains(&(len as u16)) {
            return Err(Error::InvalidBinaryParametersLengthValue(len));
        }
        Ok(Self {
            target_milestone_index,
            protocol_version,
            // SAFETY: We checked the length above
            binary_parameters: binary_parameters_box.try_into().unwrap(),
        })
    }

    /// Returns the target milestone index of a [`ParametersMilestoneOption`].
    pub fn target_milestone_index(&self) -> u32 {
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
