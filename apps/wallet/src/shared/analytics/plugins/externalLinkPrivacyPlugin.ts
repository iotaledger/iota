// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { EnrichmentPlugin, Event } from '@amplitude/analytics-types';

export const PUBLIC_TYPES = new Set<string>(['documentation', 'application', 'support']);
// these are the types that are always private
export const PRIVATE_TYPES = new Set<string>(['address', 'digest', 'object']);

export function externalLinkOpenedPrivacyPlugin(): EnrichmentPlugin {
    return {
        name: 'external-link-opened-privacy',
        type: 'enrichment',

        async execute(event: Event) {
            if (!event.event_type?.endsWith('external link opened')) {
                return event;
            }

            let props = { ...(event.event_properties ?? {}) } as Record<string, unknown>;

            const type =
                typeof props.type === 'string' && props.type.trim() ? props.type : 'unknown';

            let visibility =
                props.visibility === 'public' || props.visibility === 'private'
                    ? (props.visibility as 'public' | 'private')
                    : 'private';

            if (PRIVATE_TYPES.has(type)) {
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
