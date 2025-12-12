// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@fontsource-variable/inter';
import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { RouterProvider } from 'react-router-dom';
import { CookieManagerProvider } from '@boxfish-studio/react-cookie-manager';
import { growthbook, initAmplitude, initSentry, queryClient } from './lib/utils';
import { router } from './pages';
import { CookieDisclaimer } from './components/disclaimer/CookieDisclaimer';

initSentry();

import '@iota/dapp-kit/dist/index.css';
import './index.css';

// Load Amplitude as early as we can:
initAmplitude();

// Start loading features as early as we can:
growthbook.loadFeatures();

ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
        <GrowthBookProvider growthbook={growthbook}>
            <QueryClientProvider client={queryClient}>
                <CookieManagerProvider>
                    <RouterProvider router={router} />
                    <CookieDisclaimer />
                </CookieManagerProvider>
            </QueryClientProvider>
        </GrowthBookProvider>
    </React.StrictMode>,
);
