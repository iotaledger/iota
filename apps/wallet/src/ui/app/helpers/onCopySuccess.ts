// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toast } from '@iota/core';

import { ampli } from '_src/shared/analytics/ampli';

export interface CopySuccessOptions {
    successMessage: string;
    analyticType: string;
}

/**
 * Creates a callback function for clipboard copy success.
 * Handles toast notification and analytics tracking.
 *
 * @param options - Configuration for success message and analytics type
 * @returns A callback function to execute after successful copy
 *
 * @example
 * ```typescript
 * const handleCopySuccess = onCopySuccess({
 *     successMessage: 'Address copied',
 *     analyticType: 'address'
 * });
 *
 * <Address onCopySuccess={handleCopySuccess} />
 * ```
 */
export function onCopySuccess(options: CopySuccessOptions): () => void {
    return () => {
        toast(options.successMessage);
        ampli.elementCopied({ type: options.analyticType });
    };
}
