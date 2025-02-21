// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@fontsource-variable/inter';
import '@fontsource-variable/red-hat-mono';
import { ErrorBoundary } from '_components';
import { initAppType } from '_redux/slices/app';
import { AppType, getFromLocationSearch } from '_src/ui/app/redux/slices/app/appType';
import { initAmplitude } from '_src/shared/analytics/amplitude';
import { setAttributes } from '_src/shared/experimentation/features';
import { initSentry } from '_src/ui/app/helpers';
import store from '_store';
import { thunkExtras } from '_src/ui/app/redux/store/thunkExtras';
import { KioskClientProvider, StardustIndexerClientProvider, ThemeProvider } from '@iota/core';
import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { IotaClientProvider } from '@iota/dapp-kit';
import { PersistQueryClientProvider } from '@tanstack/react-query-persist-client';
import cn from 'clsx';
import { Fragment, StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Provider } from 'react-redux';
import { HashRouter } from 'react-router-dom';
import { App } from './app';
import { walletApiProvider } from './app/apiProvider';
import { AccountsFormProvider } from './app/components/accounts/AccountsFormContext';
import { UnlockAccountProvider } from './app/components/accounts/UnlockAccountContext';
import { IotaLedgerClientProvider } from './app/components/ledger/IotaLedgerClientProvider';
import { growthbook } from './app/experimentation/featureGating';
import { persister, queryClient } from './app/helpers/queryClient';
import { useAppSelector } from '_hooks';

import './styles/global.scss';
import 'bootstrap-icons/font/bootstrap-icons.scss';
import { defaultShouldDehydrateQuery, type Query } from '@tanstack/react-query';

async function init() {
    if (process.env.NODE_ENV === 'development') {
        Object.defineProperty(window, 'store', { value: store });
    }
    store.dispatch(initAppType(getFromLocationSearch()));
    await thunkExtras.background.init(store.dispatch);
    const { network, customRpc } = store.getState().app;
    setAttributes({ network, customRpc });
}

function renderApp() {
    const rootDom = document.getElementById('root');
    if (!rootDom) {
        throw new Error('Root element not found');
    }
    const root = createRoot(rootDom);
    root.render(
        <StrictMode>
            <Provider store={store}>
                <AppWrapper />
            </Provider>
        </StrictMode>,
    );
}

function AppWrapper() {
    const network = useAppSelector(({ app: { network, customRpc } }) => `${network}_${customRpc}`);
    const isFullscreen = useAppSelector((state) => state.app.appType === AppType.Fullscreen);
    return (
        <GrowthBookProvider growthbook={growthbook}>
            <HashRouter>
                <IotaLedgerClientProvider>
                    {/*
                     * NOTE: We set a key here to force the entire react tree to be re-created when the network changes so that
                     * the RPC client instance (api.instance.fullNode) is updated correctly. In the future, we should look into
                     * making the API provider instance a reactive value and moving it out of the redux-thunk middleware
                     */}
                    <Fragment key={network}>
                        <PersistQueryClientProvider
                            client={queryClient}
                            persistOptions={{
                                persister,
                                dehydrateOptions: {
                                    shouldDehydrateQuery: (query: Query) => {
                                        return (
                                            !query.meta?.skipPersistedCache &&
                                            defaultShouldDehydrateQuery(query)
                                        );
                                    },
                                },
                            }}
                        >
                            <IotaClientProvider
                                networks={{
                                    [walletApiProvider.network]:
                                        walletApiProvider.instance.fullNode,
                                }}
                            >
                                <StardustIndexerClientProvider>
                                    <KioskClientProvider>
                                        <AccountsFormProvider>
                                            <ThemeProvider appId="iota-wallet">
                                                <UnlockAccountProvider>
                                                    <div
                                                        className={cn(
                                                            'relative flex h-screen max-h-popup-height min-h-popup-minimum w-popup-width flex-col flex-nowrap items-center justify-center overflow-hidden',
                                                            isFullscreen && 'rounded-xl shadow-lg',
                                                        )}
                                                    >
                                                        <ErrorBoundary>
                                                            <App />
                                                        </ErrorBoundary>
                                                        <div id="overlay-portal-container"></div>
                                                        <div id="toaster-portal-container"></div>
                                                    </div>
                                                </UnlockAccountProvider>
                                            </ThemeProvider>
                                        </AccountsFormProvider>
                                    </KioskClientProvider>
                                </StardustIndexerClientProvider>
                            </IotaClientProvider>
                        </PersistQueryClientProvider>
                    </Fragment>
                </IotaLedgerClientProvider>
            </HashRouter>
        </GrowthBookProvider>
    );
}

(async () => {
    await init();
    initSentry();
    initAmplitude();
    renderApp();
})();
