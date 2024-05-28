// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { AmountBox, Box, List } from '@/components/index';

function StakingDashboardPage(): JSX.Element {
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
    return (
        <div className="flex items-center justify-center gap-4 pt-12">
            <AmountBox title={HARCODED_STAKE_DATA.title} amount={HARCODED_STAKE_DATA.value} />
            <AmountBox title={HARCODED_REWARDS_DATA.title} amount={HARCODED_REWARDS_DATA.value} />
            <Box title={HARCODED_STAKING_LIST_TITLE}>
                <List data={HARCODED_STAKING_LIST} />
            </Box>
        </div>
    );
}

export default StakingDashboardPage;
