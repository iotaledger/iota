// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook, setPolyfills } from '@growthbook/growthbook';
import { getAppsBackend } from '@iota/iota-sdk/client';

const GROWTHBOOK_ENVIRONMENTS = {
    mainnet: {
        clientKey: 'production',
    },
    testnet: {
        clientKey: 'staging',
        enableDevMode: true,
        disableCache: true,
    },
    alphanet: {
        clientKey: 'staging',
        enableDevMode: true,
        disableCache: true,
    },
};

const environment =
    (import.meta.env.VITE_EVM_BRIDGE_DEFAULT_NETWORK as keyof typeof GROWTHBOOK_ENVIRONMENTS) ||
    'testnet';

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
