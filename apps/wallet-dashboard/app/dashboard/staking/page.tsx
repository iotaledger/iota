// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { AmountBox, Box, StakeCard, NewStakePopup, StakeDetailsPopup, Button } from '@/components';
import { usePopups } from '@/hooks';
import {
    DelegatedStakeWithValidator,
    formatDelegatedStake,
    useFormatCoin,
    useGetDelegatedStake,
    useTotalDelegatedRewards,
    useTotalDelegatedStake,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';

function StakingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const { openPopup, closePopup } = usePopups();
    const { data: delegatedStakeData } = useGetDelegatedStake({
        address: account?.address || '',
    });

    const delegatedStakes = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStakes);
    const totalDelegatedRewards = useTotalDelegatedRewards(delegatedStakes);
    const [formattedDelegatedStake, symbol, queryResultStake] = useFormatCoin(
        totalDelegatedStake,
        IOTA_TYPE_ARG,
    );
    const [formattedDelegatedRewards, symbolRewards, queryResultRewards] = useFormatCoin(
        totalDelegatedRewards,
        IOTA_TYPE_ARG,
    );

    const viewStakeDetails = (stake: DelegatedStakeWithValidator) => {
        openPopup(<StakeDetailsPopup stake={stake} onClose={closePopup} />);
    };

    const addNewStake = () => {
        openPopup(<NewStakePopup onClose={closePopup} />);
    };

    return (
        <div className="flex flex-col items-center justify-center gap-4 pt-12">
            <AmountBox
                title="Currently staked"
                amount={queryResultStake.isPending ? '-' : `${formattedDelegatedStake} ${symbol}`}
            />
            <AmountBox
                title="Earned"
                amount={`${
                    queryResultRewards.isPending ? '-' : formattedDelegatedRewards
                } ${symbolRewards}`}
            />
            <Box title="Stakes">
                <div className="flex flex-col items-center gap-4">
                    <h1>List of stakes</h1>
                    {delegatedStakes?.map((stake) => (
                        <StakeCard
                            key={stake.stakedIotaId}
                            stake={stake}
                            onDetailsClick={viewStakeDetails}
                        />
                    ))}
                </div>
            </Box>
            <Button onClick={addNewStake}>New Stake</Button>
        </div>
    );
}

export default StakingDashboardPage;
