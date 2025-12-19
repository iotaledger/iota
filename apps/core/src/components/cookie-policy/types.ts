// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Represents a cookie that was detected in the browser
 */
export interface DescribedCookie {
    /**
     * Actual cookie name found in document.cookie
     */
    name: string;

    /**
     * Purpose of this cookie
     */
    purpose?: string;

    /**
     * Optional category
     */
    category?: string;

    /**
     * Provider that sets this cookie
     */
    provider?: string;

    /**
     * Cookie value (optional, for debugging)
     */
    value?: string;

    /**
     * Cookie expiration (optional, human-readable or date string)
     */
    expiration?: string;
}

export interface CookiePolicyContentProps {
    /**
     * Cookie name for storing consent state (e.g., 'AMP_COOKIES_ACCEPTED')
     */
    consentKey?: string;

    /**
     * Cookies for necessary category
     */
    necessaryCookies?: DescribedCookie[];

    /**
     * Cookies for additional category
     */
    additionalCookies?: DescribedCookie[];

    /**
     * Optional title override
     */
    title?: string;

    /**
     * Optional description override
     */
    description?: string;

    /**
     * Callback fired when user accepts additional cookies
     */
    onAccept: () => void;

    /**
     * Callback fired when user rejects additional cookies
     */
    onReject: () => void;
}
