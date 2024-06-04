// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components/index';
import { usePopups } from '@/hooks';
import UnstakePopup from './UnstakePopup';

interface StakeDetailsPopupProps {
    stake: {
        id: string;
        validator: string;
        stake: string;
        rewards: string;
    };
}

function StakeDetailsPopup({ stake }: StakeDetailsPopupProps): JSX.Element {
    const { openPopup } = usePopups();

    const unstake = (id: string) => {
        console.log(`Unstake initiated for id: ${id}`);
    };

    const openUnstakePopup = () => {
        openPopup(<UnstakePopup stake={stake} onUnstake={unstake} />);
    };

    return (
        <div className="flex min-w-[400px] flex-col gap-2">
            <p>{stake.validator}</p>
            <p>Stake: {stake.stake}</p>
            <p>Rewards: {stake.rewards}</p>
            <div className="flex justify-between gap-2">
                <Button onClick={openUnstakePopup}>Unstake</Button>
                <Button onClick={() => console.log('Stake more')}>Stake more</Button>
            </div>
        </div>
    );
}

export default StakeDetailsPopup;
