// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook } from '@growthbook/growthbook';
import { Network, getAppsBackend } from '@iota/iota-sdk/client';
import Browser from 'webextension-polyfill';

const GROWTHBOOK_ENVIRONMENTS = {
    production: {
        clientKey: 'production',
    },
    rc: {
        clientKey: 'staging',
    },
    nightly: {
        clientKey: 'staging',
    },
    development: {
        clientKey: 'staging',
        enableDevMode: true,
        disableCache: true,
    },
};

const environment =
    (process.env.BUILD_ENV as keyof typeof GROWTHBOOK_ENVIRONMENTS) || 'development';

export const growthbook = new GrowthBook({
    apiHost: getAppsBackend(),
    ...GROWTHBOOK_ENVIRONMENTS[environment],
});

export function setAttributes(network?: { network: Network; customRpc?: string | null }) {
    const activeNetwork = network
        ? network.network === Network.Custom && network.customRpc
            ? network.customRpc
            : network.network.toUpperCase()
        : null;

    growthbook.setAttributes({
        network: activeNetwork,
        version: Browser.runtime.getManifest().version,
        rc: process.env.IS_RC || false,
    });
}

// Initialize growthbook to default attributes:
setAttributes();
