// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { EnrichmentPlugin, Event } from '@amplitude/analytics-types';

export function externalLinkOpenedPrivacyPlugin(): EnrichmentPlugin {
    return {
        name: 'external-link-opened-privacy',
        type: 'enrichment',

        async execute(event: Event) {
            if (event.event_type !== 'external link opened') return event;

            const props = { ...(event.event_properties ?? {}) } as Record<string, unknown>;

            const visibility =
                props.visibility === 'public' || props.visibility === 'private'
                    ? (props.visibility as 'public' | 'private')
                    : 'private';

            props.visibility = visibility;

            if (visibility === 'private') delete props.value;

            return {
                ...event,
                event_properties: props,
            };
        },
    };
}
