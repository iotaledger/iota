// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { attachEnvironmentPlugin, getAmplitudeConsentStatus } from '@iota/core';

import { ampli } from './ampli';
import { LogLevel } from '@amplitude/analytics-types';

const IS_ENABLED =
    import.meta.env.VITE_BUILD_ENV === 'production' &&
    import.meta.env.VITE_AMPLITUDE_ENABLED === 'true';

export async function initAmplitude() {
    // Check consent status to determine initial opt-out state
    const consentStatus = getAmplitudeConsentStatus();

    if (ampli.isLoaded || consentStatus === 'declined') {
        return;
    }
    // Delay initialization by 1s to filter out immediate ghost sessions
    setTimeout(async () => {
        // Abort if the user closed the tab or backgrounded the app during the delay
        if (document.visibilityState === 'hidden') {
            return;
        }

        // Load Amplitude normally for valid sessions
        await ampli.load({
            environment: 'iotaexplorer',
            // Flip this if you'd like to test Amplitude locally
            disabled: !IS_ENABLED,
            client: {
                configuration: {
                    optOut: false,
                    autocapture: {
                        attribution: IS_ENABLED,
                        fileDownloads: IS_ENABLED,
                        formInteractions: IS_ENABLED,
                        pageViews: IS_ENABLED,
                        sessions: IS_ENABLED,
                    },
                    // set LogLevel to Debug for more verbose logging during development
                    logLevel: LogLevel.None,
                    flushIntervalMillis: 1000,
                    flushQueueSize: 30,
                },
            },
        }).promise;

        window.addEventListener('pagehide', () => {
            amplitude.setTransport('beacon');
            amplitude.flush();
        });
    }, 1000);

    // Add environment plugin to set prefix dev events
    ampli.client.add(attachEnvironmentPlugin());
}
