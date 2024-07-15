// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Error types pertaining to deserializing Stardust snapshots
use std::convert::Infallible;

use iota_stardust_sdk::types::block::output::FoundryId;
use packable::error::UnknownTagError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StardustError {
    #[error("unsupported Hornet snapshot version: expected {0}, got {1}")]
    UnsupportedHornetSnapshotVersion(u8, u8),
    #[error("invalid snapshot kind: {0}")]
    InvalidHornetSnapshotKind(u8),
    #[error("invalid Hornet genesis snapshot: milestone diff count must be 0, but was {0}")]
    InvalidHornetGenesisSnapshot(u32),
    #[error("block error: {0}")]
    BlockError(#[from] iota_stardust_sdk::types::block::Error),
    #[error("{0}")]
    UnknownTag(#[from] UnknownTagError<u8>),
    #[error(
        "cannot convert `FoundryOutput` with `FoundryId` {foundry_id} to `NativeTokenPackageData`: {err}"
    )]
    FoundryConversionError {
        foundry_id: FoundryId,
        err: anyhow::Error,
    },
    #[error("framework packages path not found")]
    FrameworkPackagesPathNotFound,
    #[error(
        "failed to derive valid move identifier from symbol `{symbol}`, invalid identifier: `{identifier}`"
    )]
    InvalidMoveIdentifierDerived { symbol: String, identifier: String },
    #[error("melting tokens must not be greater than minted tokens")]
    MeltingTokensMustNotBeGreaterThanMintedTokens,
    #[error("circulating supply must not be greater than maximum supply")]
    CirculatingSupplyMustNotBeGreaterThanMaximumSupply,
    #[error("Hornet Stardust snapshot parameters not found")]
    HornetSnapshotParametersNotFound,
}

impl From<Infallible> for StardustError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}
