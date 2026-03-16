// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, type MouseEventHandler } from 'react';
import { toast, useCopyToClipboard as useCopyToClipboardCore } from '@iota/core';
import { ampli } from '@/lib/utils/analytics';

export type CopyOptions = {
    copySuccessMessage?: string;
    analyticType?: string;
    trackEvent?: boolean;
};

/**
 * Returns a simple callback (no event parameter) for use with components that expect () => void
 * Use this variant when "copy" managed by component and need just callback onCopySuccess (e.g., KeyValueInfo's onCopySuccess)
 */
export function useCopySuccessCallback(
    copySuccessMessage = 'Copied to clipboard',
    analyticType?: string,
    trackEvent: boolean = true,
): () => void {
    return useCallback(() => {
        toast(copySuccessMessage);
        if (analyticType && trackEvent) {
            ampli.elementCopied({ type: analyticType });
        }
    }, [analyticType, copySuccessMessage, trackEvent]);
}

/**
 * Returns a MouseEventHandler callback for copying text to clipboard, with optional analytics tracking and success message
 * Use this variant when you need to handle the copy action directly in an onClick or similar event handler
 */
export function useCopyToClipboard(
    textToCopy: string,
    { copySuccessMessage = 'Copied', analyticType, trackEvent = true }: CopyOptions = {},
) {
    const copyToClipboardCore = useCopyToClipboardCore(() => {
        toast(copySuccessMessage);
        if (analyticType && trackEvent) {
            ampli.elementCopied({
                type: analyticType,
            });
        }
    });

    return useCallback<MouseEventHandler>(
        async (e) => {
            e?.stopPropagation?.();
            e?.preventDefault?.();
            await copyToClipboardCore(textToCopy);
        },
        [textToCopy, copyToClipboardCore],
    );
}
