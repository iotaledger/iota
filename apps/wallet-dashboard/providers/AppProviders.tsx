// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { IotaClientProvider, lightTheme, darkTheme, WalletProvider } from '@iota/dapp-kit';
import { getAllNetworks, getDefaultNetwork } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { useState } from 'react';
import {
    KioskClientProvider,
    StardustIndexerClientProvider,
    useLocalStorage,
    Toaster,
    ClipboardPasteSafetyWrapper,
} from '@iota/core';
import { growthbook } from '@/lib/utils';
import { ThemeProvider } from '@iota/core';
import { createIotaClient } from '@/lib/utils/defaultRpcClient';

growthbook.init();

export function AppProviders({ children }: React.PropsWithChildren) {
    const [queryClient] = useState(() => new QueryClient());
    const allNetworks = getAllNetworks();
    const defaultNetwork = getDefaultNetwork();
    const [persistedNetwork] = useLocalStorage<string>('network_iota-dashboard', defaultNetwork);

    function handleNetworkChange() {
        queryClient.resetQueries();
        queryClient.clear();
    }
    return (
        <GrowthBookProvider growthbook={growthbook}>
            <QueryClientProvider client={queryClient}>
                <IotaClientProvider
                    networks={allNetworks}
                    createClient={createIotaClient}
                    defaultNetwork={persistedNetwork}
                    onNetworkChange={handleNetworkChange}
                >
                    <StardustIndexerClientProvider>
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
                                <ClipboardPasteSafetyWrapper>
                                    <ThemeProvider appId="iota-dashboard">
                                        {children}
                                        <Toaster />
                                    </ThemeProvider>
                                </ClipboardPasteSafetyWrapper>
                            </WalletProvider>
                        </KioskClientProvider>
                    </StardustIndexerClientProvider>
                </IotaClientProvider>
                <ReactQueryDevtools initialIsOpen={false} />
            </QueryClientProvider>
        </GrowthBookProvider>
    );
}
