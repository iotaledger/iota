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
 * Anti-bot configuration: Events are queued but not sent until a human interaction is detected.
 * Sessions are classified as human on the first DOM interaction (scroll, mousemove, keydown, touchstart).
 */
const ANTI_BOT_CONFIG = {
    // Regular flush interval once the session is classified as human
    REGULAR_FLUSH_INTERVAL_MS: 1000,
    // Initial flush settings — effectively disabled so events queue locally until bot check passes
    INITIAL_FLUSH_INTERVAL_MS: 3600000, // 1 hour
    INITIAL_QUEUE_SIZE: 500,
} as const;

let isBotCleared = false;

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

const HUMAN_SIGNAL_EVENTS = ['scroll', 'mousemove', 'keydown', 'touchstart'] as const;

/**
 * Sets up anti-bot protection:
 * 1. Queues all events locally (1-hour flush interval prevents premature sends)
 * 2. Classifies the session as human on the first DOM interaction and enables regular flushing
 * 3. On page exit, beacon-flushes only if the session was classified as human
 */
function setupAntiBotProtection() {
    let flushInterval: ReturnType<typeof setInterval> | null = null;

    function enableFlushing() {
        if (isBotCleared) {
            return;
        }
        isBotCleared = true;
        ampli.flush();
        flushInterval = setInterval(() => {
            if (ampli.isLoaded) {
                ampli.flush();
            }
        }, ANTI_BOT_CONFIG.REGULAR_FLUSH_INTERVAL_MS);
    }

    const humanSignalController = new AbortController();
    const options = { passive: true, signal: humanSignalController.signal } as const;
    const handler = () => {
        humanSignalController.abort();
        enableFlushing();
    };
    for (const event of HUMAN_SIGNAL_EVENTS) {
        window.addEventListener(event, handler, options);
    }

    // Flush on page exit only if the session was classified as human.
    window.addEventListener(
        'pagehide',
        () => {
            humanSignalController.abort();

            if (flushInterval) {
                clearInterval(flushInterval);
            }

            if (isBotCleared) {
                ampli.client.setTransport('beacon');
                ampli.flush();
            }
        },
        { once: true },
    );
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
