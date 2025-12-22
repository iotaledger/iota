// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { getAmplitudeConsentStatus, getCustomNetwork } from '@iota/core';

import { ampli } from './ampli';
import { LogLevel } from '@amplitude/analytics-types';
import { getNetwork, type Network } from '@iota/iota-sdk/client';

const IS_ENABLED = import.meta.env.VITE_BUILD_ENV === 'production';

export async function initAmplitude() {
    // Check consent status to determine initial opt-out state
    const consentStatus = getAmplitudeConsentStatus();

    if (ampli.isLoaded || consentStatus === 'declined') {
        return;
    }

    await ampli.load({
        environment: 'iotaexplorer',
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_ENABLED,
        client: {
            configuration: {
                optOut: false,
                autocapture: {
                    pageViews: IS_ENABLED,
                    sessions: IS_ENABLED,
                },
                logLevel: IS_ENABLED ? LogLevel.Warn : LogLevel.None,
            },
        },
    }).promise;

    window.addEventListener('pagehide', () => {
        amplitude.setTransport('beacon');
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
