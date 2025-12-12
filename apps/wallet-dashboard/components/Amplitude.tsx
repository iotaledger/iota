// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    ampli,
    initAmplitude,
    parseNetworkIdentifier,
    setNetworkGroup,
} from '@/lib/utils/analytics';
import { useIotaClientContext } from '@iota/dapp-kit';
import { useEffect } from 'react';

async function load() {
    await initAmplitude();
    ampli.openedWalletDashboard({
        pagePath: location.pathname,
        pagePathFragment: `${location.pathname}${location.search}${location.hash}`,
        walletDashboardRev: process.env.NEXT_PUBLIC_DASHBOARD_REV,
    });
}

export function Amplitude() {
    const clientContext = useIotaClientContext();
    const activeNetwork = clientContext.network;
    const { network, customRpc } = parseNetworkIdentifier(activeNetwork);

    useEffect(() => {
        load();
    }, []);

    useEffect(() => {
        ampli.identify(undefined);
        setNetworkGroup(network, customRpc);
    }, [network, customRpc]);

    return null;
}
