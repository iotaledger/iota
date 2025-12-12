// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    getDefaultNetwork,
    getNetwork,
    Network,
    type NetworkConfiguration,
} from '@iota/iota-sdk/client';

export type NetworkEnvType =
    | { network: Exclude<Network, Network.Custom>; customRpcUrl: null }
    | { network: Network.Custom; customRpcUrl: string };

export function getCustomNetwork(rpc: string = ''): NetworkConfiguration {
    return {
        name: 'Custom RPC',
        id: Network.Custom,
        url: rpc,
        chain: 'iota:unknown',
        explorer: getNetwork(getDefaultNetwork()).explorer,
    };
}

/**
 * Returns the network name (e.g., "mainnet", "testnet", "devnet", "custom", "unknown").
 */
export function getNetworkName(network: Network, customRpc?: string | null): string {
    if (customRpc) {
        return getCustomNetwork(customRpc).name;
    }
    return getNetwork(network)?.name || 'unknown';
}
