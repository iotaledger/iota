// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';

/**
 * Copy-to-clipboard hook with optional success callback.
 *
 * @param onSuccessCallback - Optional callback to execute after successful copy
 * @returns A function that accepts text to copy and returns a promise that resolves to boolean
 *
 * @example
 * ```typescript
 * const copyToClipboard = useCopyToClipboard(() => {
 *     toast('Copied!');
 *     ampli.elementCopied({ type: 'address' });
 * });
 *
 * <Button onClick={() => copyToClipboard(address)} />
 * ```
 */
export function useCopyToClipboard(onSuccessCallback?: () => void) {
    return useCallback(
        async (text: string) => {
            if (!navigator?.clipboard) {
                return false;
            }

            try {
                await navigator.clipboard.writeText(text);
                if (onSuccessCallback) {
                    onSuccessCallback();
                }
                return true;
            } catch (error) {
                return false;
            }
        },
        [onSuccessCallback],
    );
}
