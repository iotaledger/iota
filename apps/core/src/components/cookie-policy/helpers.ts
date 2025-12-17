// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { BrowserClient } from '@amplitude/analytics-types';
import { AMP_COOKIES_KEY } from './constants';

export function setCookieAccepted(): void {
    document.cookie = `${AMP_COOKIES_KEY}=true; max-age=31536000; path=/; SameSite=Strict`;
}

export function setCookieDeclined(): void {
    document.cookie = `${AMP_COOKIES_KEY}=false; path=/; SameSite=Strict`;
}

/**
 * Handle user accepting cookies.
 * @param ampliClient - Optional Amplitude client instance to call setOptOut on
 */
export function handleConsentAccepted(ampliClient?: BrowserClient): void {
    if (ampliClient) {
        ampliClient.setOptOut(false);
    }

    setCookieAccepted();
}

/**
 * Handle user declining cookies.
 * @param ampliClient - Optional Amplitude client instance to call setOptOut on
 */
export function handleConsentDeclined(ampliClient?: BrowserClient): void {
    if (ampliClient) {
        ampliClient.setOptOut(true);
        ampliClient.reset();
    }
    cleanAmplitudeCookies();
    setCookieDeclined();
}

/**
 * Check if user has previously given consent for cookies/tracking.
 */
export function getAmplitudeConsentStatus() {
    if (typeof document === 'undefined') return 'pending';
    if (document.cookie.includes(`${AMP_COOKIES_KEY}=true`)) return 'accepted';
    if (document.cookie.includes(`${AMP_COOKIES_KEY}=false`)) return 'declined';
    return 'pending';
}

export function cleanAmplitudeCookies() {
    const cookies = document.cookie.split(';');
    cookies.forEach((cookie) => {
        const cookieNameOrigin = cookie.split('=')[0].trim();
        const cookieNameLower = cookieNameOrigin.toLowerCase();
        // Clean amplitude tracking cookies but preserve our consent cookie
        if (cookieNameLower.startsWith('amp_') && cookieNameOrigin !== AMP_COOKIES_KEY) {
            document.cookie = `${cookieNameOrigin}=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/; SameSite=Strict`;
        }
    });
}
