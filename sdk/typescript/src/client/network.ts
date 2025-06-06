// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export enum Network {
    Mainnet = 'mainnet',
    Devnet = 'devnet',
    Testnet = 'testnet',
    Localnet = 'localnet',
    Custom = 'custom',
}

// We also accept `string` in case we want to use a network not supported by the SDK
export type NetworkId = Network | string;

export type ChainType = `${string}:${string}`;

export interface NetworkConfiguration<Metadata = undefined> {
    id: Network;
    name: string;
    url: string;
    explorer: string;
    chain: ChainType;
    faucet?: string;
    kiosk?: KioskConfiguration;
    graphql?: string;
    metadata?: Metadata;
}

export interface KioskConfiguration {
    royaltyRulePackageId: string;
    kioskLockRulePackageId: string;
    floorPriceRulePackageId: string;
    personalKioskRulePackageId: string;
}

type NetworksConfiguration = Record<NetworkId, NetworkConfiguration>;

export function getAllNetworks(): NetworksConfiguration {
    const networksStringified = process.env.IOTA_NETWORKS;

    if (!networksStringified) {
        throw new Error('"IOTA_NETWORKS" env var is not set.');
    }

    let networks;

    try {
        networks = JSON.parse(networksStringified);
    } catch {
        throw new Error('Failed to parse env var "IOTA_NETWORKS".');
    }

    return networks;
}

export function getNetwork<T>(network: NetworkId): NetworkConfiguration<T> {
    const networks = getAllNetworks();

    const requestedNetwork = (networks[network] ?? network) as NetworkConfiguration<T>;

    return requestedNetwork;
}

export function getDefaultNetwork(): Network {
    return (process.env.DEFAULT_NETWORK as Network) || Network.Mainnet;
}

export function getFullnodeUrl(network: NetworkId): string {
    return getNetwork(network).url;
}

export function getGraphQLUrl(network: NetworkId): string | undefined {
    return getNetwork(network).graphql;
}
