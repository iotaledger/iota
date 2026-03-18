// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createSentryRequestInspector } from '@iota/core';
import { IotaClient, IotaHTTPTransport } from '@iota/iota-sdk/client';
import { CONFIG } from '../../config';
import { IS_SENTRY_ENABLED } from './sentry';

// NOTE: Prefer useIotaClient() in React components rather than calling this directly.
export const createIotaClient = (): IotaClient =>
    new IotaClient({
        transport: new IotaHTTPTransport({
            url: CONFIG.L1.rpcUrl,
            inspector: IS_SENTRY_ENABLED // Sentry dev hint: set VITE_SENTRY_ENABLED=true to enable
                ? createSentryRequestInspector(CONFIG.L1.rpcUrl)
                : undefined,
        }),
    });
