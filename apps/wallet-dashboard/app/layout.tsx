// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@iota/dapp-kit/dist/index.css';
import './globals.css';
import { Inter } from 'next/font/google';
import { ErrorBoundary } from '@/components/error-boundary';
import { AppProviders } from '@/providers';
import { FontLinks } from '@/components/FontLinks';
import { ConnectionGuard } from '@/components/connection-guard';
import { Amplitude } from '@/components/Amplitude';
import { METADATA } from '@/lib/constants';

const inter = Inter({ subsets: ['latin'] });

export const metadata = METADATA;

export default function RootLayout({
    children,
}: Readonly<{
    children: React.ReactNode;
}>) {
    return (
        <html lang="en">
            <body className={inter.className}>
                <AppProviders>
                    <FontLinks />
                    <Amplitude />
                    <ConnectionGuard>
                        <ErrorBoundary>{children}</ErrorBoundary>
                    </ConnectionGuard>
                </AppProviders>
            </body>
        </html>
    );
}
