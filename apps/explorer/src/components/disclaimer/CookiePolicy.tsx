// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { CookieLibrary } from '@boxfish-studio/react-cookie-manager';
import { handleConsentAccepted, handleConsentDeclined } from '@iota/core';
import { ampli } from '../../lib/utils/analytics';

export function CookiePolicy(): React.JSX.Element {
    return (
        <section className="cookie-policy-page mx-auto max-w-3xl py-16">
            <CookieLibrary
                configuration={{
                    onAcceptCookies: () => handleConsentAccepted(ampli.client),
                    onDeclineCookies: () => handleConsentDeclined(ampli.client),
                }}
            />
        </section>
    );
}
