// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel } from '@amplitude/analytics-types';

import { ampli } from './ampli';

const IS_PROD_ENV = process.env.NEXT_PUBLIC_BUILD_ENV == 'production';

export async function initAmplitude() {
    await ampli.load({
        environment: 'iotawalletdashboard',
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_PROD_ENV,
        client: {
            configuration: {
                optOut: false,
                autocapture: {
                    pageViews: IS_PROD_ENV,
                    sessions: IS_PROD_ENV,
                },
                logLevel: IS_PROD_ENV ? LogLevel.Warn : LogLevel.None,
            },
        },
    }).promise;

    window.addEventListener('pagehide', () => {
        amplitude.setTransport('beacon');
        amplitude.flush();
    });
}
