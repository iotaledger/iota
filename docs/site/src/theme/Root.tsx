/**
 * SWIZZLED VERSION: 3.5.2
 * REASONS:
 *  - Wrap the docs with QueryClientProvider, IotaClientProvider and WalletProvider
 */
import React, { useMemo } from 'react';
import { darkTheme, IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { getDefaultNetwork, getFullnodeUrl } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import '@iota/dapp-kit/dist/index.css';

import { ContextProviders } from '@site/src/components/ContextProviders';
import Disclaimer from '@site/src/components/Disclaimer';

const NETWORKS = {
    [getDefaultNetwork()]: { url: getFullnodeUrl('testnet') },
};

export default function Root({ children }) {
    // Work around server-side pre-rendering
    const queryClient = useMemo(() => new QueryClient(), []);

    return (
        <QueryClientProvider client={queryClient}>
            <IotaClientProvider networks={NETWORKS}>
                <WalletProvider theme={darkTheme}>
                    <ContextProviders>
						{children}
						<Disclaimer />
					</ContextProviders>
                </WalletProvider>
            </IotaClientProvider>
        </QueryClientProvider>
    );
}