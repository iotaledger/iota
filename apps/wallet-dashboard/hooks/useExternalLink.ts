// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';
import { ampli } from '@/lib/utils/analytics';

export interface UseExternalLinkOptions {
    /** The type of external link for analytics tracking (e.g., 'documentation', 'application', 'marketplace') */
    type?: string;
    /** Whether to track the link opening event. Default: true */
    trackEvent?: boolean;
}

/**
 * Hook for programmatic external link opening with analytics tracking.
 * Use this when you need to open external links via window.open() with event tracking.
 *
 * For declarative links in JSX, use the ExternalLink component instead.
 *
 * @param url - The URL to open
 * @param options - Configuration options
 * @returns A callback function that opens the URL and tracks the event
 *
 * @example
 * ```tsx
 * const openDocs = useExternalLink('https://docs.example.com', {
 *   type: 'documentation'
 * });
 *
 * return <Button onClick={openDocs}>View Docs</Button>;
 * ```
 */
export function useExternalLink(url: string, options?: UseExternalLinkOptions) {
    const { type, trackEvent = true } = options || {};

    const open = useCallback(() => {
        if (trackEvent && type) {
            ampli.openedLink({ type });
        }
        window.open(url, '_blank', 'noopener,noreferrer');
    }, [url, type, trackEvent]);

    return open;
}
