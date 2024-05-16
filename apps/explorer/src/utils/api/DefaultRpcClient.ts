// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { SentryHttpTransport } from '@mysten/core';
import { SuiClient, SuiHTTPTransport, getNetwork, Network, NetworkId, getAllNetworks, SuiClientOptions } from '@mysten/sui.js/client';

export { Network } from '@mysten/sui.js/client';

const supportedNetworks = getAllNetworks();

// The Explorer always show the Custom RPC input so there is no need to confuse it more by having a Custom Network here
delete supportedNetworks[Network.Custom]

export const NetworkConfigs = Object.fromEntries(Object.values(supportedNetworks).map(({ id, rpc }) => {
	return [id, {
		url: rpc
	}]
})) as Record<Network, SuiClientOptions>

const defaultClientMap: Map<NetworkId, SuiClient> = new Map();

// NOTE: This class should not be used directly in React components, prefer to use the useSuiClient() hook instead
export const createSuiClient = (network: NetworkId) => {
	const existingClient = defaultClientMap.get(network);
	if (existingClient) return existingClient;

	const supportedNetwork = getNetwork(network);
	// If network is not supported, we use assume we are using a custom RPC
	const networkUrl = supportedNetwork?.rpc ?? network;

	const client = new SuiClient({
		transport:
			supportedNetwork && network === Network.Mainnet
				? new SentryHttpTransport(networkUrl)
				: new SuiHTTPTransport({ url: networkUrl }),
	});
	defaultClientMap.set(network, client);
	return client;
};
