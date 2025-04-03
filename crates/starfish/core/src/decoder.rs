// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use reed_solomon_simd::ReedSolomonDecoder;
use starfish_config::Committee;

use crate::{
    BlockAPI, Transaction,
    block::{BlockBody, BlockV2, Shard, TransactionsCommitment},
    encoder::{Encoder, ShardEncoder},
};

pub type Decoder = ReedSolomonDecoder;

pub trait CachedStatementBlockDecoder {
    #[allow(dead_code)]
    fn decode_shards(
        &mut self,
        committee: &Committee,
        encoder: &mut Encoder,
        block: BlockV2,
        shards_collection: Vec<Option<Shard>>,
    ) -> Option<BlockV2>;
    fn reconstruct_transactions(shards: Vec<Shard>, info_length: usize) -> Vec<Transaction>;
}

impl CachedStatementBlockDecoder for Decoder {
    fn decode_shards(
        &mut self,
        committee: &Committee,
        encoder: &mut Encoder,
        mut block: BlockV2,
        shards_collection: Vec<Option<Shard>>,
    ) -> Option<BlockV2> {
        let info_length = committee.info_length();
        let total_length = committee.size();
        let parity_length = total_length - info_length;
        let position = shards_collection
            .iter()
            .position(|x| x.is_some())
            .expect("Expect a shards_collection to contain at least info_length shards");
        let shard_size = shards_collection[position].as_ref().unwrap().len();
        self.reset(info_length, parity_length, shard_size)
            .expect("decoder reset failed");
        for i in 0..info_length {
            if shards_collection[i].is_some() {
                self.add_original_shard(i, shards_collection[i].as_ref().unwrap())
                    .expect("adding shard failed")
            }
        }
        for i in info_length..total_length {
            if shards_collection[i].is_some() {
                self.add_recovery_shard(i - info_length, shards_collection[i].as_ref().unwrap())
                    .expect("adding shard failed")
            }
        }

        let mut data: Vec<Shard> = vec![vec![]; info_length];
        for (i, item) in data.iter_mut().enumerate().take(info_length) {
            if shards_collection[i].is_some() {
                *item = shards_collection[i].clone().unwrap();
            }
        }
        let result = self.decode().expect("Decoding should be correct");
        let restored: HashMap<_, _> = result.restored_original_iter().collect();
        for el in restored {
            data[el.0] = Shard::from(el.1);
        }
        drop(result);

        let recovered_statements = encoder.encode_shards(data, info_length, parity_length);
        if TransactionsCommitment::check_correctness_merkle_root(
            &recovered_statements,
            *block.transactions_commitment(),
        ) {
            let transactions = Self::reconstruct_transactions(recovered_statements, info_length);
            block.body = BlockBody::new_transactions(transactions);
            return Some(block);
        }
        None
    }

    fn reconstruct_transactions(shards: Vec<Shard>, info_length: usize) -> Vec<Transaction> {
        let mut reconstructed_data = Vec::new();
        for i in 0..info_length {
            reconstructed_data.extend(shards[i].clone());
        }

        // Read the first 4 bytes for `bytes_length` to get the size of the original
        // serialized block
        if reconstructed_data.len() < 4 {
            panic!("Reconstructed data is too short to contain a valid length");
        }

        let bytes_length = u32::from_le_bytes(
            reconstructed_data[0..4]
                .try_into()
                .expect("Failed to read bytes_length"),
        ) as usize;

        // Ensure the data length matches the declared length
        if reconstructed_data.len() < 4 + bytes_length {
            panic!(
                "Reconstructed data length does not match the declared bytes_length; {} {}",
                reconstructed_data.len(),
                bytes_length
            );
        } else {
            tracing::debug!(
                "Reconstructed data length {}, bytes_length {}",
                reconstructed_data.len(),
                bytes_length
            );
        }

        // Deserialize the rest of the data into `Vec<BaseStatement>`
        let serialized_block = &reconstructed_data[4..4 + bytes_length];
        let reconstructed_statements: Vec<Transaction> = bincode::deserialize(serialized_block)
            .expect("Deserialization of reconstructed data failed");
        reconstructed_statements
    }
}
