// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, type MouseEventHandler } from 'react';
import { toast } from '@iota/core';

export type CopyOptions = {
    copySuccessMessage?: string;
};

export function useCopyToClipboard(
    textToCopy: string,
    { copySuccessMessage = 'Copied' }: CopyOptions = {},
) {
    return useCallback<MouseEventHandler>(
        async (e) => {
            e.stopPropagation();
            e.preventDefault();
            try {
                await navigator.clipboard.writeText(textToCopy);
                toast(copySuccessMessage);
            } catch (e) {
                // silence clipboard errors
            }
        },
        [textToCopy, copySuccessMessage],
    );
}
