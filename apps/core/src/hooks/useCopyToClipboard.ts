// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MouseEventHandler, useCallback } from 'react';
import { toast } from '../components/toaster';

export type CopyOptions = {
    textToCopy?: string;
    onSuccess?: () => void;
    successMessage?: string;
};

export function useCopyToClipboard({
    textToCopy,
    onSuccess,
    successMessage = 'Copied',
}: CopyOptions) {
    return useCallback<MouseEventHandler>(
        async (e) => {
            e.stopPropagation();
            e.preventDefault();
            try {
                await navigator.clipboard.writeText(textToCopy || '');

                if (successMessage) {
                    toast(successMessage);
                }

                if (onSuccess) {
                    onSuccess();
                }

                return true;
            } catch (error) {
                return false;
            }
        },
        [textToCopy, onSuccess, successMessage],
    );
}
