// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@iota/dapp-kit/dist/index.css';
import './globals.css';

import * as Sentry from '@sentry/nextjs';
import type { Metadata } from 'next';
import { Suspense } from 'react';

import { ConnectionGuard } from '@/components';
import { AvailabilityCheckDialog } from '@/components/availability-check/AvailabilityCheckDialog';
import { Footer } from '@/components/layout';
import { StickyHeader } from '@/components/layout/StickyHeader';
import { DEFAULT_METADATA } from '@/lib/constants/metadata.constants';
import { APP_STATIC_THEME } from '@/lib/constants/theme.constants';
import { AppProviders } from '@/providers';

export function generateMetadata(): Metadata {
    return {
        ...DEFAULT_METADATA,
        other: {
            ...Sentry.getTraceData(),
        },
    };
}

export default function RootLayout({
    children,
}: Readonly<{
    children: React.ReactNode;
}>) {
    return (
        <html lang="en" className={APP_STATIC_THEME}>
            <body className="antialiased">
                <AppProviders>
                    <Suspense>
                        <ConnectionGuard>
                            <StickyHeader />
                            {children}
                            <AvailabilityCheckDialog />
                            <Footer />
                        </ConnectionGuard>
                    </Suspense>
                </AppProviders>
            </body>
        </html>
    );
}
