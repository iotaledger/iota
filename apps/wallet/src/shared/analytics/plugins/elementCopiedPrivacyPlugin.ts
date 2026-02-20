// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { EnrichmentPlugin, Event } from '@amplitude/analytics-types';

// these are the types that are always private
const PRIVATE_TYPES = new Set<string>(['address', 'digest', 'object', 'mnemonic']);

export function elementCopiedPrivacyPlugin(): EnrichmentPlugin {
    return {
        name: 'element-copied-privacy',
        type: 'enrichment',

        async execute(event: Event) {
            if (!event.event_type?.endsWith('element copied')) {
                return event;
            }

            let props = { ...(event.event_properties ?? {}) } as Record<string, unknown>;

            const type =
                typeof props.type === 'string' && props.type.trim() ? props.type : 'unknown';

            let visibility: 'private' | 'public' =
                props.visibility === 'public' ? 'public' : 'private';

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
