// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --protocol-version 4 --simulator

//# create-checkpoint

//# run-graphql
# Make sure the randomness state was created on the epoch boundary
{
    object(address: "0x8") {
        address
        version
        asMoveObject {
            contents {
                type { repr }
                json
            }
        }
    }

    transactionBlocks(last: 1) {
        nodes {
            kind {
                __typename
                ... on EndOfEpochTransaction {
                    transactions {
                        edges {
                            cursor
                            node { __typename }
                        }
                    }
                }
            }
        }
    }
}


//# set-random-state --randomness-round 0 --random-bytes SGVsbG8gU3Vp --randomness-initial-version 2
# Set the contents of the randomness object

//# create-checkpoint

//# run-graphql
{
    transactionBlocks(last: 1) {
        nodes {
            kind {
                __typename
                ... on RandomnessStateUpdateTransaction {
                    epoch { epochId }
                    randomnessRound
                    randomBytes
                    randomnessObjInitialSharedVersion
                }
            }
        }
    }
}
