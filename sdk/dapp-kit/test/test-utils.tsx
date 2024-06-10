// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getFullnodeUrl, IotaClient } from '@iota/iota.js/client';
import type { IdentifierRecord, ReadonlyWalletAccount } from '@iota/wallet-standard';
import { getWallets } from '@iota/wallet-standard';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ComponentProps } from 'react';

import { WalletProvider } from '../src/components/WalletProvider.js';
// import { WalletContext } from '../src/contexts/walletContext.js';
import { IotaClientProvider } from '../src/index.js';
import { createMockAccount } from './mocks/mockAccount.js';
import { MockWallet } from './mocks/mockWallet.js';
// import { createMockWalletStore } from './mocks/mockWalletStore.js';

export function createIotaClientContextWrapper(client: IotaClient) {
    return function IotaClientContextWrapper({ children }: { children: React.ReactNode }) {
        return <IotaClientProvider networks={{ test: client }}>{children}</IotaClientProvider>;
    };
}

export function createWalletProviderContextWrapper(
    providerProps: Omit<ComponentProps<typeof WalletProvider>, 'children'> = {},
    iotaClient: IotaClient = new IotaClient({ url: getFullnodeUrl('localnet') }),
) {
    const queryClient = new QueryClient();
    // const mockStore = createMockWalletStore();
    return function WalletProviderContextWrapper({ children }: { children: React.ReactNode }) {
        return (
            <IotaClientProvider networks={{ test: iotaClient }}>
                <QueryClientProvider client={queryClient}>
                    {/*<WalletContext.Provider value={mockStore}>*/}
                    <WalletProvider {...providerProps}>{children}</WalletProvider>;
                    {/*</WalletContext.Provider>*/}
                </QueryClientProvider>
            </IotaClientProvider>
        );
    };
}

export function registerMockWallet({
    id,
    walletName,
    accounts = [createMockAccount()],
    features = {},
}: {
    id?: string | null;
    walletName: string;
    accounts?: ReadonlyWalletAccount[];
    features?: IdentifierRecord<unknown>;
}) {
    const walletsApi = getWallets();
    const mockWallet = new MockWallet(id ?? crypto.randomUUID(), walletName, accounts, features);

    return {
        unregister: walletsApi.register(mockWallet),
        mockWallet,
    };
}

// export function registerWalletStore() {
//     const accounts = [createMockAccount()];
//     const walletsApi = getWallets();
//     const mockWallet = new MockWallet(crypto.randomUUID(), 'Mock Wallet 1', accounts, {});
// }
