// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel, type UserSession } from '@amplitude/analytics-types';
import { PersistableStorage, consentBufferPlugin } from '@iota/core';

import { ampli } from './ampli';

const IS_PROD_ENV = process.env.NEXT_PUBLIC_BUILD_ENV == 'production';

export const persistableStorage = new PersistableStorage<UserSession>();

export async function initAmplitude() {
    if (ampli.isLoaded) return;

    await ampli.load({
        environment: 'iotawalletdashboard',
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_PROD_ENV,
        client: {
            configuration: {
                optOut: false,
                logLevel: IS_PROD_ENV ? LogLevel.Warn : LogLevel.None,
                // Disable default tracking plugins that auto-send events
                defaultTracking: {
                    pageViews: false,
                    formInteractions: false,
                    fileDownloads: false,
                },
            },
        },
    }).promise;

    // Add consent buffer plugin to Amplitude
    // This plugin queues events in localStorage before user consent
    if (ampli.client) {
        await ampli.client.add(consentBufferPlugin).promise;
    }

    window.addEventListener('pagehide', () => {
        amplitude.setTransport('beacon');
        amplitude.flush();
    });
}
