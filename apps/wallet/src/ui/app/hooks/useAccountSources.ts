// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AccountSourceSerializedUI } from '_src/background/account-sources/accountSource';
import { useQuery } from '@tanstack/react-query';

import { useBackgroundClient } from './useBackgroundClient';

export const ACCOUNT_SOURCES_QUERY_KEY = ['background', 'client', 'account', 'sources'];

export function useAccountSources() {
    const backgroundClient = useBackgroundClient();
    return useQuery({
        queryKey: ACCOUNT_SOURCES_QUERY_KEY,
        queryFn: () =>
            backgroundClient.getStoredEntities<AccountSourceSerializedUI>('accountSources'),
        gcTime: 30 * 1000,
        staleTime: 15 * 1000,
        refetchInterval: 30 * 1000,
        meta: { skipPersistedCache: true },
    });
}
