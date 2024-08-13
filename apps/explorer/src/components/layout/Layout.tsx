// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KioskClientProvider, useCookieConsentBanner } from '@iota/core';
import { IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import type { Network } from '@iota/iota-sdk/client';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { Fragment } from 'react';
import { resolveValue, Toaster, type ToastType } from 'react-hot-toast';
import { Outlet, ScrollRestoration } from 'react-router-dom';

import { NetworkContext } from '~/contexts';
import { useInitialPageView, useNetwork } from '~/hooks';
import { createIotaClient, persistableStorage, SupportedNetworks } from '~/lib/utils';
import { Banner, type BannerProps } from '~/components/ui';

const TOAST_VARIANTS: Partial<Record<ToastType, BannerProps['variant']>> = {
    success: 'positive',
    error: 'error',
};

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
                            <Outlet />
                            <Toaster
                                position="bottom-center"
                                gutter={8}
                                containerStyle={{
                                    top: 40,
                                    left: 40,
                                    bottom: 40,
                                    right: 40,
                                }}
                                toastOptions={{
                                    duration: 4000,
                                }}
                            >
                                {(toast) => (
                                    <Banner shadow border variant={TOAST_VARIANTS[toast.type]}>
                                        {resolveValue(toast.message, toast)}
                                    </Banner>
                                )}
                            </Toaster>
                            <ReactQueryDevtools />
                        </NetworkContext.Provider>
                    </KioskClientProvider>
                </WalletProvider>
            </IotaClientProvider>
        </Fragment>
    );
}
