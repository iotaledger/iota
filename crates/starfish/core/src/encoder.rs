// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use bytes::Bytes;
pub(crate) use reed_solomon_simd::ReedSolomonEncoder;

use crate::{block_header::Shard, context::Context, error::ConsensusError};

/// Trait for encoding data into shards using systematic coding with
/// configurable redundancy.
pub trait ShardEncoder {
    /// Systematically encodes `data` by adding `parity_length` shards.
    /// The length of `data` must be equal to `info_length`.
    fn encode_shards(
        &mut self,
        data: Vec<Shard>,
        info_length: usize,
        parity_length: usize,
    ) -> Result<Vec<Shard>, ConsensusError>;

    /// Serializes and encodes transactions into a vector of shards using an
    /// error-correcting code with a dimension of `info_length` and
    /// redundancy of `parity_length`.
    fn encode_serialized_data(
        &mut self,
        serialized_transactions: &Bytes,
        info_length: usize,
        parity_length: usize,
    ) -> Result<Vec<Shard>, ConsensusError>;
}

impl ShardEncoder for ReedSolomonEncoder {
    fn encode_shards(
        &mut self,
        mut data: Vec<Shard>,
        info_length: usize,
        parity_length: usize,
    ) -> Result<Vec<Shard>, ConsensusError> {
        assert_eq!(
            data.len(),
            info_length,
            "Data length must match info length"
        );
        assert!(info_length > 0, "Info length must be greater than 0");
        let shard_bytes = data[0].len();
        self.reset(info_length, parity_length, shard_bytes)
            .map_err(|e| ConsensusError::EncoderResetFailed(e.to_string()))?;
        for shard in data.clone() {
            self.add_original_shard(shard)
                .map_err(|e| ConsensusError::AddShardFailed(e.to_string()))?;
        }
        let result = self
            .encode()
            .map_err(|e| ConsensusError::ShardsEncodingFailed(e.to_string()))?;
        let recovery: Vec<Shard> = result.recovery_iter().map(|slice| slice.to_vec()).collect();
        data.extend(recovery);
        Ok(data)
    }

    fn encode_serialized_data(
        &mut self,
        serialized: &Bytes,
        info_length: usize,
        parity_length: usize,
    ) -> Result<Vec<Shard>, ConsensusError> {
        let data = create_shards_from_serialized_transactions(serialized, info_length);
        self.encode_shards(data, info_length, parity_length)
    }
}

pub(crate) struct TrivialEncoder {}
impl ShardEncoder for TrivialEncoder {
    fn encode_shards(
        &mut self,
        data: Vec<Shard>,
        info_length: usize,
        _parity_length: usize,
    ) -> Result<Vec<Shard>, ConsensusError> {
        assert_eq!(
            data.len(),
            info_length,
            "Data length must match info length"
        );
        assert!(info_length > 0, "Info length must be greater than 0");
        Ok(data)
    }

    fn encode_serialized_data(
        &mut self,
        serialized: &Bytes,
        info_length: usize,
        parity_length: usize,
    ) -> Result<Vec<Shard>, ConsensusError> {
        let data = create_shards_from_serialized_transactions(serialized, info_length);
        self.encode_shards(data, info_length, parity_length)
    }
}

/// Creates shards from serialized transactions, padding as necessary to
/// ensure each shard is of equal length. The number of shards created is
/// equal to `info_length`.
fn create_shards_from_serialized_transactions(
    serialized: &Bytes,
    info_length: usize,
) -> Vec<Shard> {
    let bytes_length = serialized.len();
    let mut statements_with_len: Vec<u8> = (bytes_length as u32).to_le_bytes().to_vec();
    statements_with_len.extend_from_slice(serialized);
    // increase the length by 4 for u32
    let mut shard_bytes = (bytes_length + 4).div_ceil(info_length);

    // Ensure shard_bytes meets alignment requirements.
    if shard_bytes % 2 != 0 {
        shard_bytes += 1;
    }

    let length_with_padding = shard_bytes * info_length;
    statements_with_len.resize(length_with_padding, 0);

    let data: Vec<Shard> = statements_with_len
        .chunks(shard_bytes)
        .map(|chunk| chunk.to_vec())
        .collect();
    data
}
pub(crate) fn create_encoder(context: &Arc<Context>) -> Box<dyn ShardEncoder + Send + Sync> {
    let info_length = context.committee.info_length();
    let parity_length = context.committee.size() - info_length;
    let encoder: Box<dyn ShardEncoder + Send + Sync> = if info_length > 0 && parity_length > 0 {
        Box::new(
            ReedSolomonEncoder::new(info_length, parity_length, 2)
                .expect("We should expect correct creation of the ReedSolomonEncoder"),
        )
    } else {
        Box::new(TrivialEncoder {})
    };
    encoder
}
