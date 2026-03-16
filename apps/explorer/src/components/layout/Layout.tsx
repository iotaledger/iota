// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    KioskClientProvider,
    ThemeProvider,
    Toaster,
    IotaGraphQLClientProvider,
    IotaNamesClientProvider,
} from '@iota/core';
import { IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { getNetwork, type Network } from '@iota/iota-sdk/client';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { Fragment } from 'react';
import { Outlet, ScrollRestoration } from 'react-router-dom';
import { NetworkContext } from '~/contexts';
import { useAmplitudeIdentity, useNetwork } from '~/hooks';
import { createIotaClient, SupportedNetworks } from '~/lib/utils';
import { TrustFrameworkProvider } from '../trust-framework/trustFrameworkProvider';

export function Layout(): JSX.Element {
    const [network, setNetwork] = useNetwork();

    useAmplitudeIdentity(network);

    return (
        // NOTE: We set a top-level key here to force the entire react tree to be re-created when the network changes:
        <Fragment key={network}>
            <ScrollRestoration />
            <IotaClientProvider
                networks={SupportedNetworks}
                createClient={createIotaClient}
                network={network as Network}
                onNetworkChange={setNetwork}
            >
                <TrustFrameworkProvider>
                    <IotaGraphQLClientProvider>
                        <IotaNamesClientProvider>
                            <WalletProvider
                                autoConnect
                                enableUnsafeBurner={import.meta.env.DEV}
                                chain={getNetwork(network).chain}
                            >
                                <KioskClientProvider>
                                    <NetworkContext.Provider value={[network, setNetwork]}>
                                        <ThemeProvider appId="iota-explorer">
                                            <Outlet />
                                            <Toaster />
                                            <ReactQueryDevtools />
                                        </ThemeProvider>
                                    </NetworkContext.Provider>
                                </KioskClientProvider>
                            </WalletProvider>
                        </IotaNamesClientProvider>
                    </IotaGraphQLClientProvider>
                </TrustFrameworkProvider>
            </IotaClientProvider>
        </Fragment>
    );
}
