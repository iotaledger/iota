// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook } from '@growthbook/growthbook';
import { getAppsBackend } from '@iota/iota-sdk/client';

export const growthbook = new GrowthBook({
    // If you want to develop locally, you can set the API host to this:
    apiHost: getAppsBackend(),
    clientKey: import.meta.env.PROD ? 'production' : 'development',
    enableDevMode: import.meta.env.DEV,
});
