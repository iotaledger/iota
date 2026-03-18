// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook, setPolyfills } from '@growthbook/growthbook';
import { getAppsBackend } from '@iota/iota-sdk/client';

const GROWTHBOOK_ENVIRONMENTS = {
    production: {
        clientKey: 'production',
    },
    staging: {
        clientKey: 'staging',
    },
    development: {
        clientKey: 'staging',
        enableDevMode: true,
        disableCache: true,
    },
};

const environment =
    (import.meta.env.VITE_BUILD_ENV as keyof typeof GROWTHBOOK_ENVIRONMENTS) || 'development';

const version = import.meta.env.VITE_APP_VERSION;
if (version) {
    setPolyfills({
        fetch: (url: string, init?: RequestInit) => {
            const separator = url.includes('?') ? '&' : '?';
            return fetch(`${url}${separator}version=${version}`, init);
        },
    });
}

export const growthbook = new GrowthBook({
    apiHost: getAppsBackend(),
    ...GROWTHBOOK_ENVIRONMENTS[environment],
});
