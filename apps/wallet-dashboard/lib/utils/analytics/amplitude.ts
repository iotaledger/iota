// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel } from '@amplitude/analytics-types';
import { Network } from '@iota/iota-sdk/client';
import { setNetworkGroup, getAmplitudeConsentStatus } from '@iota/core';

import { ampli } from './ampli';

const IS_ENABLED = true;

export async function initAmplitude(network: Network, url?: string) {
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
                    pageViews: IS_ENABLED,
                    sessions: IS_ENABLED,
                },
                logLevel: IS_ENABLED ? LogLevel.Warn : LogLevel.None,
            },
        },
    }).promise;

    setNetworkGroup(ampli.client, network, url);

    window.addEventListener('pagehide', () => {
        amplitude.setTransport('beacon');
        amplitude.flush();
    });
}
