// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BrowserClient, BrowserConfig, EnrichmentPlugin, Event } from '@amplitude/analytics-types';

const IS_DEVELOPMENT = process.env.NEXT_PUBLIC_BUILD_ENV !== 'production';

const DEV_EVENT_PREFIX = 'dev_';

/**
 * Amplitude Environment Plugin
 *
 * Prefixes event names with "dev_" for easy filtering in Amplitude when running in development mode.
 * This allows developers to test and debug events without polluting production analytics data.
 *
 */
export function attachEnvironmentPlugin(): EnrichmentPlugin<BrowserClient, BrowserConfig> {
    return {
        name: 'environment-plugin',
        type: 'enrichment' as const,
        setup: async () => {},
        execute: async (context: Event) => {
            // Prefix event name for development
            if (
                IS_DEVELOPMENT &&
                context.event_type &&
                !context.event_type.startsWith(DEV_EVENT_PREFIX)
            ) {
                context.event_type = DEV_EVENT_PREFIX + context.event_type;
            }

            return context;
        },
    };
}
