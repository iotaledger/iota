// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { getAmplitudeConsentStatus, setNetworkGroup } from '@iota/core';
import { type Network } from '@iota/iota-sdk/client';

import { ampli } from './ampli';
import { LogLevel } from '@amplitude/analytics-types';

const IS_ENABLED = import.meta.env.VITE_BUILD_ENV === 'production';

export async function initAmplitude(networkId: Network | string, url?: string) {
    // Check consent status to determine initial opt-out state
    const consentStatus = getAmplitudeConsentStatus();

    if (ampli.isLoaded || consentStatus === 'declined') {
        return;
    }

    await ampli.load({
        environment: 'iotaexplorer',
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

    setNetworkGroup(ampli.client, networkId as Network, url);

    window.addEventListener('pagehide', () => {
        amplitude.setTransport('beacon');
        amplitude.flush();
    });
}
