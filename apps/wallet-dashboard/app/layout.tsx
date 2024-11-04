// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import './globals.css';
import '@iota/dapp-kit/dist/index.css';
import { Inter } from 'next/font/google';
import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { IotaClientProvider, lightTheme, darkTheme, WalletProvider } from '@iota/dapp-kit';
import { getAllNetworks, getDefaultNetwork } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import '@iota/dapp-kit/dist/index.css';
import { Popup, PopupProvider } from '@/components/Popup';
import { growthbook } from '@/lib/utils';
import { Toaster } from 'react-hot-toast';
import { ThemeProvider } from '@/contexts';

const inter = Inter({ subsets: ['latin'] });
growthbook.init();

export default function RootLayout({
    children,
}: Readonly<{
    children: React.ReactNode;
}>) {
    const [queryClient] = React.useState(() => new QueryClient());
    const bodyRef = React.useRef<HTMLBodyElement>(null);

    const allNetworks = getAllNetworks();
    const defaultNetwork = getDefaultNetwork();

    return (
        <html lang="en">
            <body className={inter.className} ref={bodyRef}>
                <GrowthBookProvider growthbook={growthbook}>
                    <QueryClientProvider client={queryClient}>
                        <IotaClientProvider networks={allNetworks} defaultNetwork={defaultNetwork}>
                            <WalletProvider
                                theme={[
                                    {
                                        variables: lightTheme,
                                    },
                                    {
                                        selector: '.dark',
                                        variables: darkTheme,
                                    },
                                ]}
                            >
                                <ThemeProvider>
                                    <PopupProvider>
                                        {children}
                                        <Toaster />
                                        <Popup />
                                    </PopupProvider>
                                </ThemeProvider>
                            </WalletProvider>
                        </IotaClientProvider>
                    </QueryClientProvider>
                </GrowthBookProvider>
            </body>
        </html>
    );
}
