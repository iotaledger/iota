// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook } from '@growthbook/growthbook';
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
    (process.env.NEXT_PUBLIC_BUILD_ENV as keyof typeof GROWTHBOOK_ENVIRONMENTS) || 'production';

export const growthbook = new GrowthBook({
    // If you want to develop locally, you can set the API host to this:
    apiHost: getAppsBackend(),
    ...GROWTHBOOK_ENVIRONMENTS[environment],
});
