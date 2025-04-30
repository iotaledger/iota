// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use reed_solomon_simd::ReedSolomonDecoder;
use starfish_config::Committee;

use crate::{Transaction, block::Shard, error::ConsensusError};

/// Trait for decoding shard collections using systematic Reed-Solomon decoding
/// and reconstructing the original transactions.
pub trait ShardsDecoder {
    /// Attempts to decode a collection of arbitrary >= info_length shards into
    /// a vector of Transactions.
    #[expect(dead_code)]
    fn decode_shards(
        &mut self,
        committee: &Committee,
        shards_collection: Vec<Option<Shard>>,
    ) -> Result<Vec<Transaction>, ConsensusError>;

    /// Reconstructs the original vector of Transactions from a vector of first
    /// info_length shards.
    fn reconstruct_transactions(
        shards: Vec<Shard>,
        info_length: usize,
    ) -> Result<Vec<Transaction>, ConsensusError>;
}

impl ShardsDecoder for ReedSolomonDecoder {
    fn decode_shards(
        &mut self,
        committee: &Committee,
        shards_collection: Vec<Option<Shard>>,
    ) -> Result<Vec<Transaction>, ConsensusError> {
        let info_length = committee.info_length();
        let total_length = committee.size();
        let parity_length = total_length - info_length;
        assert_eq!(
            shards_collection.len(),
            total_length,
            "Shards collection length must match committee size"
        );
        let shards_count = shards_collection.iter().filter(|x| x.is_some()).count();
        if shards_count < info_length {
            return Err(ConsensusError::InsufficientShardsInDecoder(
                shards_count,
                info_length,
            ));
        }
        let position = shards_collection
            .iter()
            .position(|x| x.is_some())
            .ok_or(ConsensusError::InsufficientShardsInDecoder(0, info_length))?;
        let shard_size = shards_collection[position].as_ref().unwrap().len();
        self.reset(info_length, parity_length, shard_size)
            .map_err(|e| ConsensusError::EncoderResetFailed(e.to_string()))?;
        for (i, maybe_shard) in shards_collection.iter().take(info_length).enumerate() {
            if let Some(shard) = maybe_shard {
                self.add_original_shard(i, shard)
                    .map_err(|e| ConsensusError::AddShardFailed(e.to_string()))?;
            }
        }

        for (i, maybe_shard) in shards_collection.iter().enumerate().skip(info_length) {
            if let Some(shard) = maybe_shard {
                self.add_recovery_shard(i - info_length, shard)
                    .map_err(|e| ConsensusError::AddShardFailed(e.to_string()))?;
            }
        }

        let mut data: Vec<Shard> = vec![vec![]; info_length];
        for (i, item) in data.iter_mut().enumerate().take(info_length) {
            if shards_collection[i].is_some() {
                *item = shards_collection[i].clone().unwrap();
            }
        }
        let result = self
            .decode()
            .map_err(|e| ConsensusError::ShardsDecodingFailed(e.to_string()))?;
        let restored: HashMap<_, _> = result.restored_original_iter().collect();
        for el in restored {
            data[el.0] = Shard::from(el.1);
        }
        drop(result);

        Self::reconstruct_transactions(data, info_length)
    }

    fn reconstruct_transactions(
        shards: Vec<Shard>,
        info_length: usize,
    ) -> Result<Vec<Transaction>, ConsensusError> {
        assert_eq!(
            shards.len(),
            info_length,
            "Data length must match info length"
        );
        assert!(info_length > 0, "Info length must be greater than 0");
        let mut reconstructed_data = Vec::new();
        for shard in shards.iter().take(info_length) {
            reconstructed_data.extend(shard);
        }

        // Read the first 4 bytes for `bytes_length` to get the size of the original
        // serialized block
        if reconstructed_data.len() < 4 {
            return Err(ConsensusError::ShardsVecIsTooSmall(
                reconstructed_data.len(),
                4,
            ));
        }

        let bytes_length = u32::from_le_bytes(
            reconstructed_data[0..4]
                .try_into()
                .map_err(|_| ConsensusError::ShardsVecIsTooSmall(reconstructed_data.len(), 4))?,
        ) as usize;

        // Ensure the data length matches the declared length
        if reconstructed_data.len() < 4 + bytes_length {
            return Err(ConsensusError::ShardsVecIsTooSmall(
                reconstructed_data.len(),
                4 + bytes_length,
            ));
        }

        tracing::debug!(
            "Reconstructed data length {}, bytes_length {}",
            reconstructed_data.len(),
            bytes_length
        );

        // Deserialize the rest of the data into `Vec<BaseStatement>`
        let serialized_block = &reconstructed_data[4..4 + bytes_length];
        let reconstructed_statements: Vec<Transaction> =
            bcs::from_bytes(serialized_block).map_err(ConsensusError::DeserializationFailure)?;
        Ok(reconstructed_statements)
    }
}
