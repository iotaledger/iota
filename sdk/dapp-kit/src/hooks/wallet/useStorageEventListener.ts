// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useContext, useEffect, useRef } from 'react';

import { WalletContext } from '../../contexts/walletContext.js';
import { getWalletUniqueIdentifier } from '../../utils/walletUtils.js';
import { useConnectWallet } from './useConnectWallet.js';
import { useWallets } from './useWallets.js';

/**
 * Internal hook that listens to localStorage `storage` events from other browser tabs
 * and synchronizes wallet connection state reactively. Only active when `enabled` is true.
 */
export function useStorageEventListener(enabled: boolean, storageKey: string) {
    const store = useContext(WalletContext);
    const { mutateAsync: connectWallet } = useConnectWallet();
    const wallets = useWallets();

    // Refs keep the handler stable (registered once) while always seeing the latest values.
    const connectWalletRef = useRef(connectWallet);
    const walletsRef = useRef(wallets);
    const processingRef = useRef(false);

    useEffect(() => {
        connectWalletRef.current = connectWallet;
    }, [connectWallet]);

    useEffect(() => {
        walletsRef.current = wallets;
    }, [wallets]);

    useEffect(() => {
        if (!enabled || typeof window === 'undefined') return;

        const handler = async (event: StorageEvent) => {
            if (event.key !== storageKey) return;

            // Drop events that arrive while we are already processing one.
            if (processingRef.current) return;

            if (!event.newValue) {
                store?.getState().setWalletDisconnected();
                return;
            }

            let lastConnectedWalletName: string | null = null;
            let lastConnectedAccountAddress: string | null = null;
            try {
                const parsed = JSON.parse(event.newValue);
                lastConnectedWalletName = parsed?.state?.lastConnectedWalletName ?? null;
                lastConnectedAccountAddress = parsed?.state?.lastConnectedAccountAddress ?? null;
            } catch {
                return;
            }

            if (!lastConnectedWalletName || !lastConnectedAccountAddress) {
                store?.getState().setWalletDisconnected();
                return;
            }

            // Read current state imperatively — never from a closure — so we always
            // compare against the latest committed value.
            const state = store?.getState();
            const currentAccountAddress = state?.currentAccount?.address;
            if (currentAccountAddress === lastConnectedAccountAddress) return;

            const currentWalletName = state?.currentWallet
                ? getWalletUniqueIdentifier(state.currentWallet)
                : null;
            const isSameWallet = currentWalletName === lastConnectedWalletName;

            processingRef.current = true;
            try {
                if (isSameWallet && state?.currentWallet) {
                    // Wallet is already connected — just switch the active account locally.
                    // This avoids a full reconnect round-trip and does NOT write to localStorage,
                    // breaking the ping-pong loop.
                    const accountToSelect = state.currentWallet.accounts.find(
                        (a) => a.address === lastConnectedAccountAddress,
                    );
                    if (accountToSelect) {
                        state.setAccountSwitched(accountToSelect);
                    }
                } else {
                    // Different wallet or not yet connected — full connect needed.
                    const wallet = walletsRef.current.find(
                        (w) => getWalletUniqueIdentifier(w) === lastConnectedWalletName,
                    );
                    if (wallet) {
                        await connectWalletRef.current({
                            wallet,
                            accountAddress: lastConnectedAccountAddress,
                            silent: true,
                        });
                    }
                }
            } finally {
                processingRef.current = false;
            }
        };

        window.addEventListener('storage', handler);
        return () => window.removeEventListener('storage', handler);

        // Intentionally excludes connectWallet and wallets — those are kept
        // fresh via refs so the handler is registered exactly once per storageKey change.
    }, [enabled, storageKey, store]);
}
