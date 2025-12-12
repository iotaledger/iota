// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DescribedCookie } from './types';
import type { BrowserClient } from '@amplitude/analytics-types';
import { AMP_COOKIES_KEY } from './constants';

export function parseCookies(
    necessaryCookies: DescribedCookie[],
    additionalCookies: DescribedCookie[],
) {
    const allCookies: DescribedCookie[] = [];

    if (necessaryCookies && necessaryCookies.length > 0) {
        allCookies.push(...necessaryCookies);
    }

    if (additionalCookies && additionalCookies.length > 0) {
        allCookies.push(...additionalCookies);
    }
}

/**
 * Get all cookies from document.cookie
 * @returns Array of cookie objects with name and value
 */
function getAllCookies(): { name: string; value: string }[] {
    if (typeof document === 'undefined') {
        return [];
    }

    const cookieString = document.cookie;
    if (!cookieString) {
        return [];
    }

    return cookieString.split(';').map((cookie) => {
        const [name, ...valueParts] = cookie.trim().split('=');
        return {
            name: name.trim(),
            value: valueParts.join('=').trim(),
        };
    });
}

/**
 * Check if a cookie name matches a pattern
 * @param cookieName The actual cookie name
 * @param pattern The pattern to match (exact name or wildcard like "name*")
 * @returns True if the cookie matches the pattern
 */
function matchesCookiePattern(cookieName: string, pattern: string): boolean {
    if (pattern.endsWith('*')) {
        // Wildcard matching: "someName*" matches any cookie starting with "someName"
        const prefix = pattern.slice(0, -1);
        return cookieName.startsWith(prefix);
    }
    // Exact matching
    return cookieName === pattern;
}

/**
 * Categorize all browser cookies into necessary and additional lists
 * @param necessaryCookiePatterns List of cookie patterns for necessary cookies
 * @param additionalCookiePatterns List of cookie patterns for additional cookies
 * @returns Object with categorized necessary and additional cookies
 */
export function categorizeCookies(
    necessaryCookiePatterns: DescribedCookie[],
    additionalCookiePatterns: DescribedCookie[],
): {
    necessary: DescribedCookie[];
    additional: DescribedCookie[];
} {
    const allCookies = getAllCookies();
    const necessary: DescribedCookie[] = [];
    const additional: DescribedCookie[] = [];
    const processedCookies = new Set<string>();

    // Process each cookie from the browser
    for (const cookie of allCookies) {
        if (processedCookies.has(cookie.name)) {
            continue;
        }

        let matched = false;

        // Check against necessary cookies first
        for (const pattern of necessaryCookiePatterns) {
            if (matchesCookiePattern(cookie.name, pattern.name)) {
                necessary.push({
                    name: cookie.name,
                    value: cookie.value,
                    purpose: pattern.purpose,
                    category: pattern.category,
                    provider: pattern.provider,
                    expiration: pattern.expiration,
                });
                processedCookies.add(cookie.name);
                matched = true;
                break;
            }
        }

        if (matched) {
            continue;
        }

        // Check against additional cookies
        for (const pattern of additionalCookiePatterns) {
            if (matchesCookiePattern(cookie.name, pattern.name)) {
                additional.push({
                    name: cookie.name,
                    value: cookie.value,
                    purpose: pattern.purpose,
                    category: pattern.category,
                    provider: pattern.provider,
                    expiration: pattern.expiration,
                });
                processedCookies.add(cookie.name);
                matched = true;
                break;
            }
        }

        // If not found in either list, add to additional without description
        if (!matched) {
            additional.push({
                name: cookie.name,
                value: cookie.value,
            });
            processedCookies.add(cookie.name);
        }
    }

    return { necessary, additional };
}

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
    // Enable tracking
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
    cleanAmplitudeCookies();
    if (ampliClient) {
        ampliClient.setOptOut(true);
    }
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
