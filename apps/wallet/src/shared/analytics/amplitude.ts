// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel } from '@amplitude/analytics-types';
import { getCustomNetwork } from '@iota/core';
import { getNetwork, type Network } from '@iota/iota-sdk/client';

import { ampli } from './ampli';

const IS_ENABLED = process.env.BUILD_ENV === 'production';

export async function initAmplitude() {
    ampli.load({
        environment: 'iotawallet',
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_ENABLED,
        client: {
            configuration: {
                optOut: false,
                // Explicitly use cookie storage to persist data across popup sessions
                identityStorage: 'cookie',
                autocapture: {
                    attribution: IS_ENABLED,
                    fileDownloads: IS_ENABLED,
                    formInteractions: IS_ENABLED,
                    pageViews: IS_ENABLED,
                    sessions: IS_ENABLED,
                },
                // set LogLevel to Debug for more verbose logging during development
                logLevel: LogLevel.None,
                // Flush events immediately to prevent data loss when popup closes
                flushIntervalMillis: 1000,
                flushQueueSize: 5,
            },
        },
    });

    // Flush events when popup is about to close
    window.addEventListener('pagehide', () => {
        amplitude.setTransport('beacon');
        amplitude.flush();
    });

    // Additional flush on visibility change (when popup loses focus)
    document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'hidden') {
            amplitude.setTransport('beacon');
            amplitude.flush();
        }
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
export function setNetworkGroup(network: Network, customRpc?: string | null): void {
    if (!ampli.isLoaded) {
        return;
    }
    const networkName = getNetworkName(network, customRpc);
    ampli.client.setGroup('network', networkName);
}
