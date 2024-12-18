// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@iota/dapp-kit/dist/index.css';
import './index.css';
import '@fontsource-variable/inter';
import '@fontsource-variable/red-hat-mono';

import { IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { getFullnodeUrl } from '@iota/iota-sdk/client';
import { QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { RouterProvider } from 'react-router-dom';

import { queryClient } from './lib/queryClient';
import { router } from './routes';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
        <QueryClientProvider client={queryClient}>
            <IotaClientProvider
                defaultNetwork="iota:mainnet"
                networks={{
                    'iota:testnet': { url: getFullnodeUrl('testnet') },
                    'iota:mainnet': { url: getFullnodeUrl('mainnet') },
                    'iota:devnet': { url: getFullnodeUrl('devnet') },
                }}
            >
                <WalletProvider>
                    <RouterProvider router={router} />
                </WalletProvider>
            </IotaClientProvider>
        </QueryClientProvider>
    </React.StrictMode>,
);
