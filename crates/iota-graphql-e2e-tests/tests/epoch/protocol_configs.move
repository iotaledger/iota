// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --protocol-version 1 --simulator --accounts C

//# create-checkpoint

//# run-graphql
{
    protocolConfig {
        protocolVersion
        config(key: "max_move_identifier_len") {
            value
        }
        featureFlag(key: "enable_coin_deny_list") {
            value
        }
    }
}

//# run-graphql
{
    protocolConfig(protocolVersion: 8) {
        protocolVersion
        config(key: "max_move_identifier_len") {
            value
        }
        featureFlag(key: "enable_coin_deny_list") {
            value
        }
    }
}
