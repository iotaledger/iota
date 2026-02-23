// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, type MouseEventHandler } from 'react';
import { toast } from '@iota/core';
import { ampli } from '_src/shared/analytics/ampli';

export type CopyOptions = {
    copySuccessMessage?: string;
    textType: string;
    trackEvent?: boolean;
    isPublic?: boolean;
};

export function useCopyToClipboard(
    textToCopy: string,
    { copySuccessMessage = 'Copied', textType, isPublic = false, trackEvent = true }: CopyOptions,
) {
    return useCallback<MouseEventHandler>(
        async (e) => {
            e.stopPropagation();
            e.preventDefault();
            try {
                await navigator.clipboard.writeText(textToCopy);
                toast(copySuccessMessage);
                if (trackEvent) {
                    ampli.elementCopied({
                        type: textType,
                        value: isPublic ? textToCopy : undefined,
                        visibility: isPublic ? 'public' : 'private',
                    });
                }
            } catch (e) {
                // silence clipboard errors
            }
        },
        [textToCopy, copySuccessMessage, textType, isPublic, trackEvent],
    );
}
