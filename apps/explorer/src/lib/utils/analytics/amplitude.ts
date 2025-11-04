// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { consentBufferPlugin } from '@iota/core';

import { ampli } from './ampli';
import { LogLevel } from '@amplitude/analytics-types';

const IS_PROD_ENV = import.meta.env.VITE_BUILD_ENV === 'production';

export async function initAmplitude() {
    await ampli.load({
        environment: 'iotaexplorer',
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_PROD_ENV,
        client: {
            configuration: {
                optOut: false,
                autocapture: true,
                logLevel: IS_PROD_ENV ? LogLevel.Warn : LogLevel.None,
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
