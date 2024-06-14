// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Inter } from 'next/font/google';

import './globals.css';

import { IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { getAllNetworks, getDefaultNetwork } from '@iota/iota.js/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';

import '@iota/dapp-kit/dist/index.css';
import { Popup, PopupProvider } from '@/components/Popup';
import { KioskClientProvider } from '@iota/core';

const inter = Inter({ subsets: ['latin'] });

export default function RootLayout({
    children,
}: Readonly<{
    children: React.ReactNode;
}>) {
    const [queryClient] = React.useState(() => new QueryClient());

    const allNetworks = getAllNetworks();
    const defaultNetwork = getDefaultNetwork();

    return (
        <html lang="en">
            <body className={inter.className}>
                <PopupProvider>
                    <QueryClientProvider client={queryClient}>
                        <IotaClientProvider networks={allNetworks} defaultNetwork={defaultNetwork}>
                            <KioskClientProvider>
                                <WalletProvider>{children}</WalletProvider>
                            </KioskClientProvider>
                            <Popup />
                        </IotaClientProvider>
                    </QueryClientProvider>
                </PopupProvider>
            </body>
        </html>
    );
}
