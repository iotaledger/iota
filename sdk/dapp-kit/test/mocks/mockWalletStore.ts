// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import create from 'zustand';
import type { StoreApi } from 'zustand';
// import { StoreState } from './walletStore';
import type { StoreState } from '../../src/walletStore.js';

export const createMockWalletStore = (): StoreApi<StoreState> => {
    return create<StoreState>((set) => ({
        autoConnectEnabled: false,
        wallets: [],
        accounts: [],
        currentWallet: null,
        currentAccount: null,
        lastConnectedAccountAddress: null,
        lastConnectedWalletName: null,
        connectionStatus: 'disconnected',
        setAccountSwitched: (selectedAccount) => set({ currentAccount: selectedAccount }),
        setConnectionStatus: (connectionStatus) => set({ connectionStatus }),
        setWalletConnected: (wallet, connectedAccounts, selectedAccount) =>
            set({
                accounts: connectedAccounts,
                currentWallet: wallet,
                currentAccount: selectedAccount,
                lastConnectedWalletName: wallet.name,
                lastConnectedAccountAddress: selectedAccount?.address ?? null,
                connectionStatus: 'connected',
            }),
        updateWalletAccounts: (accounts) => set({ accounts }),
        setWalletDisconnected: () =>
            set({
                accounts: [],
                currentWallet: null,
                currentAccount: null,
                lastConnectedWalletName: null,
                lastConnectedAccountAddress: null,
                connectionStatus: 'disconnected',
            }),
        setWalletRegistered: (updatedWallets) => set({ wallets: updatedWallets }),
        setWalletUnregistered: (updatedWallets, unregisteredWallet) => {
            set((state) => ({
                wallets: updatedWallets,
                ...(state.currentWallet === unregisteredWallet
                    ? {
                          accounts: [],
                          currentWallet: null,
                          currentAccount: null,
                          lastConnectedWalletName: null,
                          lastConnectedAccountAddress: null,
                          connectionStatus: 'disconnected',
                      }
                    : {}),
            }));
        },
    }));
};
