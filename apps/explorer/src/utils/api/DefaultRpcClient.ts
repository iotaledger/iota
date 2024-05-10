// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { SentryHttpTransport } from '@mysten/core';
import { SuiClient, SuiHTTPTransport, getFullnodeUrl } from '@mysten/sui.js/client';
import { ALPHANET_URL } from '~/lib/constants';

export enum Network {
	Local = 'local',
	Devnet = 'devnet',
	Testnet = 'testnet',
	Mainnet = 'mainnet',
	Alphanet = 'alphanet',
}

export const NetworkConfigs: Record<Network, { url: string }> = {
	[Network.Local]: { url: getFullnodeUrl('localnet') },
	[Network.Devnet]: { url: '' },
	[Network.Testnet]: { url: '' },
	[Network.Mainnet]: { url: '' },
	[Network.Alphanet]: { url: ALPHANET_URL },
};

const defaultClientMap: Map<Network | string, SuiClient> = new Map();

// NOTE: This class should not be used directly in React components, prefer to use the useSuiClient() hook instead
export const createSuiClient = (network: Network | string) => {
	const existingClient = defaultClientMap.get(network);
	if (existingClient) return existingClient;

	const networkUrl = network in NetworkConfigs ? NetworkConfigs[network as Network].url : network;

	const client = new SuiClient({
		transport:
			network in Network && network === Network.Mainnet
				? new SentryHttpTransport(networkUrl)
				: new SuiHTTPTransport({ url: networkUrl }),
	});
	defaultClientMap.set(network, client);
	return client;
};
