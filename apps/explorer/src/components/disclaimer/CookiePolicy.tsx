// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    CookiePolicyContent,
    AMP_COOKIES_KEY,
    handleConsentAccepted,
    handleConsentDeclined,
} from '@iota/core';
import { ampli } from '~/lib/utils';

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
                    purpose: 'Session management cookie for IOTA applications',
                    provider: 'IOTA',
                    category: 'Necessary',
                },
                {
                    name: 'AMP_*',
                    purpose: 'Amplitude analytics cookies',
                    provider: 'Amplitude',
                    category: 'Analytics',
                },
            ]}
            onAccept={onAccept}
            onReject={onReject}
        />
    );
}
