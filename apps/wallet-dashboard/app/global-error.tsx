// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@iota/dapp-kit/dist/index.css';
import './globals.css';
import { Inter } from 'next/font/google';
import { AppProviders } from '@/providers';
import { FontLinks } from '@/components/FontLinks';
import { useEffect } from 'react';
import { captureException } from '@/instrumentation';

const inter = Inter({ subsets: ['latin'] });

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string }
  reset: () => void
}) {
    useEffect(() => {
        captureException(error)
    },[error])

    return (
        <html lang="en">
            <body className={inter.className}>
                <AppProviders>
                    <FontLinks />
                    <h2>Something went wrong!</h2>
                </AppProviders>
            </body>
        </html>
    );
}