// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';
import { toast } from '../components/toaster';

export function useCopyToClipboard(onSuccessCallback?: () => void, successMessage?: string) {
    return useCallback(
        async (text: string) => {
            if (!navigator?.clipboard) {
                return false;
            }

            try {
                await navigator.clipboard.writeText(text);
                toast(successMessage || 'Copied to clipboard');
                if (onSuccessCallback) {
                    onSuccessCallback();
                }
                return true;
            } catch (error) {
                return false;
            }
        },
        [successMessage, onSuccessCallback],
    );
}
