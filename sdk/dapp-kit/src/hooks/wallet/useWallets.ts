// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useWalletStore } from './useWalletStore.js';

/**
 * Retrieves a list of registered wallets available to the dApp sorted by preference.
 */
export function useWallets() {
    return useWalletStore((state) => state.wallets);
}
