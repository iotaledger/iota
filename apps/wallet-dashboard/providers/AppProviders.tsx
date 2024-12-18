// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Toaster } from '@/components';
import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { IotaClientProvider, lightTheme, darkTheme, WalletProvider } from '@iota/dapp-kit';
import { getAllNetworks, getDefaultNetwork } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState } from 'react';
import { KioskClientProvider } from '@iota/core';
import { growthbook } from '@/lib/utils';
import { ThemeProvider } from '@iota/core';

growthbook.init();

export function AppProviders({ children }: React.PropsWithChildren) {
    const [queryClient] = useState(() => new QueryClient());
    const allNetworks = getAllNetworks();
    const defaultNetwork = getDefaultNetwork();
    function handleNetworkChange() {
        queryClient.resetQueries();
        queryClient.clear();
    }
    return (
        <GrowthBookProvider growthbook={growthbook}>
            <QueryClientProvider client={queryClient}>
                <IotaClientProvider
                    networks={allNetworks}
                    defaultNetwork={defaultNetwork}
                    onNetworkChange={handleNetworkChange}
                >
                    <KioskClientProvider>
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
                                {children}
                                <Toaster />
                            </ThemeProvider>
                        </WalletProvider>
                    </KioskClientProvider>
                </IotaClientProvider>
            </QueryClientProvider>
        </GrowthBookProvider>
    );
}
