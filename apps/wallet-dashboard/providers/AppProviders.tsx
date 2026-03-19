// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { IotaClientProvider, lightTheme, darkTheme, WalletProvider } from '@iota/dapp-kit';
import { getAllNetworks, getDefaultNetwork, getNetwork } from '@iota/iota-sdk/client';
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import React, { useState } from 'react';
import {
    KioskClientProvider,
    StardustIndexerClientProvider,
    useLocalStorage,
    Toaster,
    ClipboardPasteSafetyWrapper,
    IotaGraphQLClientProvider,
    IotaNamesClientProvider,
    Disclaimer,
    setCookieAccepted,
} from '@iota/core';
import { growthbook } from '@/lib/utils';
import { ThemeProvider } from '@iota/core';
import { createIotaClient } from '@/lib/utils/defaultRpcClient';
import { captureException } from '@/instrumentation';
import { LEGAL_LINKS } from '@/lib/constants/routes.constants';
import { ExternalLink } from '@/components/ExternalLink';

growthbook.init();

export function AppProviders({ children }: React.PropsWithChildren) {
    const [queryClient] = useState(
        () =>
            new QueryClient({
                queryCache: new QueryCache({
                    onError: (error) => {
                        captureException(error);
                    },
                }),
                mutationCache: new MutationCache({
                    onError: (error) => {
                        captureException(error);
                    },
                }),
            }),
    );
    const allNetworks = getAllNetworks();
    const defaultNetworkId = getDefaultNetwork();
    const [persistedNetworkId, setPersistedNetworkId] = useLocalStorage<string>(
        'network_iota-dashboard',
        defaultNetworkId,
    );
    const persistedNetwork = getNetwork(persistedNetworkId);

    function handleNetworkChange(newNetwork: string) {
        setPersistedNetworkId(newNetwork);
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
                                                {children}
                                                <Toaster containerClassName="!right-8" />
                                                <Disclaimer onClose={setCookieAccepted}>
                                                    <div>
                                                        By using this website, you agree with our{' '}
                                                        {LEGAL_LINKS.map(
                                                            ({ title, href }, index) => (
                                                                <React.Fragment key={href}>
                                                                    <ExternalLink
                                                                        href={href}
                                                                        type="legal"
                                                                        className="text-iota-primary-30 hover:text-iota-primary-50 dark:text-iota-primary-80 dark:hover:text-iota-primary-60"
                                                                    >
                                                                        {title}
                                                                    </ExternalLink>
                                                                    {index < LEGAL_LINKS.length - 1
                                                                        ? ', '
                                                                        : ''}
                                                                </React.Fragment>
                                                            ),
                                                        )}
                                                    </div>
                                                </Disclaimer>
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
