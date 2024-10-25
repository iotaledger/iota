// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Inter } from 'next/font/google';

import './globals.css';
import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { getAllNetworks, getDefaultNetwork } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';

import '@iota/dapp-kit/dist/index.css';
import { Popup, PopupProvider } from '@/components/Popup';
import { growthbook } from '@/lib/utils';

const inter = Inter({ subsets: ['latin'] });

export default function RootLayout({
    children,
}: Readonly<{
    children: React.ReactNode;
}>) {
    const [queryClient] = React.useState(() => new QueryClient());

    const allNetworks = getAllNetworks();
    const defaultNetwork = getDefaultNetwork();

    growthbook.init();

    return (
        <html lang="en">
            <body className={inter.className}>
                <GrowthBookProvider growthbook={growthbook}>
                    <QueryClientProvider client={queryClient}>
                        <IotaClientProvider networks={allNetworks} defaultNetwork={defaultNetwork}>
                            <WalletProvider>
                                <PopupProvider>
                                    {children}
                                    <Popup />
                                </PopupProvider>
                            </WalletProvider>
                        </IotaClientProvider>
                    </QueryClientProvider>
                </GrowthBookProvider>
            </body>
        </html>
    );
}
