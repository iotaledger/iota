// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useEffect, useRef, useState } from 'react';
import { usePathname, useSearchParams } from 'next/navigation';
import { ampli, initAmplitude } from '@/lib/utils/analytics';
import { Network } from '@iota/iota-sdk/client';
import { useIotaClientContext } from '@iota/dapp-kit';

export function Amplitude() {
    const clientContext = useIotaClientContext();
    const pathname = usePathname();
    const searchParams = useSearchParams();
    const [hash, setHash] = useState(window.location.hash);

    const initializedRef = useRef(false);

    // Initialize Amplitude once
    useEffect(() => {
        (async () => {
            await initAmplitude(clientContext.network as Network, clientContext.config?.url);
            initializedRef.current = true;
        })();
    }, [clientContext.network, clientContext.config?.url]);

    // Track hash changes
    useEffect(() => {
        setHash(window.location.hash);

        const handleHashChange = () => {
            setHash(window.location.hash);
        };

        window.addEventListener('hashchange', handleHashChange);
        return () => window.removeEventListener('hashchange', handleHashChange);
    }, []);

    // Track page changes
    useEffect(() => {
        if (!initializedRef.current) return;

        const search = searchParams.toString() ? `?${searchParams.toString()}` : '';
        ampli.openedWalletDashboard({
            pagePath: pathname,
            pagePathFragment: `${pathname}${search}${hash}`,
            walletDashboardRev: process.env.NEXT_PUBLIC_DASHBOARD_REV,
        });
    }, [pathname, searchParams, hash]);

    return null;
}
