// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, type MouseEventHandler } from 'react';
import { toast, useCopyToClipboard as useCopyToClipboardCore } from '@iota/core';
import { ampli } from '_src/shared/analytics/ampli';

export type CopyOptions = {
    copySuccessMessage?: string;
    textType?: string;
    trackEvent?: boolean;
};

export function useCopyToClipboard(
    textToCopy: string,
    { copySuccessMessage = 'Copied', textType, trackEvent = true }: CopyOptions,
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
