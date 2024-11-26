// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useProductAnalyticsConfig } from '@iota/core';
import { LEGAL_LINKS } from '~/lib/constants';
import { Link } from '~/components/ui';

export function LegalText(): JSX.Element {
    return (
        <div className="flex justify-center md:justify-start">
            <span className="text-body-md text-neutral-40">
                &copy;
                {`${new Date().getFullYear()} IOTA Stiftung. All rights reserved.`}
            </span>
        </div>
    );
}

export function LegalLinks(): JSX.Element {
    const { data: productAnalyticsConfig } = useProductAnalyticsConfig();

    return (
        <ul className="flex flex-col gap-3 md:flex-row md:gap-8">
            {LEGAL_LINKS.map(({ title, href }) => (
                <li className="flex items-center justify-center" key={href}>
                    <Link variant="text" href={href} className="text-body-md text-neutral-40">
                        {title}
                    </Link>
                </li>
            ))}
            {productAnalyticsConfig?.mustProvideCookieConsent && (
                <li className="flex items-center justify-center">
                    <Link
                        variant="text"
                        data-cc="c-settings"
                        className="text-body-md text-neutral-40"
                    >
                        Manage Cookies
                    </Link>
                </li>
            )}
        </ul>
    );
}
