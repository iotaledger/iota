// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { BrowserClient } from '@amplitude/analytics-types';
import { consentBufferPlugin } from './consentBufferPlugin';
import { AMP_COOKIES_KEY } from './constants';

export function setCookieAccepted(): void {
    document.cookie = `${AMP_COOKIES_KEY}=true; max-age=31536000; path=/; SameSite=Strict`;
}

export function setCookieDeclined(): void {
    document.cookie = `${AMP_COOKIES_KEY}=false; max-age=31536000; path=/; SameSite=Strict`;
}

/**
 * Handle user accepting cookies.
 * This will:
 * 1. Flush all buffered Amplitude events
 * 2. Enable Amplitude tracking (setOptOut false)
 * 3. Set the consent cookie
 *
 * @param ampliClient - Optional Amplitude client instance to call setOptOut on
 */
export function handleConsentAccepted(ampliClient?: BrowserClient): void {
    // Flush buffered events to Amplitude
    consentBufferPlugin.flushQueue();

    // Enable tracking
    if (ampliClient) {
        ampliClient.setOptOut(false);
    }

    setCookieAccepted();
}

/**
 * Handle user declining cookies.
 * This will:
 * 1. Clear all buffered Amplitude events
 * 2. Disable Amplitude tracking (setOptOut true)
 * 3. Set the consent cookie to false
 *
 * @param ampliClient - Optional Amplitude client instance to call setOptOut on
 */
export function handleConsentDeclined(ampliClient?: BrowserClient): void {
    // Clear buffered events
    consentBufferPlugin.clearQueue();

    // Disable tracking
    if (ampliClient) {
        ampliClient.setOptOut(true);
    }

    setCookieDeclined();
}
