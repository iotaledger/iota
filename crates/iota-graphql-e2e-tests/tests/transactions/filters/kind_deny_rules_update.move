// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// The kind filters compare with the kind numbers the indexer stores: a
// deny-rules update is a system transaction and not a claim.

//# init --simulator

//# advance-epoch --create-deny-rules-object

//# update-deny-rules --added-addresses 0xBB

//# create-checkpoint

//# run-graphql
{
  transactionBlocks(first: 50 filter: {kind: SYSTEM_TX}) {
    nodes {
      kind {
        __typename
      }
    }
  }
}

//# run-graphql
{
  transactionBlocks(first: 50 filter: {kind: CLAIM_ACCOUNT}) {
    nodes {
      digest
      kind {
        __typename
      }
    }
  }
}
