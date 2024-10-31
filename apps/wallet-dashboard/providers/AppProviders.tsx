// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { PopupProvider } from '@/components';
import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { IotaClientProvider, lightTheme, darkTheme, WalletProvider } from '@iota/dapp-kit';
import { getAllNetworks, getDefaultNetwork } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState } from 'react';
import { growthbook } from '@/lib/utils';
import { Popup } from '@/components/Popup';
import { Toaster } from 'react-hot-toast';

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
                        <PopupProvider>
                            {children}
                            <Toaster />
                            <Popup />
                        </PopupProvider>
                    </WalletProvider>
                </IotaClientProvider>
            </QueryClientProvider>
        </GrowthBookProvider>
    );
}
