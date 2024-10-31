// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Inter } from 'next/font/google';

import '@iota/dapp-kit/dist/index.css';
import './globals.css';
import React from 'react';
import { AppProviders } from '@/providers';
import { Metadata } from 'next';
import { FontLinkStyle } from '@/components/FontLinkStyle';

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
                <FontLinkStyle />
                <AppProviders>{children}</AppProviders>
            </body>
        </html>
    );
}
