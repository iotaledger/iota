// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { AmountBox, Box, List, NewStakePopup, StakeDetailsPopup, Button } from '@/components';
import { usePopups } from '@/hooks';

function StakingDashboardPage(): JSX.Element {
    const { openPopup, closePopup } = usePopups();

    const HARCODED_STAKE_DATA = {
        title: 'Your Stake',
        value: '100 IOTA',
    };
    const HARCODED_REWARDS_DATA = {
        title: 'Earned',
        value: '0.297 IOTA',
    };
    const HARCODED_STAKING_LIST_TITLE = 'List of stakes';
    const HARCODED_STAKING_LIST = [
        { id: '0', validator: 'Validator 1', stake: '50 IOTA', rewards: '0.15 IOTA' },
        { id: '1', validator: 'Validator 2', stake: '30 IOTA', rewards: '0.09 IOTA' },
        { id: '2', validator: 'Validator 3', stake: '20 IOTA', rewards: '0.06 IOTA' },
    ];

    // Use `Stake` when https://github.com/iotaledger/iota/pull/459 gets merged
    // @ts-expect-error TODO improve typing here
    const viewStakeDetails = (stake) => {
        openPopup(<StakeDetailsPopup stake={stake} />);
    };

    const addNewStake = () => {
        openPopup(<NewStakePopup onClose={closePopup} />);
    };

    return (
        <div className="flex flex-col items-center justify-center gap-4 pt-12">
            <Button onClick={addNewStake}>New Stake</Button>
            <div className="flex items-center justify-center gap-4">
                {' '}
                <AmountBox title={HARCODED_STAKE_DATA.title} amount={HARCODED_STAKE_DATA.value} />
                <AmountBox
                    title={HARCODED_REWARDS_DATA.title}
                    amount={HARCODED_REWARDS_DATA.value}
                />
                <Box title={HARCODED_STAKING_LIST_TITLE}>
                    <List
                        data={HARCODED_STAKING_LIST}
                        onItemClick={viewStakeDetails}
                        actionText="View Details"
                    />
                </Box>
            </div>
        </div>
    );
}

export default StakingDashboardPage;
