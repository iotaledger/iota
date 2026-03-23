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

const IS_DEV = import.meta.env.VITE_BUILD_ENV !== 'production';

/**
 * Anti-bot configuration: Events are queued but not sent initially.
 * After BOT_DETECTION_DELAY, we assume the user is human and start sending events.
 */
const ANTI_BOT_CONFIG = {
    // Detection delay: bots typically leave within 2 seconds
    DETECTION_DELAY_MS: 2000,
    // Regular flush interval after bot detection passes
    REGULAR_FLUSH_INTERVAL_MS: 1000,
    // Initial flush settings (effectively disabled to queue events locally)
    INITIAL_FLUSH_INTERVAL_MS: 3600000, // 1 hour
    INITIAL_QUEUE_SIZE: 50,
} as const;

let IS_BOT_CLEARED = false;

export async function initAmplitude() {
    const consentStatus = getAmplitudeConsentStatus();

    if (ampli.isLoaded || consentStatus === 'declined') {
        return;
    }

    // Load Amplitude with anti-bot flush settings
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
                    elementInteractions: IS_ENABLED,
                    frustrationInteractions: false,
                    networkTracking: false,
                    webVitals: false,
                    pageUrlEnrichment: IS_ENABLED,
                },
                logLevel: LogLevel.None,
                flushIntervalMillis: ANTI_BOT_CONFIG.INITIAL_FLUSH_INTERVAL_MS,
                flushQueueSize: ANTI_BOT_CONFIG.INITIAL_QUEUE_SIZE,
                identityStorage: 'localStorage',
            },
        },
    }).promise;

    ampli.client.add(attachEnvironmentPlugin(IS_DEV));

    setupAntiBotProtection();
}

/**
 * Sets up anti-bot protection by:
 * 1. Queueing events initially without sending them
 * 2. After DETECTION_DELAY_MS, marking user as human and flushing events
 * 3. Starting regular flush intervals for subsequent events
 * 4. Handling page exit to flush remaining events
 */
function setupAntiBotProtection() {
    let flushInterval: ReturnType<typeof setInterval> | null = null;

    // Handle page exit: only flush if user passed bot detection
    window.addEventListener(
        'pagehide',
        () => {
            if (flushInterval) {
                clearInterval(flushInterval);
            }

            if (IS_BOT_CLEARED) {
                ampli.client.setTransport('beacon');
                ampli.flush();
            }
        },
        { once: true },
    );

    // After delay, assume user is human and enable regular flushing
    setTimeout(() => {
        IS_BOT_CLEARED = true;
        ampli.flush(); // Send all queued events

        // Start regular flushing since Amplitude's config can't be changed after init
        flushInterval = setInterval(() => {
            if (ampli.isLoaded) {
                ampli.flush();
            }
        }, ANTI_BOT_CONFIG.REGULAR_FLUSH_INTERVAL_MS);
    }, ANTI_BOT_CONFIG.DETECTION_DELAY_MS);
}

/**
 * Set the Amplitude user identity with the current network context.
 * Updates user property: network.
 * This allows filtering and segmenting analytics events by network dimension.
 */
export function setAmplitudeIdentity(network: string): void {
    if (!ampli.isLoaded) {
        return;
    }

    const identifyEvent = new amplitude.Identify();
    identifyEvent.set('network', network);

    ampli.client.identify(identifyEvent);
}
