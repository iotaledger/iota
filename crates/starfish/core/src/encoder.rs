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
    if !shard_bytes.is_multiple_of(2) {
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
    if info_length == 0 {
        panic!("Info length must be greater than 0");
    }
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

#[cfg(test)]
mod tests {
    use rand::{prelude::SliceRandom, thread_rng};

    use super::*;
    use crate::{Transaction, context::Context, decoder::create_decoder};

    #[tokio::test]
    #[should_panic(expected = "Data length must match info length")]
    async fn encode_should_fail_mismatched_length() {
        let committee_size = 3;
        let (context, _) = Context::new_for_test(committee_size);
        let context = Arc::new(context);
        let mut encoder = create_encoder(&context);

        let transactions = Transaction::random_transactions(2, 16);
        let serialized = Transaction::serialize(&transactions).expect("serialization works");

        // create shards for info_length=2 but then call with mismatched info_length=3
        let data = create_shards_from_serialized_transactions(&serialized, 2);
        let _ = encoder.encode_shards(data, committee_size, 0).unwrap();
    }

    // Test encoding and decoding with trivial encoder/decoder (no redundancy)
    // for various counts and lengths of random transactions
    #[tokio::test]
    async fn trivial_encoding_decoding_random_transactions() {
        // Committee size == info_length → no parity
        let (context, _) = Context::new_for_test(2);
        let context = Arc::new(context);
        let info_length = context.committee.info_length();
        let parity_length = context.committee.parity_length();
        assert_eq!(info_length, 2);
        assert_eq!(parity_length, 0);

        let mut encoder = create_encoder(&context);
        let mut decoder = create_decoder(&context);

        for count in 1..8 {
            for max_len in 0..64 {
                let transactions = Transaction::random_transactions(count, max_len);
                let serialized = Transaction::serialize(&transactions)
                    .expect("We should expect serialization to work");

                let shards = encoder
                    .encode_serialized_data(&serialized, info_length, parity_length)
                    .expect("We should expect that the encoder can succeed");

                let shards_collection: Vec<Option<Shard>> = shards.into_iter().map(Some).collect();

                let decoded = decoder
                    .decode_shards(context.committee.info_length(), 0, shards_collection)
                    .expect("We should expect that the decoder can succeed");

                // Check that decoded transactions match original transactions
                assert_eq!(decoded, transactions);
            }
        }
    }

    // Test encoding and decoding with Reed-Solomon encoder/decoder for various
    // counts and lengths of random transactions, randomly dropping up to
    // parity_length shards to simulate successful decoding using parity shards

    #[tokio::test]
    async fn reed_solomon_encoding_decoding_random_transactions() {
        for committee_size in 4..10 {
            let (context, _) = Context::new_for_test(committee_size);
            let parity_length = context.committee.parity_length();
            let info_length = context.committee.info_length();

            let context = Arc::new(context);
            let mut encoder = create_encoder(&context.clone());
            let mut decoder = create_decoder(&context.clone());

            for count in 1..8 {
                for max_len in 0..32 {
                    let transactions = Transaction::random_transactions(count, max_len);
                    let serialized = Transaction::serialize(&transactions)
                        .expect("We should expect serialization to work");

                    let shards = encoder
                        .encode_serialized_data(&serialized, info_length, parity_length)
                        .expect("We should expect that encode can succeed");

                    // check shard size alignment
                    let bytes_length = serialized.len();
                    let mut expected_shard_bytes = (bytes_length + 4).div_ceil(info_length);
                    if expected_shard_bytes % 2 != 0 {
                        expected_shard_bytes += 1;
                    }

                    for shard in &shards {
                        assert_eq!(
                            shard.len(),
                            expected_shard_bytes,
                            "Shard size should match alignment logic (count={count}, max_len={max_len})"
                        );
                    }

                    // randomly drop up to parity_length shards
                    let mut rng = thread_rng();
                    let mut shards_collection: Vec<Option<Shard>> =
                        shards.into_iter().map(Some).collect();

                    let mut indices: Vec<usize> = (0..shards_collection.len()).collect();
                    indices.shuffle(&mut rng);

                    for &i in indices.iter().take(parity_length) {
                        shards_collection[i] = None;
                    }

                    let decoded = decoder
                        .decode_shards(info_length, parity_length, shards_collection)
                        .expect("We should expect that decode can succeed");

                    assert_eq!(decoded, transactions);
                }
            }
        }
    }
}
