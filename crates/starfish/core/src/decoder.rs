// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use reed_solomon_simd::ReedSolomonDecoder;
use starfish_config::Committee;

use crate::{
    BlockAPI, Transaction,
    block::{BlockBody, BlockV2, Shard, TransactionsCommitment},
    encoder::{ReedSolomonEncoder, ShardEncoder},
};
use crate::error::ConsensusError;

/// Trait for decoding shard collections using systematic Reed-Solomon decoding
/// and reconstructing the original transactions.
pub trait ShardsDecoder {
    /// Attempts to decode a shard collection into a valid `BlockBody`.
    #[expect(dead_code)]
    fn decode_shards(
        &mut self,
        committee: &Committee,
        encoder: &mut ReedSolomonEncoder,
        shards_collection: Vec<Option<Shard>>,
    ) -> Result<BlockBody, ConsensusError>;

    /// Reconstructs the original list of `Transaction` objects from a list of shards.
    fn reconstruct_transactions(shards: Vec<Shard>, info_length: usize) -> Result<Vec<Transaction>, ConsensusError>;
}


impl ShardsDecoder for ReedSolomonDecoder {
    fn decode_shards(
        &mut self,
        committee: &Committee,
        encoder: &mut ReedSolomonEncoder,
        shards_collection: Vec<Option<Shard>>,
    ) -> Result<BlockBody, ConsensusError> {
        let info_length = committee.info_length();
        let total_length = committee.size();
        let parity_length = total_length - info_length;
        let shards_count = shards_collection.iter().filter(|x| x.is_some()).count();
        if shards_count < info_length {
            return Err(ConsensusError::InsufficientShardsInDecoder(shards_count, info_length));
        }
        let position = shards_collection
            .iter()
            .position(|x| x.is_some())
            .ok_or_else(|| ConsensusError::InsufficientShardsInDecoder(0, info_length))?;
        let shard_size = shards_collection[position].as_ref().unwrap().len();
        self.reset(info_length, parity_length, shard_size)
            .map_err(|e| ConsensusError::EncoderResetFailed(e.to_string()))?;
        for i in 0..info_length {
            if shards_collection[i].is_some() {
                self.add_original_shard(i, shards_collection[i].as_ref().unwrap())
                    .map_err(|e| ConsensusError::AddShardFailed(e.to_string()))?;
            }
        }
        for i in info_length..total_length {
            if shards_collection[i].is_some() {
                self.add_recovery_shard(i - info_length, shards_collection[i].as_ref().unwrap())
                    .map_err(|e| ConsensusError::AddShardFailed(e.to_string()))?;
            }
        }

        let mut data: Vec<Shard> = vec![vec![]; info_length];
        for (i, item) in data.iter_mut().enumerate().take(info_length) {
            if shards_collection[i].is_some() {
                *item = shards_collection[i].clone().unwrap();
            }
        }
        let result = self.decode().map_err(|e| ConsensusError::ShardsDecodingFailed(e.to_string()))?;
        let restored: HashMap<_, _> = result.restored_original_iter().collect();
        for el in restored {
            data[el.0] = Shard::from(el.1);
        }
        drop(result);
        // decoder restores only the first info_length shards, we need to encode again to get all shards
        let recovered_transactions = encoder.encode_shards(data, info_length, parity_length)?;

            let transactions =
                Self::reconstruct_transactions(recovered_transactions, info_length)?;
            Ok(BlockBody::new_transactions(transactions))
    }

    fn reconstruct_transactions(shards: Vec<Shard>, info_length: usize) -> Result<Vec<Transaction>, ConsensusError> {
        let mut reconstructed_data = Vec::new();
        for i in 0..info_length {
            reconstructed_data.extend(shards[i].clone());
        }

        // Read the first 4 bytes for `bytes_length` to get the size of the original
        // serialized block
        if reconstructed_data.len() < 4 {
            return Err(ConsensusError::ShardsVecIsTooSmall(reconstructed_data.len(), 4));
        }

        let bytes_length = u32::from_le_bytes(
            reconstructed_data[0..4]
                .try_into()
                .map_err(|_| ConsensusError::ShardsVecIsTooSmall(reconstructed_data.len(), 4))?
        ) as usize;

        // Ensure the data length matches the declared length
        if reconstructed_data.len() < 4 + bytes_length {
            return Err(ConsensusError::ShardsVecIsTooSmall(reconstructed_data.len(), 4 + bytes_length));
        }


            tracing::debug!(
                "Reconstructed data length {}, bytes_length {}",
                reconstructed_data.len(),
                bytes_length
            );


        // Deserialize the rest of the data into `Vec<BaseStatement>`
        let serialized_block = &reconstructed_data[4..4 + bytes_length];
        let reconstructed_statements: Vec<Transaction> = bcs::from_bytes(serialized_block)
            .map_err(ConsensusError::DeserializationFailure)?;
        Ok(reconstructed_statements)
    }
}
