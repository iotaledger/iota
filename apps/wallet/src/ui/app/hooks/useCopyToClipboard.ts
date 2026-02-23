// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, type MouseEventHandler } from 'react';
import { toast, useCopyToClipboard as useCopyToClipboardCore } from '@iota/core';
import { ampli } from '_src/shared/analytics/ampli';

export type CopyOptions = {
    copySuccessMessage?: string;
    /**
     * Type of text being copied for analytics tracking.
     * When provided, automatically tracks the copy event.
     * Omit this prop to disable tracking (e.g., when using custom analytics).
     */
    textType?: string;
};

/**
 * Wrapper around @iota/core's useCopyToClipboard that adds toast notifications and analytics tracking.
 * This provides a generic solution for copy functionality across the wallet.
 */
export function useCopyToClipboard(
    textToCopy: string,
    { copySuccessMessage = 'Copied', textType }: CopyOptions,
) {
    const copyToClipboardCore = useCopyToClipboardCore(() => {
        toast(copySuccessMessage);
        if (textType) {
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
