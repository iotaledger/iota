// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SentryHttpTransport } from '@iota/core';
import {
    IotaClient,
    IotaHTTPTransport,
    getNetwork,
    Network,
    type NetworkId,
    getAllNetworks,
} from '@iota/iota-sdk/client';

export const SupportedNetworks = getAllNetworks();
// The Explorer always shows the Custom RPC input so there is no need to confuse it more by having a Custom Network here
delete SupportedNetworks[Network.Custom];

const defaultClientMap: Map<NetworkId, IotaClient> = new Map();

// NOTE: This class should not be used directly in React components, prefer to use the useIotaClient() hook instead
export const createIotaClient = (network: NetworkId): IotaClient => {
    const existingClient = defaultClientMap.get(network);
    if (existingClient) return existingClient;

    const supportedNetwork = getNetwork(network);
    // If network is not supported, we use assume we are using a custom RPC
    const networkUrl = supportedNetwork?.url ?? network;

    const client = new IotaClient({
        transport:
            supportedNetwork && network === Network.Mainnet // Sentry dev hint: change this to eg [Network.Localnet]
                ? new SentryHttpTransport(networkUrl)
                : new IotaHTTPTransport({ url: networkUrl }),
    });
    defaultClientMap.set(network, client);
    return client;
};
