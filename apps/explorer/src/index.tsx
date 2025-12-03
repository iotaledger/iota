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

initSentry();

import '@iota/dapp-kit/dist/index.css';
import './index.css';
import { Disclaimer, setCookieAccepted } from '@iota/core';
import { LEGAL_LINKS } from './lib';
import { Link } from './components';

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
                    <Disclaimer onClose={setCookieAccepted}>
                        <div>
                            By using this website, you agree with our{' '}
                            {LEGAL_LINKS.map((link, index) => (
                                <React.Fragment key={link.href}>
                                    <Link
                                        href={link.href}
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        className="underline hover:text-white"
                                    >
                                        {link.title}
                                    </Link>
                                    {index < LEGAL_LINKS.length - 1 && ' and '}
                                </React.Fragment>
                            ))}
                        </div>
                    </Disclaimer>
                </CookieManagerProvider>
            </QueryClientProvider>
        </GrowthBookProvider>
    </React.StrictMode>,
);
