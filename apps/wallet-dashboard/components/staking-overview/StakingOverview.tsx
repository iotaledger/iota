// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useGetDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { StartStaking } from './StartStaking';
import { StakingData } from './StakingData';

export function StakingOverview() {
    const account = useCurrentAccount();
    const { data: delegatedStakeData } = useGetDelegatedStake({
        address: account?.address || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    return (delegatedStakeData?.length ?? 0) > 0 ? (
        <StakingData stakingData={delegatedStakeData} />
    ) : (
        <StartStaking />
    );
}
