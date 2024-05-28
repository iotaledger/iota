// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components/index';

interface StakeDetailsPopupProps {
    stake: {
        id: string;
        validator: string;
        stake: string;
        rewards: string;
    };
}

function StakeDetailsPopup({ stake }: StakeDetailsPopupProps): JSX.Element {
    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <p>{stake.validator}</p>
            <p>Stake: {stake.stake}</p>
            <p>Rewards: {stake.rewards}</p>
            <div className="flex justify-between gap-2">
                <Button onClick={() => console.log('Unstake')}>Unstake</Button>
                <Button onClick={() => console.log('Stake more')}>Stake more</Button>
            </div>
        </div>
    );
}

export default StakeDetailsPopup;
