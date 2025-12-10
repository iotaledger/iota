// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel, TransportType, type UserSession } from '@amplitude/analytics-types';
import { getCustomNetwork, PersistableStorage } from '@iota/core';

import { ampli } from './ampli';
import { getNetwork, type Network } from '@iota/iota-sdk/client';

const IS_PROD_ENV = import.meta.env.VITE_BUILD_ENV === 'production';

export const persistableStorage = new PersistableStorage<UserSession>();

const ApiKey = {
    production: '896b9073219c06800d9bf0aecf1b6f80',
    development: '253fa1582d8ed913d8c5957f601df3fe',
};

export async function initAmplitude() {
    ampli.load({
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_PROD_ENV,
        client: {
            apiKey: IS_PROD_ENV ? ApiKey.production : ApiKey.development,
            configuration: {
                cookieStorage: persistableStorage,
                logLevel: IS_PROD_ENV ? LogLevel.Warn : LogLevel.Debug,
            },
        },
    });

    window.addEventListener('pagehide', () => {
        amplitude.setTransport(TransportType.SendBeacon);
        amplitude.flush();
    });
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
export function setNetworkGroup(network: Network, customRpc?: string | null): void {
    if (!ampli.isLoaded) {
        return;
    }
    const networkName = getNetworkName(network, customRpc);
    ampli.client.setGroup('network', networkName);
}
