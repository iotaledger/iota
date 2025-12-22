// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork, Network } from '@iota/iota-sdk/client';
import * as amplitude from '@amplitude/analytics-browser';
import { getCustomNetwork } from './api-env';

export type BrowserClient = amplitude.Types.BrowserClient;

const GROUP_KEY = 'network';

/**
 * Update the user's network group in Amplitude.
 * This allows filtering events by network in Amplitude analytics.
 */
export function setNetworkGroup(
    amplitudeClient: BrowserClient,
    networkId: Network,
    url?: string,
): void {
    if (!amplitudeClient) {
        console.warn('Amplitude client is not initialized. Cannot set network group.');
        return;
    }

    const knownNetworkName = getNetwork(networkId)?.name || 'not-defined';
    const customRpcName = getCustomNetwork(url).name;

    const networkName = networkId === Network.Custom && url ? customRpcName : knownNetworkName;

    amplitudeClient?.setGroup(GROUP_KEY, networkName);
}
