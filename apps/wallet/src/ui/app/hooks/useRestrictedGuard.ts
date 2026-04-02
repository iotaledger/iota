// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppsBackendClient } from '@iota/apps-backend-client';
import { MILLISECONDS_PER_MINUTE } from '@iota/core';
import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';

export function useRestrictedGuard() {
    const navigate = useNavigate();
    const location = useLocation();
    const client = useAppsBackendClient();

    const { data } = useQuery({
        queryKey: ['restricted-guard'],
        queryFn: async () => ({ restricted: await client.checkRestricted() }),
        // Refetch every 5 minutes to ensure all wallets remain disabled, even if they have been open for a long time.
        refetchInterval: 5 * MILLISECONDS_PER_MINUTE,
        gcTime: 0,
        retry: 0,
        meta: {
            skipPersistedCache: true,
        },
    });

    useEffect(() => {
        if (!data) return;
        if (data.restricted && location.pathname !== '/restricted') {
            navigate('/restricted', { replace: true });
        } else if (!data.restricted && location.pathname === '/restricted') {
            // If access is not restricted, but the user is on the restricted page, then we want to get them out of there:
            navigate('/', { replace: true });
        }
    }, [navigate, data, location.pathname]);

    return data?.restricted ?? false;
}
