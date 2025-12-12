// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    handleConsentAccepted,
    CookiePolicyContent,
    AMP_COOKIES_KEY,
    handleConsentDeclined,
} from '@iota/core';

/**
 * Cookie Policy page - displays information about cookies we use
 * Purpose: Just render content showing how we use cookies
 * No banner management - that's handled separately
 */
export function CookiePolicy(): React.JSX.Element {
    return (
        <CookiePolicyContent
            consentKey={AMP_COOKIES_KEY}
            necessaryCookies={[
                {
                    name: AMP_COOKIES_KEY,
                    purpose: 'Session management cookie for IOTA applications',
                    provider: 'IOTA',
                    category: 'Necessary',
                },
            ]}
            additionalCookies={[
                {
                    name: 'AMP_*',
                    purpose: 'Amplitude analytics cookies',
                    provider: 'Amplitude',
                    category: 'Analytics',
                },
            ]}
            onAccept={handleConsentAccepted}
            onReject={handleConsentDeclined}
        />
    );
}
