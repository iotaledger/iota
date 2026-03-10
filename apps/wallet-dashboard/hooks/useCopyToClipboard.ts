// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, type MouseEventHandler } from 'react';
import { toast, useCopyToClipboard as useCopyToClipboardCore } from '@iota/core';
import { ampli } from '@/lib/utils/analytics';

export type CopyOptions = {
    /**
     * Success message to show when copy succeeds
     * @default 'Copied'
     */
    copySuccessMessage?: string;
    /**
     * Type of element being copied for analytics tracking
     * When provided, automatically tracks ampli.elementCopied event
     */
    textType?: string;
    /**
     * Whether to track the copy event in analytics
     * @default true
     */
    trackEvent?: boolean;
};

/**
 * Custom hook that wraps @iota/core's useCopyToClipboard with automatic analytics tracking.
 *
 * This hook provides a MouseEventHandler that:
 * - Copies text to clipboard
 * - Shows a success toast message
 * - Automatically tracks analytics events when textType is provided
 * - Prevents default event behavior and stops propagation
 *
 * @param textToCopy - The text to copy to clipboard
 * @param options - Configuration options for copy behavior
 * @returns MouseEventHandler that can be used directly in onClick handlers
 *
 * @example
 * ```tsx
 * const onCopySuccess = useCopyToClipboard(address, {
 *   copySuccessMessage: 'Address copied',
 *   textType: 'address'
 * });
 *
 * <button onClick={onCopySuccess}>Copy</button>
 * ```
 */
export function useCopyToClipboard(
    textToCopy: string,
    { copySuccessMessage = 'Copied', textType, trackEvent = true }: CopyOptions = {},
) {
    const copyToClipboardCore = useCopyToClipboardCore(() => {
        toast(copySuccessMessage);
        if (textType && trackEvent) {
            ampli.elementCopied({
                type: textType,
            });
        }
    });

    return useCallback<MouseEventHandler>(
        async (e) => {
            e.stopPropagation();
            e.preventDefault();
            await copyToClipboardCore(textToCopy);
        },
        [textToCopy, copyToClipboardCore],
    );
}
