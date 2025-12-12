// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Network } from '@iota/iota-sdk/client';
import * as amplitude from '@amplitude/analytics-browser';
import { getNetworkName } from './api-env';

export type BrowserClient = amplitude.Types.BrowserClient;
/**
 * Update the user's network group in Amplitude.
 * This allows filtering events by network in Amplitude analytics.
 */
export function setNetworkGroup(
    amplitudeClient: BrowserClient,
    network: Network,
    customRpc?: string | null,
    groupKey: string = 'network',
): void {
    if (!amplitudeClient) {
        console.warn('Amplitude client is not initialized. Cannot set network group.');
        return;
    }
    const networkName = getNetworkName(network, customRpc);
    amplitudeClient?.setGroup(groupKey, networkName);
}
