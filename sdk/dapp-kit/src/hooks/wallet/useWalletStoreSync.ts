// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useContext, useSyncExternalStore } from 'react';

import { WalletContext } from '../../contexts/walletContext.js';
import type { StoreState } from '../../walletStore.js';

/**
 * Predefined approved selectors for secure, controlled access to store values.
 * The selectors control what data is returned and in what format.
 */
const APPROVED_SELECTORS = {
    lastConnectedWalletName: (state: StoreState): string | null => state.lastConnectedWalletName,
    connectionStatus: (state: StoreState) => state.connectionStatus,
};

type ApprovedSelectorKey = keyof typeof APPROVED_SELECTORS;

// SSR-safe defaults - the selectors control the return types
const SSR_DEFAULTS: Record<ApprovedSelectorKey, unknown> = {
    lastConnectedWalletName: null,
    connectionStatus: 'disconnected' as const,
};

// Function overloads for type safety
export function useWalletStoreSync(selectorKey: 'lastConnectedWalletName'): string | null;
export function useWalletStoreSync(
    selectorKey: 'connectionStatus',
): 'disconnected' | 'connecting' | 'connected';

// Implementation
export function useWalletStoreSync(selectorKey: ApprovedSelectorKey) {
    const store = useContext(WalletContext);
    const selector = APPROVED_SELECTORS[selectorKey];
    const ssrDefault = SSR_DEFAULTS[selectorKey];

    return useSyncExternalStore(
        (callback) => {
            if (!store) return () => {};
            return store.subscribe(callback);
        },
        () => (store ? selector(store.getState()) : ssrDefault),
        () => ssrDefault,
    );
}
