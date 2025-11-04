// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { CookieLibrary } from '@boxfish-studio/react-cookie-manager';
import { handleConsentAccepted, handleConsentDeclined } from '@iota/core';
import { ampli } from '../../shared/analytics';

export function CookiePolicy(): React.JSX.Element {
    return (
        <section className="py-16 max-w-3xl mx-auto cookie-policy-page">
            <CookieLibrary
                configuration={{
                    onAcceptCookies: () => handleConsentAccepted(ampli.client),
                    onDeclineCookies: () => handleConsentDeclined(ampli.client),
                }}
            />
        </section>
    );
}
