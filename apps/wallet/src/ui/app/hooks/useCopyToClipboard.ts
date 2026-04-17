// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, type MouseEventHandler } from 'react';
import { useCopyToClipboard as useCopyToClipboardCore } from '@iota/core';
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
        if (textType && trackEvent) {
            ampli.copiedElement({
                type: textType,
            });
        }
    }, copySuccessMessage);

    return useCallback<MouseEventHandler>(
        async (e) => {
            e.stopPropagation();
            e.preventDefault();
            await copyToClipboardCore(textToCopy);
        },
        [textToCopy, copyToClipboardCore],
    );
}
