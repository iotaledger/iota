// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Milestone option types.

mod parameters;

pub use self::parameters::ParametersMilestoneOption;
use crate::block::Error;

/// A milestone option.
#[derive(Clone, Debug, Eq, PartialEq, packable::Packable)]
#[packable(unpack_error = Error)]
#[packable(tag_type = u8, with_error = Error::InvalidMilestoneOptionKind)]
pub enum MilestoneOption {
    /// A parameters milestone option.
    #[packable(tag = ParametersMilestoneOption::KIND)]
    Parameters(ParametersMilestoneOption),
}

impl MilestoneOption {
    /// Return the milestone option kind of a [`MilestoneOption`].
    pub fn kind(&self) -> u8 {
        match self {
            Self::Parameters(_) => ParametersMilestoneOption::KIND,
        }
    }

    /// Gets a reference to a [`ParametersMilestoneOption`], if any.
    pub fn parameters(&self) -> Option<&ParametersMilestoneOption> {
        match self {
            Self::Parameters(opt) => Some(opt),
        }
    }
}

impl PartialOrd for MilestoneOption {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MilestoneOption {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.kind().cmp(&other.kind())
    }
}
