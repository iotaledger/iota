// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { toast } from 'react-hot-toast';

import { CookiePolicyContent } from '@/components/cookie-policy/CookiePolicyContent';
import {
    onAmplitudeConsentAccepted,
    onAmplitudeConsentDeclined,
} from '@/lib/utils/analytics/amplitude';
import { AMP_COOKIES_KEY } from '@/lib/utils/analytics/constants';

export default function CookiePolicy() {
    async function handleConsentAccepted() {
        await onAmplitudeConsentAccepted();
        toast.success('Your cookie preferences have been saved.');
    }

    async function handleConsentDeclined() {
        onAmplitudeConsentDeclined();
        toast.success('Your cookie preferences have been saved.');
    }
    return (
        <section className="cookie-policy-page">
            <CookiePolicyContent
                consentKey={AMP_COOKIES_KEY}
                necessaryCookies={[
                    {
                        name: AMP_COOKIES_KEY,
                        purpose:
                            "Stores the user's Amplitude cookies consent state for the current domain",
                        provider: 'IOTA',
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
                onAccept={handleConsentAccepted}
                onReject={handleConsentDeclined}
            />
        </section>
    );
}
