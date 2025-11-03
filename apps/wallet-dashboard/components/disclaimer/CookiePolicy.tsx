// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { CookieLibrary } from '@boxfish-studio/react-cookie-manager';
import { useCookiesManager } from './useCookiesManager';

export function CookiePolicy(): React.JSX.Element {
    const { onAcceptCookies, onDeclineCookies } = useCookiesManager();
    return (
        <section className="cookie-policy-page mx-auto max-w-3xl py-16">
            <CookieLibrary
                configuration={{
                    onAcceptCookies,
                    onDeclineCookies,
                }}
            />
        </section>
    );
}
