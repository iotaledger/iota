// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook } from '@growthbook/growthbook';
import { getAppsBackend } from '@iota/iota-sdk/client';

const GROWTHBOOK_ENVIRONMENTS = {
    production: {
        clientKey: 'production',
        enableDevMode: false,
    },
    staging: {
        clientKey: 'staging',
        enableDevMode: false,
    },
    development: {
        clientKey: 'staging',
        enableDevMode: true,
    },
};

const environment =
    (process.env.NEXT_PUBLIC_BUILD_ENV as keyof typeof GROWTHBOOK_ENVIRONMENTS) || 'development';

export const growthbook = new GrowthBook({
    apiHost: getAppsBackend(),
    ...GROWTHBOOK_ENVIRONMENTS[environment],
});
