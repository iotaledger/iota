// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel, TransportType, type UserSession } from '@amplitude/analytics-types';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import { PersistableStorage } from '@iota/core';

import { ampli } from './ampli';

// const IS_PROD_ENV = process.env.NEXT_PUBLIC_BUILD_ENV == 'production';
const IS_PROD_ENV = true;

export const persistableStorage = new PersistableStorage<UserSession>();

export async function initAmplitude() {
    await ampli.load({
        environment: 'iotawalletdashboard',
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_PROD_ENV,
        client: {
            configuration: {
                cookieStorage: persistableStorage,
                logLevel: IS_PROD_ENV ? LogLevel.Warn : amplitude.Types.LogLevel.Debug,
            },
        },
    });

    window.addEventListener('pagehide', () => {
        amplitude.setTransport(TransportType.SendBeacon);
        amplitude.flush();
    });
}

/**
 * Get the network identifier for analytics tracking.
 * - For custom RPC: returns the URL
 * - For known networks: returns the network name (e.g., "mainnet", "testnet", "devnet")
 */
export function getNetworkName(network: Network, customRpc?: string | null): string {
    if (customRpc) {
        return customRpc;
    }
    return getNetwork(network)?.name || 'unknown';
}

/**
 * Parse a network identifier into Network enum and optional custom RPC URL.
 * The explorer stores network as either a Network enum value OR a custom RPC URL string.
 */
export function parseNetworkIdentifier(networkId: string): {
    network: Network;
    customRpc: string | null;
} {
    const isCustomRpc =
        networkId && !(Object.values(Network) as string[]).includes(networkId.toUpperCase());
    return {
        network: isCustomRpc ? Network.Custom : (networkId as Network),
        customRpc: isCustomRpc ? networkId : null,
    };
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
