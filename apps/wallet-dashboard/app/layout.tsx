// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Inter } from 'next/font/google';

import './globals.css';

import { SuiClientProvider, WalletProvider } from '@mysten/dapp-kit';
import { getAllNetworks } from '@mysten/sui.js/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';

import '@mysten/dapp-kit/dist/index.css';

const inter = Inter({ subsets: ['latin'] });

export default function RootLayout({
    children,
}: Readonly<{
    children: React.ReactNode;
}>) {
    const [queryClient] = React.useState(() => new QueryClient());

    const allNetworks = getAllNetworks();

    return (
        <html lang="en">
            <body className={inter.className}>
                <QueryClientProvider client={queryClient}>
                    <SuiClientProvider networks={allNetworks} defaultNetwork="testnet">
                        <WalletProvider>{children}</WalletProvider>
                    </SuiClientProvider>
                </QueryClientProvider>
            </body>
        </html>
    );
}
