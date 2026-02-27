// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@rainbow-me/rainbowkit/styles.css';
import '@iota/dapp-kit/dist/index.css';
import './globals.css';

import React from 'react';
import ReactDOM from 'react-dom/client';
import {
    getDefaultConfig,
    darkTheme as rainbowDarkTheme,
    lightTheme as rainbowLightTheme,
    RainbowKitProvider,
    Chain,
} from '@rainbow-me/rainbowkit';
import { darkTheme, IotaClientProvider, lightTheme, WalletProvider } from '@iota/dapp-kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from './App.tsx';
import { ThemeProvider } from './providers/ThemeProvider.tsx';
import { WagmiProvider } from 'wagmi';
import { useTheme } from './hooks/useTheme.ts';
import { Theme } from './lib/enums/index.ts';
import {
    getDefaultNetwork,
    L2_CHAIN_CONFIG,
    L2_WAGMI_CONFIG,
    networkConfig,
} from './config/index.ts';
import { EvmRpcClientProvider } from './providers/EvmRpcClientProvider.tsx';
import { Toaster } from './components/index.ts';
import { IotaGraphQLClientProvider, Disclaimer, handleConsentAccepted } from '@iota/core';
import { growthbook, interceptProviderAnnouncements } from './lib/utils/index.ts';
import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { getNetwork } from '@iota/iota-sdk/client';
import { metaMaskWallet, walletConnectWallet } from '@rainbow-me/rainbowkit/wallets';
import { LEGAL_LINKS } from './lib/constants/routes.constants.ts';
import { Link } from './components/link/Link.tsx';
import { initAmplitude } from './shared/analytics';

// We intercept EIP-6963 announcements
// to only allow certain wallets (metamask) to be discovered
interceptProviderAnnouncements();

growthbook.init();

// Load Amplitude as early as we can (respects opt-out based on consent status):
initAmplitude();

const queryClient = new QueryClient();

const wagmiConfig = getDefaultConfig({
    ...L2_WAGMI_CONFIG,
    chains: [L2_CHAIN_CONFIG as Chain],
    wallets: [
        {
            groupName: 'Suggested',
            wallets: [metaMaskWallet, walletConnectWallet],
        },
    ],
});

ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
        <WagmiProvider config={wagmiConfig}>
            <GrowthBookProvider growthbook={growthbook}>
                <EvmRpcClientProvider baseUrl={L2_CHAIN_CONFIG.evmRpcUrl}>
                    <QueryClientProvider client={queryClient}>
                        <IotaClientProvider
                            networks={networkConfig}
                            defaultNetwork={getDefaultNetwork()}
                        >
                            <IotaGraphQLClientProvider>
                                <WalletProvider
                                    autoConnect
                                    theme={[
                                        {
                                            variables: lightTheme,
                                        },
                                        {
                                            selector: '.dark',
                                            variables: darkTheme,
                                        },
                                    ]}
                                    chain={getNetwork(getDefaultNetwork()).chain}
                                >
                                    <ThemeProvider appId="IOTA-evm-bridge">
                                        <RainbowKit>
                                            <App />
                                            <Toaster />
                                            <Disclaimer
                                                onClose={() => {
                                                    handleConsentAccepted();
                                                }}
                                            >
                                                <div className="text-body-md text-iota-neutral-10 dark:text-iota-neutral-92">
                                                    By using this website, you agree with our{' '}
                                                    {LEGAL_LINKS.map(({ text, url }, index) => (
                                                        <React.Fragment key={text}>
                                                            <Link isExternal href={url}>
                                                                {text}
                                                            </Link>
                                                            {index < LEGAL_LINKS.length - 1
                                                                ? ', '
                                                                : ''}
                                                        </React.Fragment>
                                                    ))}
                                                </div>
                                            </Disclaimer>
                                        </RainbowKit>
                                    </ThemeProvider>
                                </WalletProvider>
                            </IotaGraphQLClientProvider>
                        </IotaClientProvider>
                    </QueryClientProvider>
                </EvmRpcClientProvider>
            </GrowthBookProvider>
        </WagmiProvider>
    </React.StrictMode>,
);

function RainbowKit({ children }: React.PropsWithChildren) {
    const { theme: currentTheme } = useTheme();
    const theme = currentTheme === Theme.Dark ? rainbowDarkTheme() : rainbowLightTheme();

    return (
        <RainbowKitProvider
            initialChain={L2_CHAIN_CONFIG}
            modalSize="compact"
            theme={{
                ...theme,
                ...{
                    radii: {
                        ...theme.radii,
                        connectButton: '999px',
                    },
                },
            }}
        >
            {children}
        </RainbowKitProvider>
    );
}
