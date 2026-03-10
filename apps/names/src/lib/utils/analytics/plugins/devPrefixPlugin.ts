// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Development Prefix Plugin for Amplitude Analytics
 *
 * Prefixes all event names with 'dev_' in non-production environments.
 * This allows separating development events from production events in Amplitude.
 */

import type { BrowserConfig, EnrichmentPlugin, Event } from '@amplitude/analytics-types';

const IS_DEVELOPMENT = process.env.NEXT_PUBLIC_BUILD_ENV !== 'production';
const DEV_EVENT_PREFIX = 'dev_';

/**
 * Amplitude enrichment plugin that prefixes event names with 'dev_' in development.
 */
export function devPrefixPlugin(): EnrichmentPlugin {
    return {
        name: 'dev-prefix',
        type: 'enrichment',

        setup: async (_config: BrowserConfig) => {
            return Promise.resolve();
        },

        execute: async (event: Event): Promise<Event> => {
            if (
                IS_DEVELOPMENT &&
                event.event_type &&
                !event.event_type.startsWith(DEV_EVENT_PREFIX)
            ) {
                event.event_type = DEV_EVENT_PREFIX + event.event_type;
            }

            return event;
        },
    };
}
