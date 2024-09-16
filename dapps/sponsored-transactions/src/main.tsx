// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import ReactDOM from 'react-dom/client';

import { App } from './App';

import '@iota/dapp-kit/dist/index.css';
import './index.css';

import { getFullnodeUrl } from '@iota/iota-sdk/client';

const queryClient = new QueryClient();

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
        <QueryClientProvider client={queryClient}>
            <IotaClientProvider
                defaultNetwork="testnet"
                networks={{ testnet: { url: getFullnodeUrl('testnet') } }}
            >
                <WalletProvider enableUnsafeBurner>
                    <App />
                </WalletProvider>
            </IotaClientProvider>
        </QueryClientProvider>
    </React.StrictMode>,
);
