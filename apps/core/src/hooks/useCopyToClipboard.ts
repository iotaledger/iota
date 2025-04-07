// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MouseEventHandler, useCallback } from 'react';
import { toast } from '../components/toaster';

export type CopyOptions = {
    textToCopy?: string;
    onSuccess?: () => void;
    successMessage?: string;
    showAddressWarning?: boolean;
};

export function useCopyToClipboard({
    textToCopy,
    onSuccess,
    successMessage = 'Copied',
    showAddressWarning = false,
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

                // Show warning if the address is copied
                if (showAddressWarning) {
                    toast.warning('Make sure that the address you copied is the correct one');
                }

                if (onSuccess) {
                    onSuccess();
                }

                return true;
            } catch (error) {
                return false;
            }
        },
        [textToCopy, onSuccess, successMessage, showAddressWarning],
    );
}
