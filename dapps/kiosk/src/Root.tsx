// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { getFullnodeUrl } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Toaster } from 'react-hot-toast';
import { Outlet } from 'react-router-dom';

import { Header } from './components/Base/Header';
import { KisokClientProvider } from './context/KioskClientContext';

const queryClient = new QueryClient();

export default function Root() {
    return (
        <QueryClientProvider client={queryClient}>
            <IotaClientProvider
                defaultNetwork="testnet"
                networks={{ testnet: { url: getFullnodeUrl('testnet') } }}
            >
                <WalletProvider>
                    <KisokClientProvider>
                        <Header />
                        <div className="min-h-[80vh]">
                            <Outlet />
                        </div>
                        <div className="mt-6 border-t border-primary text-center py-6">
                            Copyright © Mysten Labs, Inc.
                            Modifications Copyright © 2024 IOTA Stiftung
                        </div>
                        <Toaster position="bottom-center" />
                    </KisokClientProvider>
                </WalletProvider>
            </IotaClientProvider>
        </QueryClientProvider>
    );
}
