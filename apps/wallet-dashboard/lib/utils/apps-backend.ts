// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AppsBackendClient } from '@iota/apps-backend-client';
import { getAppsBackend } from '@iota/iota-sdk/client';

export const appsBackendClient = new AppsBackendClient({
    url: getAppsBackend(),
});
