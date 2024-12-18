// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useCoinsReFetchingConfig } from '_app/hooks/useCoinsReFetchingConfig';
import { useIotaClientQuery } from '@iota/dapp-kit';

export function useGetAllBalances(owner: string) {
    const { staleTime, refetchInterval } = useCoinsReFetchingConfig();

    return useIotaClientQuery(
        'getAllBalances',
        { owner: owner! },
        {
            enabled: !!owner,
            refetchInterval,
            staleTime,
        },
    );
}
