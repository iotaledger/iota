// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook } from '@growthbook/growthbook';
import { getAppsBackend } from '@iota/iota-sdk/client';

export const growthbook = new GrowthBook({
    // If you want to develop locally, you can set the API host to this:
    apiHost: getAppsBackend(),
    clientKey: process.env.NODE_ENV === 'production' ? 'production' : 'development',
    enableDevMode: process.env.NODE_ENV === 'development',
});
