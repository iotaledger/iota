// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc};

use reed_solomon_simd::ReedSolomonDecoder;

use crate::{Transaction, block_header::Shard, context::Context, error::ConsensusError};

/// Trait for decoding shard collections using systematic Reed-Solomon decoding
/// and reconstructing the original transactions.
pub trait ShardsDecoder {
    /// Attempts to decode a collection of arbitrary >= info_length shards into
    /// a vector of Transactions.
    fn decode_shards(
        &mut self,
        info_length: usize,
        parity_length: usize,
        shards_collection: Vec<Option<Shard>>,
    ) -> Result<Vec<Transaction>, ConsensusError>;

    /// Reconstructs the original vector of Transactions from a vector of first
    /// info_length shards.
    fn reconstruct_transactions(
        shards: Vec<Shard>,
        info_length: usize,
    ) -> Result<Vec<Transaction>, ConsensusError>
    where
        Self: Sized;
}

impl ShardsDecoder for ReedSolomonDecoder {
    fn decode_shards(
        &mut self,
        info_length: usize,
        parity_length: usize,
        shards_collection: Vec<Option<Shard>>,
    ) -> Result<Vec<Transaction>, ConsensusError> {
        let total_length = info_length + parity_length;

        assert_eq!(
            shards_collection.len(),
            total_length,
            "Shards collection length must match total_length"
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
        reconstruct_transactions_impl(shards, info_length)
    }
}

/// Trivial decoder: assumes no redundancy, just reconstructs transactions
/// directly from the provided shards.
pub(crate) struct TrivialDecoder {}

impl ShardsDecoder for TrivialDecoder {
    fn decode_shards(
        &mut self,
        info_length: usize,
        parity_length: usize,
        shards_collection: Vec<Option<Shard>>,
    ) -> Result<Vec<Transaction>, ConsensusError> {
        let total_length = info_length + parity_length;
        assert_eq!(
            shards_collection.len(),
            total_length,
            "Shards collection length must match total_length"
        );

        let mut data: Vec<Shard> = Vec::with_capacity(info_length);
        for maybe_shard in shards_collection.into_iter().take(info_length) {
            match maybe_shard {
                Some(shard) => data.push(shard),
                None => {
                    return Err(ConsensusError::InsufficientShardsInDecoder(
                        data.len(),
                        info_length,
                    ));
                }
            }
        }

        Self::reconstruct_transactions(data, info_length)
    }

    fn reconstruct_transactions(
        shards: Vec<Shard>,
        info_length: usize,
    ) -> Result<Vec<Transaction>, ConsensusError> {
        reconstruct_transactions_impl(shards, info_length)
    }
}

/// Common logic for reconstructing transactions from data shards.
fn reconstruct_transactions_impl(
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

    let serialized_block = &reconstructed_data[4..4 + bytes_length];
    let reconstructed_transactions: Vec<Transaction> =
        bcs::from_bytes(serialized_block).map_err(ConsensusError::DeserializationFailure)?;
    Ok(reconstructed_transactions)
}

/// Creates a decoder from the context, using ReedSolomonDecoder if redundancy
/// is present, otherwise falling back to TrivialDecoder.
pub(crate) fn create_decoder(context: &Arc<Context>) -> Box<dyn ShardsDecoder + Send + Sync> {
    let info_length = context.committee.info_length();
    let parity_length = context.committee.size() - info_length;

    if info_length > 0 && parity_length > 0 {
        Box::new(
            ReedSolomonDecoder::new(info_length, parity_length, 2)
                .expect("We should expect correct creation of the ReedSolomonDecoder"),
        )
    } else {
        Box::new(TrivialDecoder {})
    }
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        Transaction, block_header::Shard, context::Context, decoder::create_decoder,
        encoder::create_encoder,
    };

    #[tokio::test]
    async fn decode_should_fail_cases() {
        let (context, _) = Context::new_for_test(4); // info=2, parity=2
        let context = Arc::new(context);
        let mut encoder = create_encoder(&context);
        let mut decoder = create_decoder(&context);

        let transactions = Transaction::random_transactions(3, 64);
        let serialized = Transaction::serialize(&transactions).expect("serialization should work");

        let shards = encoder
            .encode_serialized_data(&serialized, context.committee.info_length(), 2)
            .expect("encode should succeed");

        // Case 1: too few shards (< info_length)
        let shards_collection: Vec<Option<Shard>> = vec![Some(shards[0].clone()), None, None, None];
        assert!(
            decoder
                .decode_shards(context.committee.info_length(), 2, shards_collection)
                .is_err()
        );

        // Case 2: corrupted shard length
        let mut shards_collection: Vec<Option<Shard>> =
            shards.clone().into_iter().map(Some).collect();
        shards_collection[1] = Some(vec![1, 2, 3]); // wrong size
        assert!(
            decoder
                .decode_shards(context.committee.info_length(), 2, shards_collection)
                .is_err()
        );

        // Case 3: missing too many shards (drop 3 shards, parity=2)
        let mut shards_collection: Vec<Option<Shard>> = shards.into_iter().map(Some).collect();
        shards_collection[0] = None;
        shards_collection[1] = None;
        shards_collection[2] = None;
        assert!(
            decoder
                .decode_shards(context.committee.info_length(), 2, shards_collection)
                .is_err()
        );
    }
}
