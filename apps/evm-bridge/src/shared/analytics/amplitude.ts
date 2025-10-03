// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel, TransportType, type UserSession } from '@amplitude/analytics-types';
import { PersistableStorage } from '@iota/core';

import { ampli } from './ampli';

const IS_PROD_ENV = import.meta.env.VITE_BUILD_ENV === 'production';

export const persistableStorage = new PersistableStorage<UserSession>();

const ApiKey = {
    production: 'placeholder-production-api-key',
    development: 'placeholder-development-api-key',
};

export async function initAmplitude() {
    await ampli.load({
        environment: 'iotaevmbridge',
        // Flip this if you'd like to test Amplitude locally
        disabled: !IS_PROD_ENV,
        client: {
            configuration: {
                cookieStorage: persistableStorage,
                logLevel: IS_PROD_ENV ? LogLevel.Warn : amplitude.Types.LogLevel.Debug,
            },
        },
    });

    window.addEventListener('pagehide', () => {
        amplitude.setTransport(TransportType.SendBeacon);
        amplitude.flush();
    });
}

export function getUrlWithDeviceId(url: URL) {
    const deviceId = ampli.client.getDeviceId();
    if (deviceId) {
        url.searchParams.set('amplitude_device_id', deviceId);
    }
    return url;
}