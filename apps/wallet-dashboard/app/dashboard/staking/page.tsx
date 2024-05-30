// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { AmountBox, StakeCard, StakeDetailsPopup } from '@/components/index';
import { usePopups, useStakingData } from '@/hooks';
import { Stake } from '@/lib/types';

function StakingDashboardPage(): JSX.Element {
    const { openPopup } = usePopups();
    const { totalStakeConverted, totalRewardsConverted, stakingData } = useStakingData();

    const handleOpenPopup = (stake: Stake) => {
        openPopup(<StakeDetailsPopup {...stake} />);
    };

    return (
        <div className="flex flex-col items-center justify-center gap-4 pt-12">
            <AmountBox title="Currently staked" amount={`${totalStakeConverted}`} />
            <AmountBox title="Earned" amount={`${totalRewardsConverted}`} />
            <div className="flex flex-col items-center gap-4">
                <h1>List of stakes</h1>
                {stakingData?.map((stake) => (
                    <StakeCard key={stake.id} stake={stake} onDetailsClick={handleOpenPopup} />
                ))}
            </div>
        </div>
    );
}

export default StakingDashboardPage;
