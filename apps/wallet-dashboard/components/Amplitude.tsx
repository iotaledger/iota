// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { ampli, initAmplitude } from '@/lib/utils/analytics';
import { useIotaClientContext } from '@iota/dapp-kit';
import { useEffect } from 'react';

export function Amplitude() {
    const { network } = useIotaClientContext();

    // Handle network availability and changes
    useEffect(() => {
        if (!network || !process.env.NODE_ENV) {
            return;
        }

        (async () => {
            if (!ampli.isLoaded) {
                await initAmplitude();
                await ampli.identify(undefined, {
                    groups: {
                        network: network,
                    },
                }).promise;
                ampli.openedWalletDashboard({
                    pagePath: location.pathname,
                    pagePathFragment: `${location.pathname}${location.search}${location.hash}`,
                    walletDashboardRev: process.env.NEXT_PUBLIC_DASHBOARD_REV,
                });
            } else {
                await ampli.identify(undefined, {
                    groups: {
                        network: network,
                    },
                }).promise;
            }
        })();
    }, [network]);

    return null;
}
