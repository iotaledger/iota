// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useEffect, useRef } from 'react';
import { usePathname, useSearchParams } from 'next/navigation';
import { ampli, initAmplitude } from '@/lib/utils/analytics';
import { Network } from '@iota/iota-sdk/client';
import { useIotaClientContext } from '@iota/dapp-kit';

export function Amplitude() {
    const clientContext = useIotaClientContext();
    const networkId = clientContext.network as Network;
    const pathname = usePathname();
    const searchParams = useSearchParams();
    const previousUrlRef = useRef<string>('');

    // Initialize Amplitude once
    useEffect(() => {
        initAmplitude(networkId, clientContext.config?.url);
    }, [networkId, clientContext.config?.url]);

    // Track page changes
    useEffect(() => {
        if (!ampli.isLoaded) return;
        console.log('openedWalletDashboard event');
        const search = searchParams.toString() ? `?${searchParams.toString()}` : '';
        const hash = window.location.hash || '';
        const currentUrl = `${pathname}${search}${hash}`;

        // Only call openedWalletDashboard if the URL has actually changed
        if (currentUrl !== previousUrlRef.current) {
            previousUrlRef.current = currentUrl;
            ampli.openedWalletDashboard({
                pagePath: pathname,
                pagePathFragment: currentUrl,
                walletDashboardRev: process.env.NEXT_PUBLIC_DASHBOARD_REV,
            });
        }
    }, [pathname, searchParams]);

    return null;
}
