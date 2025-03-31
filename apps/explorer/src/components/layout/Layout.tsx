// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KioskClientProvider, useCookieConsentBanner, ThemeProvider, Toaster } from '@iota/core';
import { IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import type { Network } from '@iota/iota-sdk/client';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { Fragment } from 'react';
import { Outlet, ScrollRestoration } from 'react-router-dom';
import { NetworkContext } from '~/contexts';
import { useInitialPageView, useNetwork } from '~/hooks';
import { createIotaClient, persistableStorage, SupportedNetworks } from '~/lib/utils';

export function Layout(): JSX.Element {
    const [network, setNetwork] = useNetwork();

    useCookieConsentBanner(persistableStorage, {
        cookie_name: 'iota_explorer_cookie_consent',
        onBeforeLoad: async () => {
            await import('./CookieConsent.css');
            document.body.classList.add('cookie-consent-theme');
        },
    });

    useInitialPageView(network);

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
                <WalletProvider autoConnect enableUnsafeBurner={import.meta.env.DEV}>
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
            </IotaClientProvider>
        </Fragment>
    );
}
