// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, type MouseEventHandler } from 'react';
import { toast } from '@iota/core';
import { ampli } from '_src/shared/analytics/ampli';

export type CopyOptions = {
    copySuccessMessage?: string;
    textType?: string;
};

export function useCopyToClipboard(
    textToCopy: string,
    { copySuccessMessage = 'Copied', textType }: CopyOptions,
) {
    return useCallback<MouseEventHandler>(
        async (e) => {
            e.stopPropagation();
            e.preventDefault();
            try {
                await navigator.clipboard.writeText(textToCopy);
                toast(copySuccessMessage);
                if (textType) {
                    ampli.elementCopied({
                        type: textType,
                    });
                }
            } catch (e) {
                // silence clipboard errors
            }
        },
        [textToCopy, copySuccessMessage, textType],
    );
}
