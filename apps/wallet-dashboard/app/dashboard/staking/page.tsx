// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { AmountBox, StakeCard, StakeDetailsPopup } from '@/components/index';
import { usePopups } from '@/hooks';
import { Stake } from '@/lib/types';
import { useGetDelegatedStake } from '@mysten/core';
import { useCurrentAccount } from '@mysten/dapp-kit';

// Unit used to convert stake and rewards from raw values. This is temporary and will be removed once we have the correct conversion factor.
const HARDCODED_CONVERSION_UNIT = 1000000000;

function StakingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const { openPopup } = usePopups();
    const { data: stakeData } = useGetDelegatedStake({
        address: account?.address || '',
    });

    const totalStake =
        stakeData?.reduce((sum, validator) => {
            return (
                sum +
                validator.stakes.reduce((stakeSum, stake) => stakeSum + Number(stake.principal), 0)
            );
        }, 0) ?? 0;
    const totalRewards =
        stakeData?.reduce((sum, validator) => {
            return (
                sum +
                validator.stakes.reduce(
                    (rewardSum, stake) =>
                        stake.status === 'Active'
                            ? rewardSum + Number(stake.estimatedReward)
                            : rewardSum,
                    0,
                )
            );
        }, 0) ?? 0;
    const totalStakeConverted = totalStake / HARDCODED_CONVERSION_UNIT;
    const totalRewardsConverted = totalRewards / HARDCODED_CONVERSION_UNIT;
    const stakingList = stakeData?.flatMap((validator, index) =>
        validator.stakes.map((stake, stakeIndex) => ({
            id: `${index}-${stakeIndex}`,
            validator: validator.validatorAddress,
            stake: `${Number(stake.principal) / HARDCODED_CONVERSION_UNIT}`,
            rewards:
                stake.status === 'Active'
                    ? `${Number(stake.estimatedReward) / HARDCODED_CONVERSION_UNIT}`
                    : '0',
            stakeActiveEpoch: stake.stakeActiveEpoch,
            stakeRequestEpoch: stake.stakeRequestEpoch,
            status: stake.status,
        })),
    );

    const handleOpenPopup = (stake: Stake) => {
        openPopup(<StakeDetailsPopup {...stake} />);
    };

    return (
        <div className="flex flex-col items-center justify-center gap-4 pt-12">
            <AmountBox title="Currently staked" amount={`${totalStakeConverted}`} />
            <AmountBox title="Earned" amount={`${totalRewardsConverted}`} />
            <div className="flex flex-col items-center gap-4">
                <h1>List of stakes</h1>
                {stakingList?.map((stake) => (
                    <StakeCard key={stake.id} stake={stake} onDetailsClick={handleOpenPopup} />
                ))}
            </div>
        </div>
    );
}

export default StakingDashboardPage;
