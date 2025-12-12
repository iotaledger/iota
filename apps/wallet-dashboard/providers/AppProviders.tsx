// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { IotaClientProvider, lightTheme, darkTheme, WalletProvider } from '@iota/dapp-kit';
import { getAllNetworks, getDefaultNetwork, getNetwork } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { useState } from 'react';
import { CookieManagerProvider } from '@boxfish-studio/react-cookie-manager';
import {
    KioskClientProvider,
    StardustIndexerClientProvider,
    useLocalStorage,
    Toaster,
    ClipboardPasteSafetyWrapper,
    IotaGraphQLClientProvider,
    IotaNamesClientProvider,
} from '@iota/core';
import { growthbook } from '@/lib/utils';
import { ThemeProvider } from '@iota/core';
import { createIotaClient } from '@/lib/utils/defaultRpcClient';
import { CookieDisclaimer } from '@/components/disclaimer/CookieDisclaimer';

growthbook.init();

export function AppProviders({ children }: React.PropsWithChildren) {
    const [queryClient] = useState(() => new QueryClient());
    const allNetworks = getAllNetworks();
    const defaultNetworkId = getDefaultNetwork();
    const [persistedNetworkId] = useLocalStorage<string>(
        'network_iota-dashboard',
        defaultNetworkId,
    );
    const persistedNetwork = getNetwork(persistedNetworkId);

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
                    defaultNetwork={persistedNetworkId}
                    onNetworkChange={handleNetworkChange}
                >
                    <StardustIndexerClientProvider>
                        <IotaGraphQLClientProvider>
                            <IotaNamesClientProvider>
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
                                        chain={persistedNetwork.chain}
                                    >
                                        <ClipboardPasteSafetyWrapper>
                                            <ThemeProvider appId="iota-dashboard">
                                                <CookieManagerProvider>
                                                    {children}
                                                    <Toaster containerClassName="!right-8" />
                                                    <CookieDisclaimer />
                                                </CookieManagerProvider>
                                            </ThemeProvider>
                                        </ClipboardPasteSafetyWrapper>
                                    </WalletProvider>
                                </KioskClientProvider>
                            </IotaNamesClientProvider>
                        </IotaGraphQLClientProvider>
                    </StardustIndexerClientProvider>
                </IotaClientProvider>
                <ReactQueryDevtools initialIsOpen={false} />
            </QueryClientProvider>
        </GrowthBookProvider>
    );
}
