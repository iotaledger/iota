// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';

/**
 * Base copy-to-clipboard hook with optional success callback.
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

export interface CopyDependencies {
    toast?: (message: string) => void;
    trackCopy?: (type: string) => void;
}

export interface CopyOptions {
    successMessage?: string;
    analyticType?: string;
    trackEvent?: boolean;
}

/**
 * Factory to create copy-to-clipboard hooks with injected dependencies.
 * Enables decoupling from specific toast/analytics implementations.
 *
 * @example
 * ```typescript
 * export const { useCopyToClipboard, useCopySuccessCallback } = createCopyToClipboardHooks({
 *     toast: (message) => toast(message),
 *     trackCopy: (type) => ampli.elementCopied({ type }),
 * });
 * ```
 */
export function createCopyToClipboardHooks(dependencies: CopyDependencies) {
    const triggerSuccess = (options?: CopyOptions) => {
        if (options?.successMessage && dependencies.toast) {
            dependencies.toast(options.successMessage);
        }

        if (options?.trackEvent !== false && options?.analyticType && dependencies.trackCopy) {
            dependencies.trackCopy(options.analyticType);
        }
    };

    // For direct button clicks - handles clipboard write + success actions
    function useCopyToClipboard(text: string, options?: CopyOptions) {
        return useCallback(
            async (e?: React.MouseEvent) => {
                e?.stopPropagation?.();
                e?.preventDefault?.();

                if (!navigator?.clipboard) {
                    return false;
                }

                try {
                    await navigator.clipboard.writeText(text);
                    triggerSuccess(options);
                    return true;
                } catch (error) {
                    return false;
                }
            },
            [text, options],
        );
    }

    // For components that handle clipboard internally (KeyValueInfo, Address, etc.)
    function useCopySuccessCallback(options?: CopyOptions) {
        return useCallback(() => {
            triggerSuccess(options);
        }, [options]);
    }

    return {
        useCopyToClipboard,
        useCopySuccessCallback,
    };
}
