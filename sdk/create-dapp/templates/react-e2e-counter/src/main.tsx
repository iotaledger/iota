import React from 'react';
import ReactDOM from 'react-dom/client';
import '@iota/dapp-kit/dist/index.css';
import './index.css';

import { IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from './App.tsx';
import { networkConfig } from './networkConfig.ts';

const queryClient = new QueryClient();

ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
        <QueryClientProvider client={queryClient}>
            <IotaClientProvider networks={networkConfig} defaultNetwork="testnet">
                <WalletProvider autoConnect>
                    <App />
                </WalletProvider>
            </IotaClientProvider>
        </QueryClientProvider>
    </React.StrictMode>,
);
