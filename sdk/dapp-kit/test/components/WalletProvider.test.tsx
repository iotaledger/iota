// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { act, renderHook, waitFor } from '@testing-library/react';

import {
    useAccounts,
    useConnectWallet,
    useCurrentAccount,
    useCurrentWallet,
    useDisconnectWallet,
    useWallets,
} from '../../src/index.js';
import { DEFAULT_STORAGE_KEY } from '../../src/constants/walletDefaults.js';
import { createMockAccount } from '../mocks/mockAccount.js';
import { iotaFeatures, superCoolFeature } from '../mocks/mockFeatures.js';
import { createWalletProviderContextWrapper, registerMockWallet } from '../test-utils.js';

describe('WalletProvider', () => {
    test('the correct wallet and account information is returned on initial render', () => {
        const wrapper = createWalletProviderContextWrapper();
        const { result } = renderHook(
            () => ({
                wallets: useWallets(),
                currentWallet: useCurrentWallet(),
                currentAccount: useCurrentAccount(),
            }),
            { wrapper },
        );

        expect(result.current.currentWallet.isConnected).toBeFalsy();
        expect(result.current.currentAccount).toBeFalsy();
        expect(result.current.wallets).toHaveLength(0);
    });

    test('the list of wallets is ordered correctly by preference', () => {
        const { unregister: unregister1 } = registerMockWallet({
            walletName: 'Mock Wallet 1',
            features: iotaFeatures,
        });
        const { unregister: unregister2 } = registerMockWallet({
            walletName: 'Mock Wallet 2',
            features: iotaFeatures,
        });
        const { unregister: unregister3 } = registerMockWallet({
            walletName: 'Mock Wallet 3',
            features: iotaFeatures,
        });

        const wrapper = createWalletProviderContextWrapper({
            preferredWallets: ['Mock Wallet 2', 'Mock Wallet 1'],
        });
        const { result } = renderHook(() => useWallets(), { wrapper });
        const walletNames = result.current.map((wallet) => wallet.name);

        expect(walletNames).toStrictEqual(['Mock Wallet 2', 'Mock Wallet 1', 'Mock Wallet 3']);

        act(() => {
            unregister1();
            unregister2();
            unregister3();
        });
    });

    test('the unsafe burner wallet is registered when enableUnsafeBurner is set', async () => {
        const wrapper = createWalletProviderContextWrapper({
            enableUnsafeBurner: true,
        });
        const { result } = renderHook(() => useWallets(), { wrapper });
        const walletNames = result.current.map((wallet) => wallet.name);

        expect(walletNames).toStrictEqual(['Unsafe Burner Wallet']);
    });

    test('unregistered wallets are removed from the list of wallets', async () => {
        const { unregister: unregister1 } = registerMockWallet({
            walletName: 'Mock Wallet 1',
            features: iotaFeatures,
        });
        const { unregister: unregister2 } = registerMockWallet({
            walletName: 'Mock Wallet 2',
            features: iotaFeatures,
        });
        const { unregister: unregister3 } = registerMockWallet({
            walletName: 'Mock Wallet 3',
            features: iotaFeatures,
        });

        const wrapper = createWalletProviderContextWrapper();
        const { result } = renderHook(() => useWallets(), { wrapper });

        act(() => unregister2());

        const walletNames = result.current.map((wallet) => wallet.name);
        expect(walletNames).toStrictEqual(['Mock Wallet 1', 'Mock Wallet 3']);

        act(() => {
            unregister1();
            unregister3();
        });
    });

    test('the list of wallets is correctly filtered by required features', () => {
        const { unregister: unregister1 } = registerMockWallet({
            walletName: 'Mock Wallet 1',
            features: superCoolFeature,
        });
        const { unregister: unregister2 } = registerMockWallet({ walletName: 'Mock Wallet 2' });

        const wrapper = createWalletProviderContextWrapper({
            walletFilter: (wallet) => !!wallet.features['my-dapp:super-cool-feature'],
        });
        const { result } = renderHook(() => useWallets(), { wrapper });
        const walletNames = result.current.map((wallet) => wallet.name);

        expect(walletNames).toStrictEqual(['Mock Wallet 1']);

        act(() => {
            unregister1();
            unregister2();
        });
    });

    test('accounts are properly updated when changed from a wallet', async () => {
        const { unregister, mockWallet } = registerMockWallet({
            walletName: 'Mock Wallet 1',
            accounts: [createMockAccount(), createMockAccount(), createMockAccount()],
        });

        const wrapper = createWalletProviderContextWrapper();
        const { result } = renderHook(
            () => ({
                connectWallet: useConnectWallet(),
                currentAccount: useCurrentAccount(),
                accounts: useAccounts(),
            }),
            { wrapper },
        );

        result.current.connectWallet.mutate({ wallet: mockWallet });

        await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));

        // Simulate deleting the account we're currently connected to.
        act(() => {
            mockWallet.deleteFirstAccount();
        });

        expect(result.current.currentAccount).toBeTruthy();
        await waitFor(() => {
            expect(result.current.currentAccount!.address).toBe(result.current.accounts[0].address);
        });

        expect(result.current.accounts).toHaveLength(2);

        act(() => unregister());
    });

    describe('wallet auto-connection', () => {
        test('auto-connecting to a wallet works successfully', async () => {
            const { unregister, mockWallet } = registerMockWallet({
                walletName: 'Mock Wallet 1',
                accounts: [createMockAccount(), createMockAccount()],
                features: iotaFeatures,
            });

            const wrapper = createWalletProviderContextWrapper({
                autoConnect: true,
            });
            const { result, unmount } = renderHook(() => useConnectWallet(), { wrapper });

            // Manually connect a wallet so we have a wallet to auto-connect to later.
            result.current.mutate({
                wallet: mockWallet,
                accountAddress: mockWallet.accounts[1].address,
            });

            await waitFor(() => expect(result.current.isSuccess).toBe(true));

            // Now unmount our component tree to simulate someone leaving the page.
            unmount();

            // Render our component tree again and auto-connect to our previously connected wallet account.
            const { result: updatedResult } = renderHook(
                () => ({
                    currentWallet: useCurrentWallet(),
                    currentAccount: useCurrentAccount(),
                }),
                { wrapper },
            );

            await waitFor(() => expect(updatedResult.current.currentWallet.isConnected).toBe(true));
            expect(updatedResult.current.currentWallet.currentWallet!.name).toStrictEqual(
                'Mock Wallet 1',
            );

            expect(updatedResult.current.currentAccount).toBeTruthy();
            expect(updatedResult.current.currentAccount!.address).toStrictEqual(
                mockWallet.accounts[1].address,
            );

            act(() => unregister());
        });

        test('auto-connecting to an id-based wallet works', async () => {
            const wallet1 = registerMockWallet({
                id: '1',
                walletName: 'Mock Wallet',
                features: iotaFeatures,
            });

            const wallet2 = registerMockWallet({
                id: '2',
                walletName: 'Mock Wallet',
                features: iotaFeatures,
            });

            const wrapper = createWalletProviderContextWrapper({
                autoConnect: true,
            });
            const { result, unmount } = renderHook(() => useConnectWallet(), { wrapper });

            result.current.mutate({ wallet: wallet1.mockWallet });

            await waitFor(() => expect(result.current.isSuccess).toBe(true));

            // Now unmount our component tree to simulate someone leaving the page.
            unmount();

            // Render our component tree again and auto-connect to our previously connected wallet account.
            const { result: updatedResult } = renderHook(
                () => ({
                    currentWallet: useCurrentWallet(),
                    currentAccount: useCurrentAccount(),
                }),
                { wrapper },
            );

            await waitFor(() => expect(updatedResult.current.currentWallet.isConnected).toBe(true));
            expect(updatedResult.current.currentWallet.currentWallet!.id).toStrictEqual('1');
            expect(updatedResult.current.currentAccount).toBeTruthy();

            act(() => {
                wallet1.unregister();
                wallet2.unregister();
            });
        });

        test('wallet connection info is removed upon disconnection', async () => {
            const { unregister, mockWallet } = registerMockWallet({
                walletName: 'Mock Wallet 1',
            });
            const wrapper = createWalletProviderContextWrapper({
                autoConnect: true,
            });

            const { result, unmount } = renderHook(
                () => ({
                    connectWallet: useConnectWallet(),
                    disconnectWallet: useDisconnectWallet(),
                    currentAccount: useCurrentAccount(),
                }),
                { wrapper },
            );

            result.current.connectWallet.mutate({
                wallet: mockWallet,
            });
            await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));

            // By disconnecting, we should remove any wallet connection info that we have stored.
            result.current.disconnectWallet.mutate();
            await waitFor(() => expect(result.current.disconnectWallet.isSuccess).toBe(true));

            // Now unmount our component tree to simulate someone leaving the page.
            unmount();

            // Render our component tree again and assert that we weren't able to auto-connect.
            const { result: updatedResult } = renderHook(() => useCurrentWallet(), { wrapper });
            expect(updatedResult.current.isConnected).toBeFalsy();

            act(() => unregister());
        });
    });

    describe('cross-tab synchronization', () => {
        // ---------------------------------------------------------------------------
        // Helpers
        // ---------------------------------------------------------------------------

        function dispatchStorageEvent(key: string, newValue: string | null) {
            window.dispatchEvent(
                new StorageEvent('storage', {
                    key,
                    newValue,
                    storageArea: localStorage,
                }),
            );
        }

        function serializePersistedState(walletName: string | null, accountAddress: string | null) {
            return JSON.stringify({
                state: {
                    lastConnectedWalletName: walletName,
                    lastConnectedAccountAddress: accountAddress,
                },
            });
        }

        // ---------------------------------------------------------------------------
        // Scenario: wallet not connected
        // The tab has never connected. Storage events should be safely ignored when
        // syncTabs is off, and should trigger a connect when syncTabs is on.
        // ---------------------------------------------------------------------------

        describe('wallet not connected', () => {
            test('ignores storage events when syncTabs is false (default)', async () => {
                const account = createMockAccount();
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    accounts: [account],
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: false });
                const { result } = renderHook(
                    () => ({
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                expect(result.current.currentWallet.isDisconnected).toBe(true);

                // Another tab connects — this tab should stay disconnected
                act(() => {
                    dispatchStorageEvent(
                        DEFAULT_STORAGE_KEY,
                        serializePersistedState(mockWallet.name, account.address),
                    );
                });

                expect(result.current.currentWallet.isDisconnected).toBe(true);
                expect(result.current.currentAccount).toBeNull();

                act(() => unregister());
            });

            test('ignores storage events for unrelated localStorage keys', async () => {
                const account = createMockAccount();
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    accounts: [account],
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: true });
                const { result } = renderHook(
                    () => ({
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                expect(result.current.currentWallet.isDisconnected).toBe(true);

                // Event for an unrelated key — must be ignored
                act(() => {
                    dispatchStorageEvent(
                        'some-other-key',
                        serializePersistedState(mockWallet.name, account.address),
                    );
                });

                expect(result.current.currentWallet.isDisconnected).toBe(true);

                act(() => unregister());
            });
        });

        // ---------------------------------------------------------------------------
        // Scenario: wallet connected, tab opened for the first time
        // Another tab is already connected. When this tab opens it auto-connects via
        // the normal autoConnect path (not via storage events). The storage event path
        // is only relevant for tabs that are already open and idle.
        // We verify that a storage event while disconnected triggers a connection.
        // ---------------------------------------------------------------------------

        describe('wallet connected, opened for the first time', () => {
            test('connects when a storage event arrives on a freshly opened disconnected tab', async () => {
                const account = createMockAccount();
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    accounts: [account],
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: true });
                const { result } = renderHook(
                    () => ({
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                // Tab starts with no wallet connected
                expect(result.current.currentWallet.isDisconnected).toBe(true);

                // Simulate another (already-open) tab writing the connect event to storage
                act(() => {
                    dispatchStorageEvent(
                        DEFAULT_STORAGE_KEY,
                        serializePersistedState(mockWallet.name, account.address),
                    );
                });

                await waitFor(() => expect(result.current.currentWallet.isConnected).toBe(true));
                expect(result.current.currentAccount?.address).toBe(account.address);

                act(() => unregister());
            });

            test('stays disconnected when storage event arrives but syncTabs is false', async () => {
                const account = createMockAccount();
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    accounts: [account],
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: false });
                const { result } = renderHook(
                    () => ({
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                expect(result.current.currentWallet.isDisconnected).toBe(true);

                act(() => {
                    dispatchStorageEvent(
                        DEFAULT_STORAGE_KEY,
                        serializePersistedState(mockWallet.name, account.address),
                    );
                });

                // syncTabs is off — tab must not react
                expect(result.current.currentWallet.isDisconnected).toBe(true);

                act(() => unregister());
            });
        });

        // ---------------------------------------------------------------------------
        // Scenario: wallet connected, 2+ tabs open, account switched in another tab
        // ---------------------------------------------------------------------------

        describe('wallet connected, 2+ tabs, account switched in another tab', () => {
            test('switches to the new account without triggering a full reconnect', async () => {
                const account1 = createMockAccount();
                const account2 = createMockAccount();
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    accounts: [account1, account2],
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: true });
                const { result } = renderHook(
                    () => ({
                        connectWallet: useConnectWallet(),
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                // Connect to account1 first
                result.current.connectWallet.mutate({
                    wallet: mockWallet,
                    accountAddress: account1.address,
                });
                await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));
                expect(result.current.currentAccount?.address).toBe(account1.address);

                // Another tab switches to account2
                act(() => {
                    dispatchStorageEvent(
                        DEFAULT_STORAGE_KEY,
                        serializePersistedState(mockWallet.name, account2.address),
                    );
                });

                await waitFor(() =>
                    expect(result.current.currentAccount?.address).toBe(account2.address),
                );
                // Wallet must still be connected — only the active account changed
                expect(result.current.currentWallet.isConnected).toBe(true);

                act(() => unregister());
            });

            test('does not switch account when syncTabs is false', async () => {
                const account1 = createMockAccount();
                const account2 = createMockAccount();
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    accounts: [account1, account2],
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: false });
                const { result } = renderHook(
                    () => ({
                        connectWallet: useConnectWallet(),
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                result.current.connectWallet.mutate({
                    wallet: mockWallet,
                    accountAddress: account1.address,
                });
                await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));
                expect(result.current.currentAccount?.address).toBe(account1.address);

                // Another tab switches to account2 — should be ignored
                act(() => {
                    dispatchStorageEvent(
                        DEFAULT_STORAGE_KEY,
                        serializePersistedState(mockWallet.name, account2.address),
                    );
                });

                // Active account must not change
                expect(result.current.currentAccount?.address).toBe(account1.address);

                act(() => unregister());
            });

            test('does not trigger an infinite loop when account is switched', async () => {
                const account1 = createMockAccount();
                const account2 = createMockAccount();
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    accounts: [account1, account2],
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: true });
                const { result } = renderHook(
                    () => ({
                        connectWallet: useConnectWallet(),
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                result.current.connectWallet.mutate({
                    wallet: mockWallet,
                    accountAddress: account1.address,
                });
                await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));

                const connectSpy = mockWallet.mocks.connect;
                const callsBefore = connectSpy.mock.calls.length;

                // Simulate the storage event that another tab emits on account switch
                act(() => {
                    dispatchStorageEvent(
                        DEFAULT_STORAGE_KEY,
                        serializePersistedState(mockWallet.name, account2.address),
                    );
                });

                await waitFor(() =>
                    expect(result.current.currentAccount?.address).toBe(account2.address),
                );

                // The wallet connect() must NOT have been called again —
                // setAccountSwitched handles same-wallet account changes without a
                // round-trip to the extension, which would write to localStorage and
                // produce another storage event, causing an infinite loop.
                expect(connectSpy.mock.calls.length).toBe(callsBefore);

                act(() => unregister());
            });
        });

        // ---------------------------------------------------------------------------
        // Scenario: wallet connected, 2+ tabs open, disconnected in another tab
        // ---------------------------------------------------------------------------

        describe('wallet connected, 2+ tabs, disconnected in another tab', () => {
            test('disconnects when another tab clears the storage key', async () => {
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: true });
                const { result } = renderHook(
                    () => ({
                        connectWallet: useConnectWallet(),
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                result.current.connectWallet.mutate({ wallet: mockWallet });
                await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));
                expect(result.current.currentWallet.isConnected).toBe(true);

                // Another tab disconnects — newValue becomes null
                act(() => {
                    dispatchStorageEvent(DEFAULT_STORAGE_KEY, null);
                });

                await waitFor(() => expect(result.current.currentWallet.isDisconnected).toBe(true));
                expect(result.current.currentAccount).toBeNull();

                act(() => unregister());
            });

            test('disconnects when another tab writes empty wallet fields', async () => {
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: true });
                const { result } = renderHook(
                    () => ({
                        connectWallet: useConnectWallet(),
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                result.current.connectWallet.mutate({ wallet: mockWallet });
                await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));
                expect(result.current.currentWallet.isConnected).toBe(true);

                // Disconnect represented as null fields inside the persisted state object
                act(() => {
                    dispatchStorageEvent(DEFAULT_STORAGE_KEY, serializePersistedState(null, null));
                });

                await waitFor(() => expect(result.current.currentWallet.isDisconnected).toBe(true));
                expect(result.current.currentAccount).toBeNull();

                act(() => unregister());
            });

            test('stays connected when syncTabs is false and another tab disconnects', async () => {
                const { unregister, mockWallet } = registerMockWallet({
                    walletName: 'Mock Wallet 1',
                    features: iotaFeatures,
                });

                const wrapper = createWalletProviderContextWrapper({ syncTabs: false });
                const { result } = renderHook(
                    () => ({
                        connectWallet: useConnectWallet(),
                        currentWallet: useCurrentWallet(),
                        currentAccount: useCurrentAccount(),
                    }),
                    { wrapper },
                );

                result.current.connectWallet.mutate({ wallet: mockWallet });
                await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));
                expect(result.current.currentWallet.isConnected).toBe(true);

                // syncTabs is off — disconnect from another tab must be ignored
                act(() => {
                    dispatchStorageEvent(DEFAULT_STORAGE_KEY, null);
                });

                expect(result.current.currentWallet.isConnected).toBe(true);
                expect(result.current.currentAccount).toBeTruthy();

                act(() => unregister());
            });
        });
    });
});
