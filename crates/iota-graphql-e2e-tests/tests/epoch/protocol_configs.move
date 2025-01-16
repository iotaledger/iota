// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --protocol-version 3 --simulator --accounts C

//# create-checkpoint

//# run-graphql
{
    protocolConfig {
        protocolVersion
        config(key: "max_move_identifier_len") {
            value
        }
        featureFlag(key: "bridge") {
            value
        }
    }
}

//# run-graphql
{
    protocolConfig(protocolVersion: 1) {
        protocolVersion
        config(key: "max_move_identifier_len") {
            value
        }
        featureFlag(key: "bridge") {
            value
        }
    }
}
