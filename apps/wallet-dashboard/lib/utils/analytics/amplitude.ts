// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LogLevel } from '@amplitude/analytics-types';
import { attachEnvironmentPlugin, getAmplitudeConsentStatus } from '@iota/core';

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

    window.addEventListener('pagehide', () => {
        ampli.client.setTransport('beacon');
        ampli.flush();
    });

    // Add environment plugin to set prefix dev events
    ampli.client.add(attachEnvironmentPlugin(IS_DEV));
}
