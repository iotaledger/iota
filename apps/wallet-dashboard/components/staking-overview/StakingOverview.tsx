// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { usePopups } from '@/hooks';
import {
    ExtendedDelegatedStake,
    formatDelegatedStake,
    useFormatCoin,
    useGetDelegatedStake,
    useTotalDelegatedRewards,
    useTotalDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { Address, Button, ButtonSize, ButtonType, Panel } from '@iota/apps-ui-kit';
import { StartStaking } from './StartStaking';

export function StakingOverview() {
    const account = useCurrentAccount();
    const { openPopup, closePopup } = usePopups();
    const { data: delegatedStakeData } = useGetDelegatedStake({
        address: account?.address || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const extendedStakes = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];
    const totalDelegatedStake = useTotalDelegatedStake(extendedStakes);
    const totalDelegatedRewards = useTotalDelegatedRewards(extendedStakes);
    // const [formattedDelegatedStake, stakeSymbol, stakeResult] = useFormatCoin(
    //     totalDelegatedStake,
    //     IOTA_TYPE_ARG,
    // );
    // const [formattedDelegatedRewards, rewardsSymbol, rewardsResult] = useFormatCoin(
    //     totalDelegatedRewards,
    //     IOTA_TYPE_ARG,
    // );

    // const viewStakeDetails = (extendedStake: ExtendedDelegatedStake) => {
    //     openPopup(<StakeDetailsPopup extendedStake={extendedStake} onClose={closePopup} />);
    // };

    // const addNewStake = () => {
    //     openPopup(<NewStakePopup onClose={closePopup} />);
    // };

    return <StartStaking />;
}
