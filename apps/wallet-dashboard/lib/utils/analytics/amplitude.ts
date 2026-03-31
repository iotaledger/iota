// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LogLevel } from '@amplitude/analytics-types';
import {
    attachEnvironmentPlugin,
    dialogContextPlugin,
    getAmplitudeConsentStatus,
} from '@iota/core';
import { Identify } from '@amplitude/analytics-browser';

import { ampli } from './ampli';

const IS_ENABLED =
    process.env.NEXT_PUBLIC_BUILD_ENV === 'production' &&
    process.env.NEXT_PUBLIC_AMPLITUDE_ENABLED === 'true';

const IS_DEV = process.env.NEXT_PUBLIC_BUILD_ENV !== 'production';

export async function initAmplitude() {
    // Check consent status to determine initial opt-out state
    const consentStatus = getAmplitudeConsentStatus();

    if (ampli.isLoaded || consentStatus === 'declined') {
        return;
    }

    await ampli.load({
        environment: 'iotawalletdashboard',
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_ENABLED,
        client: {
            configuration: {
                optOut: false,
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
            },
        },
    }).promise;

    // Add dialog context plugin to enrich events with dialog information
    if (IS_ENABLED) {
        ampli.client.add(dialogContextPlugin(ampli.client));
    }

    window.addEventListener('pagehide', () => {
        ampli.client.setTransport('beacon');
        ampli.flush();
    });

    // Add environment plugin to set prefix dev events
    ampli.client.add(attachEnvironmentPlugin(IS_DEV));
}

type AmplitudeIdentityOptions = {
    network: string;
};

/**
 * Set Amplitude user properties with the current app context.
 * Currently updates the user property: network.
 * This allows filtering and segmenting analytics events by this dimension.
 */
export function setAmplitudeIdentity(options: AmplitudeIdentityOptions): void {
    const consentStatus = getAmplitudeConsentStatus();
    if (!ampli.isLoaded || !IS_ENABLED || consentStatus === 'declined') {
        return;
    }

    const identifyEvent = new Identify();
    identifyEvent.set('network', options.network);

    ampli.client.identify(identifyEvent);
}
