// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@fontsource-variable/inter';
import { GrowthBookProvider } from '@growthbook/growthbook-react';
import { QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { RouterProvider } from 'react-router-dom';
import { growthbook, initAmplitude, initSentry, queryClient } from './lib/utils';
import { router } from './pages';

initSentry();

import '@iota/dapp-kit/dist/index.css';
import './index.css';
import { Disclaimer, handleConsentAccepted } from '@iota/core';
import { LEGAL_LINKS } from './lib';
import { Link } from './components';
import { initIdentityWasmWeb } from './lib/utils/trust-framework/identity';

// Load Amplitude as early as we can:
initAmplitude();

// Load Identity WASM module as early as we can:
initIdentityWasmWeb();

// Start loading features as early as we can:
growthbook.refreshFeatures();

ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
        <GrowthBookProvider growthbook={growthbook}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
                <Disclaimer
                    onClose={() => {
                        handleConsentAccepted();
                    }}
                >
                    <div className="text-body-md text-iota-neutral-10 dark:text-iota-neutral-92">
                        By using this website, you agree with our{' '}
                        {LEGAL_LINKS.map(({ title, href }, index) => (
                            <React.Fragment key={title}>
                                <Link
                                    className="text-iota-primary-30 hover:text-iota-primary-50 dark:text-iota-primary-80 dark:hover:text-iota-primary-60"
                                    variant="mono"
                                    href={href}
                                >
                                    {title}
                                </Link>
                                {index < LEGAL_LINKS.length - 1 ? ', ' : ''}
                            </React.Fragment>
                        ))}
                    </div>
                </Disclaimer>
            </QueryClientProvider>
        </GrowthBookProvider>
    </React.StrictMode>,
);
