// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getDefaultNetwork, Network } from '@iota/iota-sdk/client';
import { useLayoutEffect, useMemo } from 'react';
// eslint-disable-next-line no-restricted-imports
import { useSearchParams } from 'react-router-dom';
import { growthbook } from '~/lib/utils/growthbook';
import { queryClient } from '~/lib/utils/queryClient';
import * as Sentry from '@sentry/react';

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
