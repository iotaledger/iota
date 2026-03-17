// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createCopyToClipboardHooks, toast } from '@iota/core';
import { ampli } from '_src/shared/analytics/ampli';

/**
 * Wallet-specific copy-to-clipboard hooks with integrated toast and analytics.
 * Created using dependency injection to decouple from specific implementations.
 */
export const { useCopyToClipboard, useCopySuccessCallback } = createCopyToClipboardHooks({
    toast: (message) => toast(message),
    trackCopy: (type) => ampli.elementCopied({ type }),
});
