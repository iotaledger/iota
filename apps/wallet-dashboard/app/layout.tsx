// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@iota/dapp-kit/dist/index.css';
import './globals.css';
import { Inter } from 'next/font/google';
import { Metadata } from 'next';
import { AppProviders } from '@/providers';
import { FontLinks } from '@/components/FontLinks';
import { ConnectionGuard } from '@/components/connection-guard';
import { Amplitude } from '@/components/Amplitude';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
    title: 'IOTA Wallet Dashboard',
    description: 'IOTA Wallet Dashboard',
};

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
                    <ConnectionGuard>{children}</ConnectionGuard>
                </AppProviders>
            </body>
        </html>
    );
}
