// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::{BaseStatement, Shard};
use reed_solomon_simd::ReedSolomonEncoder;

pub type Encoder = ReedSolomonEncoder;

pub trait ShardEncoder {
    fn encode_shards(
        &mut self,
        data: Vec<Shard>,
        info_length: usize,
        parity_length: usize,
    ) -> Vec<Shard>;

    fn encode_statements(
        &mut self,
        block: Vec<BaseStatement>,
        info_length: usize,
        parity_length: usize,
    ) -> Vec<Shard>;
}

impl ShardEncoder for Encoder {
    fn encode_shards(
        &mut self,
        mut data: Vec<Shard>,
        info_length: usize,
        parity_length: usize,
    ) -> Vec<Shard> {
        let shard_bytes = data[0].len();
        self.reset(info_length, parity_length, shard_bytes)
            .expect("Reset failed");
        for shard in data.clone() {
            self.add_original_shard(shard).expect("Adding shard failed");
        }
        let result = self.encode().expect("Encoding failed");
        let recovery: Vec<Shard> = result.recovery_iter().map(|slice| slice.to_vec()).collect();
        data.extend(recovery);
        data
    }

    fn encode_statements(
        &mut self,
        block: Vec<BaseStatement>,
        info_length: usize,
        parity_length: usize,
    ) -> Vec<Shard> {
        let mut serialized =
            bincode::serialize(&block).expect("Serialization of statements before encoding failed");
        let bytes_length = serialized.len();
        let mut statements_with_len: Vec<u8> = (bytes_length as u32).to_le_bytes().to_vec();
        statements_with_len.append(&mut serialized);
        // increase the length by 4 for u32
        let mut shard_bytes = (bytes_length + 4 + info_length - 1) / info_length;

        // Ensure shard_bytes meets alignment requirements.
        if shard_bytes % 64 != 0 {
            shard_bytes += 64 - shard_bytes % 64;
        }

        let length_with_padding = shard_bytes * info_length;
        statements_with_len.resize(length_with_padding, 0);

        let data: Vec<Shard> = statements_with_len
            .chunks(shard_bytes)
            .map(|chunk| chunk.to_vec())
            .collect();

        self.encode_shards(data, info_length, parity_length)
    }
}
