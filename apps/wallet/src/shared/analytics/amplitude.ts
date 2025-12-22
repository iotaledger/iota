// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel } from '@amplitude/analytics-types';

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
