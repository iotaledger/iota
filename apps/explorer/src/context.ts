// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as Sentry from '@sentry/react';
import { createContext, useContext, useLayoutEffect, useMemo } from 'react';
// eslint-disable-next-line no-restricted-imports
import { useSearchParams } from 'react-router-dom';

import { Network } from './utils/api/defaultRpcClient';
import { growthbook } from './utils/growthbook';
import { queryClient } from './utils/queryClient';
import { getDefaultNetwork } from '@iota/iota.js/client';

export const NetworkContext = createContext<
    [Network | string, (network: Network | string) => void]
>(['', () => null]);

export function useNetworkContext(): [Network | string, (network: Network | string) => void] {
    return useContext(NetworkContext);
}

export function useNetwork(): [string, (network: Network | string) => void] {
    const [searchParams, setSearchParams] = useSearchParams();

    const network = useMemo(() => {
        const networkParam = searchParams.get('network');

        if (
            networkParam &&
            (Object.values(Network) as string[]).includes(networkParam.toUpperCase())
        ) {
            return networkParam.toUpperCase();
        }

        return networkParam ?? getDefaultNetwork();
    }, [searchParams]);

    const setNetwork = (network: Network | string) => {
        // When resetting the network, we reset the query client at the same time:
        queryClient.cancelQueries();
        queryClient.clear();

        setSearchParams({ network: network.toLowerCase() });
    };

    useLayoutEffect(() => {
        growthbook.setAttributes({
            network,
            environment: import.meta.env.VITE_VERCEL_ENV,
        });

        Sentry.setContext('network', {
            network,
        });
    }, [network]);

    return [network, setNetwork];
}
