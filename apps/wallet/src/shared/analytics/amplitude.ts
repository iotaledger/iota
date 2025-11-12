// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel, type UserSession } from '@amplitude/analytics-types';
import { PersistableStorage, getCustomNetwork } from '@iota/core';
import { getNetwork, type Network } from '@iota/iota-sdk/client';

import { ampli } from './ampli';

const IS_PROD_ENV = process.env.BUILD_ENV === 'production';

export const persistableStorage = new PersistableStorage<UserSession>();

export async function initAmplitude() {
    await ampli.load({
        environment: 'iotawallet',
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_PROD_ENV,
        client: {
            configuration: {
                // TODO add consentBufferPlugin in the next iteration
                logLevel: IS_PROD_ENV ? LogLevel.Warn : LogLevel.Debug,
            },
        },
    }).promise;

    window.addEventListener('pagehide', () => {
        amplitude.setTransport('beacon');
        amplitude.flush();
    });
}

export function getUrlWithDeviceId(url: URL) {
    const amplitudeDeviceId = ampli.client.getDeviceId();
    if (amplitudeDeviceId) {
        url.searchParams.append('deviceId', amplitudeDeviceId);
    }
    return url;
}

/**
 * Get the network name for analytics tracking.
 * Returns the network name (e.g., "mainnet", "testnet", "devnet", "custom").
 */
export function getNetworkName(network: Network, customRpc?: string | null): string {
    if (customRpc) {
        return getCustomNetwork(customRpc).name || 'custom';
    }
    return getNetwork(network)?.name || 'unknown';
}

/**
 * Update the user's network group in Amplitude.
 * This allows filtering events by network in Amplitude analytics.
 */
export function setNetworkGroup(activeNetwork: Network, customRpc?: string | null): void {
    const networkName = getNetworkName(activeNetwork, customRpc);
    ampli.client.setGroup('activeNetwork', networkName);
}
