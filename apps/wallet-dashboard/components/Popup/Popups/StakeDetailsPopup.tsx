// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components/index';
import { Stake } from '@/lib/types';

function StakeDetailsPopup(stake: Stake): JSX.Element {
    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <p>{stake.validator}</p>
            <p>Stake: {stake.stake}</p>
            <p>Rewards: {stake.rewards}</p>
            <p>Stake Active Epoch: {stake.stakeActiveEpoch}</p>
            <p>Stake Request Epoch: {stake.stakeRequestEpoch}</p>
            {stake.status === 'Active' && <p>Estimated reward: {stake.estimatedReward}</p>}
            <p>Status: {stake.status}</p>
            <div className="flex justify-between gap-2">
                <Button onClick={() => console.log('Unstake')}>Unstake</Button>
                <Button onClick={() => console.log('Stake more')}>Stake more</Button>
            </div>
        </div>
    );
}

export default StakeDetailsPopup;
