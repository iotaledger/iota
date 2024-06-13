// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { AmountBox, Box, StakeCard, NewStakePopup, StakeDetailsPopup, Button } from '@/components';
import { usePopups, useDelegatedStake } from '@/hooks';
import { Stake } from '@/lib/types';

function StakingDashboardPage(): JSX.Element {
    const { openPopup, closePopup } = usePopups();
    const { totalStake, totalRewards, delegatedStake } = useDelegatedStake();

    const viewStakeDetails = (stake: Stake) => {
        openPopup(<StakeDetailsPopup stake={stake} />);
    };

    const addNewStake = () => {
        openPopup(<NewStakePopup onClose={closePopup} />);
    };

    return (
        <div className="flex flex-col items-center justify-center gap-4 pt-12">
            <AmountBox title="Currently staked" amount={`${totalStake}`} />
            <AmountBox title="Earned" amount={`${totalRewards}`} />
            <Box title="Stakes">
                <div className="flex flex-col items-center gap-4">
                    <h1>List of stakes</h1>
                    {delegatedStake?.map((stake) => (
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
