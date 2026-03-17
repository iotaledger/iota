// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createCopyToClipboardHooks, toast } from '@iota/core';

/**
 * Explorer-specific copy-to-clipboard hooks with toast notifications.
 * No analytics tracking for explorer.
 */
export const { useCopyToClipboard, useCopySuccessCallback } = createCopyToClipboardHooks({
    toast: (message) => toast.success(message),
});
