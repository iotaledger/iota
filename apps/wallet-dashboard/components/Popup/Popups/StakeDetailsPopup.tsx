// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components/index';
import { Stake } from '@/lib/interfaces';

function StakeDetailsPopup({
    validator,
    stake,
    rewards,
    stakeActiveEpoch,
    stakeRequestEpoch,
    status,
}: Stake): JSX.Element {
    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <p>{validator}</p>
            <p>Stake: {stake}</p>
            <p>Rewards: {rewards}</p>
            <p>Stake Active Epoch: {stakeActiveEpoch}</p>
            <p>Stake Request Epoch: {stakeRequestEpoch}</p>
            <p>Status: {status}</p>
            <div className="flex justify-between gap-2">
                <Button onClick={() => console.log('Unstake')}>Unstake</Button>
                <Button onClick={() => console.log('Stake more')}>Stake more</Button>
            </div>
        </div>
    );
}

export default StakeDetailsPopup;
