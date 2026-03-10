// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DEFAULT_ERROR_MESSAGE, ERROR_PATTERNS } from '@/lib/constants/errorMessages.constants';

/**
 * Get user-friendly error message by key or error object
 * @param keyOrError - Error message manual key or Error object
 * @param args - Optional arguments for message interpolation
 * @returns User-friendly error message
 */
export function getUserFriendlyErrorMessage(keyOrError: string | Error): string {
    const errorMessage =
        typeof keyOrError === 'string'
            ? keyOrError.toLowerCase()
            : keyOrError.message.toLowerCase();

    for (const [pattern, message] of Object.entries(ERROR_PATTERNS)) {
        if (errorMessage.includes(pattern.toLowerCase())) {
            return message;
        }
    }

    return DEFAULT_ERROR_MESSAGE;
}
