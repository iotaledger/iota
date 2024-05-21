// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Error types pertaining to deserializing Stardust snapshots
use iota_sdk::types::block::output::{FoundryId, NftId};
use std::convert::Infallible;

use packable::error::UnknownTagError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StardustError {
    #[error("unsupported snapshot version: expected {0}, got {1}")]
    UnsupportedSnapshotVersion(u8, u8),
    #[error("invalid snapshot kind: {0}")]
    InvalidSnapshotKind(u8),
    #[error("block error: {0}")]
    BlockError(#[from] iota_sdk::types::block::Error),
    #[error("{0}")]
    UnknownTag(#[from] UnknownTagError<u8>),
    #[error("cannot convert `FoundryOutput` with `FoundryId` {foundry_id} to `NativeTokenPackageData`: {err}")]
    FoundryConversionError {
        foundry_id: FoundryId,
        err: anyhow::Error,
    },
    #[error("framework packages path not found")]
    FrameworkPackagesPathNotFound,
    #[error("failed to derive valid move identifier from symbol `{symbol}`, invalid identifier: `{identifier}`")]
    InvalidMoveIdentifierDerived { symbol: String, identifier: String },
    #[error("Nft Output Immutable Metadata not found")]
    NftImmutableMetadataNotFound,
    #[error("cannot convert `Irc27Metadata` from Nft output with `NftId` {nft_id} to `NativeTokenPackageData`: {err}")]
    Irc27ConversionError { nft_id: NftId, err: anyhow::Error },
}

impl From<Infallible> for StardustError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}
