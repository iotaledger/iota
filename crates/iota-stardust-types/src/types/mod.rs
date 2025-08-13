// Copyright 2020-2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Common types required by nodes and clients APIs like blocks, responses and
//! DTOs.

#[cfg(feature = "std")]
extern crate std;

pub mod block;

use core::ops::Deref;

use self::block::protocol::ProtocolParameters;

/// Borrowed or Owned. Useful for generic impls and UX.
#[derive(Clone, Debug)]
pub enum Boo<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<T> Boo<'_, T> {
    pub fn into_owned(self) -> T
    where
        T: Clone,
    {
        match self {
            Boo::Borrowed(b) => b.clone(),
            Boo::Owned(o) => o,
        }
    }
}

impl<T> AsRef<T> for Boo<'_, T> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<T> Deref for Boo<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self {
            Boo::Borrowed(b) => b,
            Boo::Owned(ref o) => o,
        }
    }
}

impl<T> From<T> for Boo<'_, T> {
    fn from(value: T) -> Self {
        Self::Owned(value)
    }
}

impl<'a, T> From<&'a T> for Boo<'a, T> {
    fn from(value: &'a T) -> Self {
        Self::Borrowed(value)
    }
}

impl<'a, T> From<&'a Self> for Boo<'a, T> {
    fn from(value: &'a Self) -> Self {
        match *value {
            Self::Borrowed(b) => Self::Borrowed(b),
            Self::Owned(ref o) => Self::Borrowed(o),
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct ValidationParams<'a> {
    protocol_parameters: Option<Boo<'a, ProtocolParameters>>,
    token_supply: Option<u64>,
}

impl<'a> ValidationParams<'a> {
    pub fn with_protocol_parameters(
        mut self,
        protocol_parameters: impl Into<Boo<'a, ProtocolParameters>>,
    ) -> Self {
        let protocol_parameters = protocol_parameters.into();
        let token_supply = protocol_parameters.token_supply();
        self.protocol_parameters.replace(protocol_parameters);
        self.with_token_supply(token_supply)
    }

    pub fn with_token_supply(mut self, token_supply: u64) -> Self {
        self.token_supply.replace(token_supply);
        self
    }

    pub fn protocol_parameters(&self) -> Option<&ProtocolParameters> {
        self.protocol_parameters.as_deref()
    }

    pub fn token_supply(&self) -> Option<u64> {
        self.token_supply
            .or_else(|| self.protocol_parameters.as_ref().map(|p| p.token_supply()))
    }
}

impl From<u64> for ValidationParams<'_> {
    fn from(value: u64) -> Self {
        Self::default().with_token_supply(value)
    }
}

impl From<ProtocolParameters> for ValidationParams<'_> {
    fn from(value: ProtocolParameters) -> Self {
        Self::default().with_protocol_parameters(value)
    }
}

impl<'a> From<&'a ProtocolParameters> for ValidationParams<'a> {
    fn from(value: &'a ProtocolParameters) -> Self {
        Self::default().with_protocol_parameters(value)
    }
}

impl<'a> From<&'a Self> for ValidationParams<'a> {
    fn from(value: &'a Self) -> Self {
        Self {
            protocol_parameters: value.protocol_parameters.as_ref().map(Into::into),
            token_supply: value.token_supply,
        }
    }
}

pub trait TryFromDto: Sized {
    type Dto;
    type Error;

    fn try_from_dto(dto: Self::Dto) -> Result<Self, Self::Error> {
        Self::try_from_dto_with_params(dto, ValidationParams::default())
    }

    fn try_from_dto_with_params<'a>(
        dto: Self::Dto,
        params: impl Into<ValidationParams<'a>> + Send,
    ) -> Result<Self, Self::Error> {
        Self::try_from_dto_with_params_inner(dto, params.into())
    }

    fn try_from_dto_with_params_inner(
        dto: Self::Dto,
        params: ValidationParams<'_>,
    ) -> Result<Self, Self::Error>;
}
