// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '@/lib/utils/analytics';
import {
    CookiePolicyContent,
    AMP_COOKIES_KEY,
    handleConsentAccepted,
    handleConsentDeclined,
} from '@iota/core';

export function CookiePolicy(): React.JSX.Element {
    function onAccept() {
        handleConsentAccepted(ampli.client);
    }

    function onReject() {
        handleConsentDeclined(ampli.client);
    }

    return (
        <CookiePolicyContent
            consentKey={AMP_COOKIES_KEY}
            necessaryCookies={[
                {
                    name: AMP_COOKIES_KEY,
                    purpose:
                        "Stores the user's Amplitude cookies consent state for the current domain",
                    provider: 'IOTA',
                    category: 'Analytics',
                    expiration: '1 year',
                },
            ]}
            additionalCookies={[
                {
                    name: 'AMP_*',
                    purpose:
                        'Stores anonymous session and device identifiers used by Amplitude to track user interactions and analytics data across your visits.',
                    provider: 'Amplitude',
                    category: 'Analytics',
                    expiration: '1 year',
                },
                {
                    name: 'AMP_MKTG_*',
                    purpose:
                        'Stores marketing attribution data including UTM parameters, referrer information, and click IDs to track campaign effectiveness.',
                    provider: 'Amplitude',
                    category: 'Analytics',
                    expiration: '1 year',
                },
            ]}
            onAccept={onAccept}
            onReject={onReject}
        />
    );
}
