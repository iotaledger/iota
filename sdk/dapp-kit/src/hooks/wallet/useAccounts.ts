// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletAccount } from '@iota/wallet-standard';

import { useWalletStore } from './useWalletStore.js';

/**
 * Retrieves a list of connected accounts authorized by the dApp.
 */
export function useAccounts(): readonly WalletAccount[] {
    return useWalletStore((state) => state.accounts);
}
