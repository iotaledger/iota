// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel } from '@amplitude/analytics-types';
import { getCustomNetwork } from '@iota/core';
import { getNetwork, type Network } from '@iota/iota-sdk/client';

import { ampli } from './ampli';
import store from '_src/ui/app/redux/store';
import { AppType } from '_src/ui/app/redux/slices/app/appType';
import Browser from 'webextension-polyfill';

const IS_ENABLED = true;

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
                    pageViews: IS_ENABLED,
                    sessions: IS_ENABLED,
                },
                logLevel: IS_ENABLED ? LogLevel.Warn : LogLevel.Debug,
                // Flush events immediately to prevent data loss when popup closes
                flushIntervalMillis: 1000,
                flushQueueSize: 5,
            },
        },
    });

    setAmplitudeIdentity();

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

type AmplitudeIdentityOptions = {
    network?: Network;
    customRpc?: string | null;
    appType?: AppType;
};

/**
 * Update the user's network group in Amplitude.
 * This allows filtering events by network in Amplitude analytics.
 */
export function setAmplitudeIdentity(options?: AmplitudeIdentityOptions): void {
    if (!ampli.isLoaded) {
        return;
    }
    const {
        network: stateNetwork,
        customRpc: stateCustomRpc,
        appType: stateAppType,
    } = store.getState().app;

    const networkName = getNetworkName(
        options?.network ?? stateNetwork,
        options?.customRpc ?? stateCustomRpc,
    );

    const appType = options?.appType ?? stateAppType;
    const walletAppMode = appType === AppType.Fullscreen ? 'Fullscreen' : 'Pop-up';
    const walletVersion = Browser.runtime.getManifest().version;

    const identifyEvent = new amplitude.Identify();
    identifyEvent.set('network', networkName);
    identifyEvent.set('walletAppMode', walletAppMode);
    identifyEvent.set('walletVersion', walletVersion);

    ampli.client.identify(identifyEvent);
}
