// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CookieManager, type SKCMConfiguration } from '@boxfish-studio/react-cookie-manager';
import { handleConsentAccepted, handleConsentDeclined } from '@iota/core';
import { ampli } from '../../lib/utils/analytics';

const COOKIES_KEY = 'AMP_COOKIES_ACCEPTED';

export function CookieDisclaimer() {
    const configuration: SKCMConfiguration = {
        disclaimer: {
            title: undefined,
            body: 'We use cookies and analytics tools to help us improve your experience. ',
            policyText: 'Read our Cookie Policy',
            policyUrl: '/cookie-policy',
        },
        services: {
            customNecessaryCookies: [
                {
                    name: COOKIES_KEY,
                    purpose:
                        'Flag indicating that Amplitude analytics cookies may be created after consent',
                    expiry: '1 year',
                    type: 'http',
                    showDisclaimerIfMissing: true,
                },
            ],
        },
        onAcceptCookies: () => handleConsentAccepted(ampli.client),
        onDeclineCookies: () => handleConsentDeclined(ampli.client),
    };
    return (
        <>
            <CookieManager configuration={configuration} />
        </>
    );
}
