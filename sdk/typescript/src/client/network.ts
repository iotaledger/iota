// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function getFullnodeUrl(network: 'mainnet' | 'testnet' | 'devnet' | 'localnet') {
    switch (network) {
        case 'mainnet':
            return 'https://fullnode.mainnet.iota.io:443';
        case 'testnet':
            return 'https://fullnode.testnet.iota.io:443';
        case 'devnet':
            return 'https://fullnode.devnet.iota.io:443';
        case 'localnet':
            return 'http://127.0.0.1:9000';
        default:
            throw new Error(`Unknown network: ${network}`);
    }
}
