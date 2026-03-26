// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { initAmplitude, setAmplitudeIdentity } from '@/lib/utils/analytics';
import { useEffect } from 'react';
import { useIotaClientContext } from '@iota/dapp-kit';

// Initialize Amplitude immediately when this module loads (client-side only)
let amplitudeInitialized = false;
let amplitudeInitPromise: Promise<void> | null = null;

if (typeof window !== 'undefined' && !amplitudeInitialized) {
    amplitudeInitialized = true;
    amplitudeInitPromise = initAmplitude();
}

export function Amplitude() {
    const clientContext = useIotaClientContext();
    const activeNetwork = clientContext.network;

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
