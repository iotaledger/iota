// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel } from '@amplitude/analytics-core';
import { plugin as engagementPlugin } from '@amplitude/engagement-browser';
import { sessionReplayPlugin } from '@amplitude/plugin-session-replay-browser';

import { ampli } from './ampli';
import { AMP_COOKIES_KEY } from './constants';
import { devPrefixPlugin } from './plugins/devPrefixPlugin';

// Dev note: do not change, production-only gate (Session Replay must never run outside prod)
const IS_PRODUCTION = process.env.NEXT_PUBLIC_BUILD_ENV === 'production';

// Dev note: set IS_ENABLED=true to test Amplitude events locally
const IS_ENABLED = IS_PRODUCTION && process.env.NEXT_PUBLIC_AMPLITUDE_ENABLED === 'true';

/**
 * Check if user has previously given consent for cookies/tracking.
 */
export function getAmplitudeConsentStatus() {
    if (typeof document === 'undefined') return 'pending';
    if (document.cookie.includes(`${AMP_COOKIES_KEY}=true`)) return 'accepted';
    if (document.cookie.includes(`${AMP_COOKIES_KEY}=false`)) return 'declined';
    return 'pending';
}

/**
 * Initialize Amplitude.
 * This should be called once when the app starts.
 * Multiple calls to this function are safe - subsequent calls will be ignored.
 */
export async function initAmplitude() {
    if (ampli.isLoaded) {
        return;
    }

    try {
        await ampli.load({
            environment: 'iotanames',
            disabled: !IS_ENABLED,
            client: {
                configuration: {
                    autocapture: {
                        attribution: IS_ENABLED,
                        fileDownloads: false,
                        formInteractions: false,
                        pageViews: IS_ENABLED,
                        sessions: IS_ENABLED,
                        elementInteractions: IS_ENABLED,
                        frustrationInteractions: false,
                        networkTracking: false,
                        webVitals: false,
                        pageUrlEnrichment: IS_ENABLED,
                    },
                    logLevel: LogLevel.None,
                },
            },
        }).promise;

        window.addEventListener('pagehide', () => {
            amplitude.setTransport('beacon');
            amplitude.flush();
        });

        ampli.client.add(engagementPlugin({ serverZone: 'EU' }));

        if (IS_ENABLED) {
            // Add dev prefix plugin to prefix events with 'dev_' in non-production environments
            ampli.client.add(devPrefixPlugin());
            // Session Replay runs only in production (safety gate to avoid polluting recordings)
            if (IS_PRODUCTION) {
                const sessionReplayTracking = sessionReplayPlugin({
                    sampleRate: 1, // set to 1 to capture all sessions; adjust as needed (e.g., 0.1 for 10%)
                    privacyConfig: {
                        defaultMaskLevel: 'medium',
                        maskSelector: [
                            '.amp-obfuscation', // any element with this class will be masked according to the defaultMaskLevel
                            // specific selectors for the dropdown menu (comes from @iota/dapp-kit) to ensure it's fully masked
                            '[data-radix-popper-content-wrapper]',
                            '[class*="AccountDropdownMenu"]',
                        ],
                    },
                });
                ampli.client.add(sessionReplayTracking);
            }
        }
    } catch (error) {
        console.error('[Amplitude] Initialization failed:', error);
        throw error;
    }
}

/**
 * Call this function when user gives consent for cookies/tracking.
 * This will enable future event tracking.
 */
export async function onAmplitudeConsentAccepted() {
    setAmpliConsentAccepted();
    return initAmplitude();
}

export function setAmpliConsentAccepted() {
    document.cookie = `${AMP_COOKIES_KEY}=true; max-age=31536000; path=/; SameSite=Strict`;
}

/**
 * Call this function when user declines cookies/tracking.
 * This will disable event tracking and clear all Amplitude data.
 */
export function onAmplitudeConsentDeclined() {
    // First, opt out to prevent Amplitude from creating new cookies
    ampli.client.setOptOut(true);
    // Clear all Amplitude storage (localStorage and cookies)
    ampli.client.reset();
    cleanAmplitudeCookies();
    document.cookie = `${AMP_COOKIES_KEY}=false; max-age=31536000; path=/; SameSite=Strict`;
}

export function cleanAmplitudeCookies() {
    const cookies = document.cookie.split(';');
    cookies.forEach((cookie) => {
        const cookieNameOrigin = cookie.split('=')[0].trim();
        const cookieNameLower = cookieNameOrigin.toLowerCase();
        if (
            cookieNameLower.startsWith('amp_') &&
            cookieNameLower !== AMP_COOKIES_KEY.toLowerCase()
        ) {
            document.cookie = `${cookieNameOrigin}=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/`;
            document.cookie = `${cookieNameOrigin}=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/; domain=${window.location.hostname}`;
            const domainParts = window.location.hostname.split('.');
            if (domainParts.length > 2) {
                const parentDomain = domainParts.slice(-2).join('.');
                document.cookie = `${cookieNameOrigin}=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/; domain=.${parentDomain}`;
            }
        }
    });
}
