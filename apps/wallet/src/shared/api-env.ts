// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { Network, type NetworkConfiguration } from '@mysten/sui.js/client';

export type NetworkEnvType =
	| { network: Exclude<Network, Network.Custom>; customRpcUrl: null }
	| { network: Network.Custom; customRpcUrl: string };

export function getCustomNetwork(rpc: string = ''): NetworkConfiguration {
	return {
		name: 'Custom Network',
		id: Network.Custom,
		url: rpc,
		chain: 'sui:unknown',
		explorer: 'custom explorer',
	};
}
