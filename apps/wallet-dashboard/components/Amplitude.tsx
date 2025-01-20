// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { ampli, initAmplitude } from '@/lib/utils/analytics';
import { useEffect } from 'react';

async function load() {
    await initAmplitude();
    await ampli.openedWalletDashboard({
        pagePath: location.pathname,
        pagePathFragment: `${location.pathname}${location.search}${location.hash}`,
        walletDashboardRev: process.env.NEXT_PUBLIC_DASHBOARD_REV,
    });
}

export function Amplitude() {
    useEffect(() => {
        load();
    }, []);

    return null;
}
