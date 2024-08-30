// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useProductAnalyticsConfig } from '@iota/core';
import { Text } from '@iota/ui';

import { LEGAL_LINKS } from '~/lib/constants';
import { Link } from '~/components/ui';

export function LegalText(): JSX.Element {
    return (
        <div className="flex justify-center md:justify-start">
            <Text color="steel-darker" variant="pSubtitleSmall/medium">
                &copy;
                {`${new Date().getFullYear()} <a href='https://www.iota.org/'>IOTA Stiftung</a>. The explorer is adapted from the <a href='https://github.com/MystenLabs/sui/tree/mainnet-v1.22.0/apps/explorer'>Sui Explorer</a>, © 2024 by <a href='https://mystenlabs.com/'>Mysten Labs</a>, licensed under <a href='https://github.com/MystenLabs/sui/blob/mainnet-v1.22.0/LICENSE'>Apache License 2.0</a>. All rights reserved.`}
            </Text>
        </div>
    );
}

export function LegalLinks(): JSX.Element {
    const { data: productAnalyticsConfig } = useProductAnalyticsConfig();

    return (
        <ul className="flex flex-col gap-3 md:flex-row md:gap-8">
            {LEGAL_LINKS.map(({ title, href }) => (
                <li className="flex items-center justify-center" key={href}>
                    <Link variant="text" href={href}>
                        <Text variant="subtitleSmall/medium" color="steel-darker">
                            {title}
                        </Text>
                    </Link>
                </li>
            ))}
            {productAnalyticsConfig?.mustProvideCookieConsent && (
                <li className="flex items-center justify-center">
                    <Link variant="text" data-cc="c-settings">
                        <Text variant="subtitleSmall/medium" color="steel-darker">
                            Manage Cookies
                        </Text>
                    </Link>
                </li>
            )}
        </ul>
    );
}
