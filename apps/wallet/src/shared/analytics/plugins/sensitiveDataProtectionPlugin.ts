// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { EnrichmentPlugin, Event } from '@amplitude/analytics-types';

/**
 * Sensitive Data Protection Plugin
 * - Guarantees that certain data types are always private
 * - Automatically cleans value
 * - Dynamic name for reuse across different event types
 */
export function sensitiveDataProtectionPlugin(
    eventType: string,
    privateTypes: Set<string>,
): EnrichmentPlugin {
    const PRIVATE_TYPES_NORMALIZED = new Set(Array.from(privateTypes).map((t) => t.toLowerCase()));

    return {
        name: `sensitive-data-protection-${eventType.replace(/\s+/g, '-')}`,
        type: 'enrichment',

        async execute(event: Event) {
            if (!event.event_type?.endsWith(eventType)) {
                return event;
            }

            let props = { ...(event.event_properties ?? {}) } as Record<string, unknown>;

            const type =
                typeof props.type === 'string' && props.type.trim()
                    ? props.type.trim().toLowerCase()
                    : 'unknown';

            // By default private, unless explicitly 'public'
            let visibility: 'private' | 'public' =
                props.visibility === 'public' ? 'public' : 'private';

            // Force to private if it's a sensitive type
            if (PRIVATE_TYPES_NORMALIZED.has(type)) {
                visibility = 'private';
            }

            props.type = type;
            props.visibility = visibility;

            if (visibility === 'private') {
                const { value, ...rest } = props;
                props = rest;
            }

            return {
                ...event,
                event_properties: { ...props },
            };
        },
    };
}
