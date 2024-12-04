// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { PopupProvider, Toaster } from '@/components';
import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { IotaClientProvider, lightTheme, darkTheme, WalletProvider } from '@iota/dapp-kit';
import { getAllNetworks, getDefaultNetwork } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState } from 'react';
import { growthbook } from '@/lib/utils';
import { Popup } from '@/components/Popup';
import { ThemeProvider } from '@iota/core';

growthbook.init();

export function AppProviders({ children }: React.PropsWithChildren) {
    const [queryClient] = useState(() => new QueryClient());
    const allNetworks = getAllNetworks();
    const defaultNetwork = getDefaultNetwork();

    return (
        <GrowthBookProvider growthbook={growthbook}>
            <QueryClientProvider client={queryClient}>
                <IotaClientProvider networks={allNetworks} defaultNetwork={defaultNetwork}>
                    <WalletProvider
                        autoConnect={true}
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
                        <ThemeProvider appId="iota-dashboard">
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
    );
}
