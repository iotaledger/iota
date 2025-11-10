// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AMP_COOKIES_KEY } from '@iota/core';
import Browser from 'webextension-polyfill';

/**
 * Chrome extension-specific cookie consent management.
 * Uses Browser.storage.local for reliable persistence in extension contexts.
 */

/**
 * Set cookie consent to accepted in Chrome storage
 */
export async function setCookieConsentAccepted(): Promise<void> {
    try {
        await Browser.storage.local.set({ [AMP_COOKIES_KEY]: true });
    } catch (error) {
        console.error('Failed to set cookie consent in Browser storage:', error);
        throw error;
    }
}

/**
 * Get cookie consent status from Chrome storage
 * @returns Promise that resolves to true if cookies are accepted, false if declined, undefined if not set
 */
export async function getCookieConsent(): Promise<boolean | undefined> {
    try {
        const result = await Browser.storage.local.get([AMP_COOKIES_KEY]);
        return result[AMP_COOKIES_KEY];
    } catch (error) {
        console.error('Failed to get cookie consent from Browser storage:', error);
        return undefined;
    }
}

/**
 * Check if cookie consent has been set (either accepted or declined)
 * @returns Promise that resolves to true if consent has been set, false otherwise
 */
export async function hasCookieConsentBeenSet(): Promise<boolean> {
    const consent = await getCookieConsent();
    return consent !== undefined;
}

/**
 * Clear cookie consent from Chrome storage
 */
export async function clearCookieConsent(): Promise<void> {
    try {
        await Browser.storage.local.remove(AMP_COOKIES_KEY);
    } catch (error) {
        console.error('Failed to clear cookie consent from Browser storage:', error);
        throw error;
    }
}
