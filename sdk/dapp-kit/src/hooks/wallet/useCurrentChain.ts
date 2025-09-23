// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ChainType } from '@iota/iota-sdk/client';
import { useWalletStore } from './useWalletStore.js';

/**
 * Retrieves the chain the dapp is configured with, if any.
 */
export function useCurrentChain(): ChainType | undefined {
    return useWalletStore((state) => state.chain);
}
