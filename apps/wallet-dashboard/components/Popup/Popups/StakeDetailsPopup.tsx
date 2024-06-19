// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components/index';
import { usePopups } from '@/hooks';
import UnstakePopup from './UnstakePopup';
import { ExtendedDelegatedStake } from '@iota/core';

interface StakeDetailsPopupProps {
    stake: ExtendedDelegatedStake;
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
            <p>{stake.validatorAddress}</p>
            <p>Stake: {stake.principal}</p>
            <p>Stake Active Epoch: {stake.stakeActiveEpoch}</p>
            <p>Stake Request Epoch: {stake.stakeRequestEpoch}</p>
            {stake.status === 'Active' && <p>Estimated reward: {stake.estimatedReward}</p>}
            <p>Status: {stake.status}</p>
            <div className="flex justify-between gap-2">
                <Button onClick={openUnstakePopup}>Unstake</Button>
                <Button onClick={() => console.log('Stake more')}>Stake more</Button>
            </div>
        </div>
    );
}

export default StakeDetailsPopup;
