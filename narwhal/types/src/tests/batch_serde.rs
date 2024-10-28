// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde_test::{Token, assert_tokens};

use crate::{
    Batch, BatchV1, MetadataV1, VersionedMetadata, serde::batch_serde::Token::NewtypeVariant,
};
#[test]
fn test_serde_batch() {
    let tx = || vec![1; 5];

    let batch = Batch::V1(BatchV1 {
        transactions: (0..2).map(|_| tx()).collect(),
        versioned_metadata: VersionedMetadata::V1(MetadataV1 {
            created_at: 1666205365890,
            received_at: None,
        }),
    });

    assert_tokens(&batch, &[
        NewtypeVariant {
            name: "Batch",
            variant: "V1",
        },
        Token::Struct {
            name: "BatchV1",
            len: 2,
        },
        Token::Str("transactions"),
        Token::Seq { len: Some(2) },
        Token::Seq { len: Some(5) },
        Token::U8(1),
        Token::U8(1),
        Token::U8(1),
        Token::U8(1),
        Token::U8(1),
        Token::SeqEnd,
        Token::Seq { len: Some(5) },
        Token::U8(1),
        Token::U8(1),
        Token::U8(1),
        Token::U8(1),
        Token::U8(1),
        Token::SeqEnd,
        Token::SeqEnd,
        Token::Str("versioned_metadata"),
        NewtypeVariant {
            name: "VersionedMetadata",
            variant: "V1",
        },
        Token::Struct {
            name: "MetadataV1",
            len: 2,
        },
        Token::Str("created_at"),
        Token::U64(1666205365890),
        Token::Str("received_at"),
        Token::None,
        Token::StructEnd,
        Token::StructEnd,
    ]);
}
