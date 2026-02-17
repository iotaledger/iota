// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BrowserClient, BrowserConfig, EnrichmentPlugin, Event } from '@amplitude/analytics-types';

/**
 * Amplitude Environment Plugin
 *
 * Prefixes event names with "dev_" for easy filtering in Amplitude when running in development mode.
 * This allows developers to test and debug events without polluting production analytics data.
 *
 */
export function attachEnvironmentPlugin(
    isDev: boolean,
): EnrichmentPlugin<BrowserClient, BrowserConfig> {
    const DEV_EVENT_PREFIX = 'dev_';

    return {
        name: 'environment-plugin',
        type: 'enrichment' as const,
        setup: async () => {},
        execute: async (event: Event) => {
            try {
                if (isDev && event.event_type && !event.event_type.startsWith(DEV_EVENT_PREFIX)) {
                    return { ...event, event_type: DEV_EVENT_PREFIX + event.event_type };
                }
                return event;
            } catch {
                return event;
            }
        },
    };
}
