// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MILLISECONDS_PER_MINUTE } from '@iota/core';
import { getAppsBackend } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';
import { getEnvironmentKey } from '_src/shared/experimentation/features';
import { useEffect } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';

export const RESTRICTED_ERROR = {
    status: 403,
};

export function useRestrictedGuard() {
    const navigate = useNavigate();
    const location = useLocation();
    const backendUrl = getAppsBackend();

    const { data } = useQuery({
        queryKey: ['restricted-guard', backendUrl],
        queryFn: async () => {
            // NOTE: We use fetch directly here instead of the RPC layer because we don't want this instrumented,
            // and we also need to work with the response object directly.
            const res = await fetch(`${backendUrl}/api/restricted/`, {
                method: 'POST',
                headers: {
                    // Resetting accept makes the response non-HTML
                    accept: '',
                    'content-type': 'application/json',
                },
                body: JSON.stringify({
                    env: getEnvironmentKey(),
                }),
            });

            return { restricted: res.status === RESTRICTED_ERROR.status };
        },

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
