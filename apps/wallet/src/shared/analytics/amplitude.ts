// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LogLevel } from '@amplitude/analytics-types';
import { attachEnvironmentPlugin, dialogContextPlugin, getCustomNetwork } from '@iota/core';
import { getNetwork, type Network } from '@iota/iota-sdk/client';
import { ampli } from './ampli';
import { type ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';
import Browser from 'webextension-polyfill';
import { Identify } from '@amplitude/analytics-browser';

const IS_ENABLED = process.env.BUILD_ENV === 'production';

const IS_DEV = process.env.BUILD_ENV !== 'production';

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
                    attribution: false,
                    fileDownloads: false,
                    formInteractions: false,
                    pageViews: IS_ENABLED,
                    sessions: IS_ENABLED,
                    elementInteractions: false,
                    frustrationInteractions: false,
                    networkTracking: false,
                    webVitals: false,
                    pageUrlEnrichment: IS_ENABLED,
                },
                // set LogLevel to Debug for more verbose logging during development
                logLevel: LogLevel.None,
                // Flush events immediately to prevent data loss when popup closes
                flushIntervalMillis: 1000,
                flushQueueSize: 5,
            },
        },
    });

    // Add dialog context plugin to enrich events with dialog information
    if (IS_ENABLED) {
        ampli.client.add(dialogContextPlugin(ampli.client));
    }

    // Flush events when popup is about to close
    window.addEventListener('pagehide', () => {
        ampli.client.setTransport('beacon');
        ampli.flush();
    });

    // Additional flush on visibility change (when popup loses focus)
    document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'hidden') {
            ampli.client.setTransport('beacon');
            ampli.flush();
        }
    });

    // Add environment plugin to set prefix dev events
    ampli.client.add(attachEnvironmentPlugin(IS_DEV));
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

type AmplitudeIdentityOptions = {
    network: Network;
    customRpc: string | null;
    extensionViewType: ExtensionViewType;
};

/**
 * Set the Amplitude user identity with current app context.
 * Updates user properties: network, walletAppMode, and walletVersion.
 * This allows filtering and segmenting analytics events by these dimensions.
 */
export function setAmplitudeIdentity(options: AmplitudeIdentityOptions): void {
    if (!ampli.isLoaded) {
        return;
    }

    const identifyEvent = new Identify();
    identifyEvent.set('network', getNetworkName(options.network, options.customRpc));
    identifyEvent.set('walletAppMode', options.extensionViewType);
    identifyEvent.set('walletVersion', Browser.runtime.getManifest().version);

    ampli.client.identify(identifyEvent);
}
