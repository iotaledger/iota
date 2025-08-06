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
import { IotaGraphQLClientProvider } from '@iota/core';
import { growthbook } from './lib/utils/index.ts';
import { GrowthBookProvider } from '@growthbook/growthbook-react';

growthbook.init();

const queryClient = new QueryClient();

ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
        <WagmiProvider
            config={getDefaultConfig({
                ...L2_WAGMI_CONFIG,
                chains: [L2_CHAIN_CONFIG as Chain],
            })}
        >
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
                                >
                                    <ThemeProvider appId="IOTA-evm-bridge">
                                        <RainbowKit>
                                            <App />
                                            <Toaster />
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
