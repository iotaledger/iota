// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClientGraphQLTransport } from '@iota/graphql-transport';
import { getAllNetworks, getNetwork, IotaClient, type NetworkId } from '@iota/iota-sdk/client';

export const SupportedNetworks = getAllNetworks();

const defaultClientMap: Map<NetworkId, IotaClient> = new Map();

// NOTE: This class should not be used directly in React components, prefer to use the useIotaClient() hook instead
export const createIotaClient = (network: NetworkId): IotaClient => {
    const existingClient = defaultClientMap.get(network);
    if (existingClient) return existingClient;

    const supportedNetwork = getNetwork(network);
    // If network is not supported, we use assume we are using a custom RPC
    const networkGraphqlUrl = supportedNetwork?.graphql ?? network;
    const networkJsonRpcUrl = supportedNetwork?.url;

    const client = new IotaClient({
        transport: new IotaClientGraphQLTransport({
            url: networkGraphqlUrl,
            fallbackTransportUrl: networkJsonRpcUrl,
            unsupportedMethods: [
                'multiGetObjects',
                'getReferenceGasPrice',
                'getOwnedObjects',
                'executeTransactionBlock',
            ],
        }),
    });
    defaultClientMap.set(network, client);
    return client;
};
