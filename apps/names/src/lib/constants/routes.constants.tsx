// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Discord, SmX } from '@iota/apps-ui-icons';

import { RouteTitle } from '../enums';
import type { Route } from '../interfaces';

export const MY_NAMES_ROUTE: Route = {
    title: RouteTitle.MyNames,
    path: '/my-names',
    id: 'my-names',
};

export const AUCTION_ROUTE: Route = {
    title: RouteTitle.Auction,
    path: '/auctions',
    id: 'auctions',
};

export const DOCS_ROUTE: Route = {
    title: RouteTitle.Documentation,
    path: 'https://docs.iotanames.com',
    isExternal: true,
};

export const PROTECTED_ROUTES = [MY_NAMES_ROUTE] as const satisfies Route[];

export const PUBLIC_ROUTES: Route[] = [DOCS_ROUTE] as const satisfies Route[];

export const CONNECT_ROUTE: Route = {
    path: '/',
};

export const FOOTER_SOCIAL_LINKS: Route[] = [
    {
        path: 'https://discord.iota.org/',
        icon: <Discord />,
        title: 'IOTA Discord',
    },
    {
        path: 'https://x.com/iotanames',
        icon: <SmX />,
        title: 'IOTA Names on X',
    },
];

export const FOOTER_LEGAL_LINKS: Route[] = [
    {
        title: 'Terms & Conditions',
        path: '/terms-of-service',
    },
    {
        title: 'Privacy Policy',
        path: '/privacy-policy',
    },
    {
        title: 'Cookie Policy',
        path: '/cookie-policy',
    },
];
