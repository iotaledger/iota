// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { attachEnvironmentPlugin, getAmplitudeConsentStatus } from '@iota/core';

import { ampli } from './ampli';
import { LogLevel } from '@amplitude/analytics-types';

const IS_ENABLED =
    import.meta.env.VITE_BUILD_ENV === 'production' &&
    import.meta.env.VITE_AMPLITUDE_ENABLED === 'true';

const IS_DEV = import.meta.env.VITE_BUILD_ENV !== 'production';

let IS_BOT_CLEARED = false;

export async function initAmplitude() {
    const consentStatus = getAmplitudeConsentStatus();

    if (ampli.isLoaded || consentStatus === 'declined') {
        return;
    }

    await ampli.load({
        environment: 'iotaexplorer',
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
                logLevel: LogLevel.None,
                // Anti-bot: Hold events initially to filter crawlers
                flushIntervalMillis: 3600000, // 1 hour - effectively disabled
                flushQueueSize: 50, // Small queue sufficient for 2s of events
                identityStorage: 'localStorage',
            },
        },
    }).promise;

    ampli.client.add(attachEnvironmentPlugin(IS_DEV));

    let flushInterval: ReturnType<typeof setInterval> | null = null;

    // Flush events on page hide, if the bot has been cleared
    // to ensure events are sent before the page unloads
    window.addEventListener(
        'pagehide',
        () => {
            if (flushInterval) {
                clearInterval(flushInterval);
            }
            if (IS_BOT_CLEARED) {
                ampli.client.setTransport('beacon');
                ampli.client.flush();
            }
        },
        { once: true },
    );

    // Restore default flush behavior after a short delay to filter out bot traffic
    const BOT_WAIT_TIME = 2000;
    setTimeout(() => {
        IS_BOT_CLEARED = true;

        ampli.flush();

        const FLUSH_INTERVAL = 1000;
        flushInterval = setInterval(() => {
            if (ampli.isLoaded) {
                ampli.flush();
            }
        }, FLUSH_INTERVAL);
    }, BOT_WAIT_TIME);
}
