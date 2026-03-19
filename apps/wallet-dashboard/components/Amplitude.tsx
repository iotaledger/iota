// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { ampli, initAmplitude, setAmplitudeIdentity } from '@/lib/utils/analytics';
import { useEffect, useRef } from 'react';
import { useIotaClientContext } from '@iota/dapp-kit';

// Initialize Amplitude immediately when this module loads (client-side only)
let amplitudeInitialized = false;
let amplitudeInitPromise: Promise<void> | null = null;

if (typeof window !== 'undefined' && !amplitudeInitialized) {
    amplitudeInitialized = true;
    amplitudeInitPromise = initAmplitude();
}

async function trackPageOpen() {
    // Wait for initialization to complete before tracking
    if (amplitudeInitPromise) {
        await amplitudeInitPromise;
    }

    ampli.openedWalletDashboard({
        pagePath: location.pathname,
        pagePathFragment: `${location.pathname}${location.search}${location.hash}`,
        walletDashboardRev: process.env.NEXT_PUBLIC_DASHBOARD_REV,
    });
}

export function Amplitude() {
    const hasTracked = useRef(false);
    const clientContext = useIotaClientContext();
    const activeNetwork = clientContext.network;

    useEffect(() => {
        if (!hasTracked.current) {
            hasTracked.current = true;
            trackPageOpen();
        }
    }, []);

    useEffect(() => {
        if (amplitudeInitPromise) {
            (async () => {
                await amplitudeInitPromise;
                setAmplitudeIdentity({ network: activeNetwork });
            })();
        }
    }, [activeNetwork]);

    return null;
}
