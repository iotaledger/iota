// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook } from '@growthbook/growthbook';
import { getAppsBackend } from '@iota/core';

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

export const growthbook = new GrowthBook({
    apiHost: getAppsBackend(),
    ...GROWTHBOOK_ENVIRONMENTS[environment],
});
