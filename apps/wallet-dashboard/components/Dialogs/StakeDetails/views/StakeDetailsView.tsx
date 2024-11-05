// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components';
import { ExtendedDelegatedStake } from '@iota/core';

interface StakeDetailsPopupProps {
    extendedStake: ExtendedDelegatedStake;
    onUnstake: () => void;
}

export function StakeDetailsView({
    extendedStake,
    onUnstake,
}: StakeDetailsPopupProps): JSX.Element {
    return (
        <>
            <div className="flex w-full max-w-[336px] flex-1 flex-col">
                <div className="flex w-full max-w-full flex-1 flex-col gap-2 overflow-auto">
                    <p>Stake ID: {extendedStake.stakedIotaId}</p>
                    <p>Validator: {extendedStake.validatorAddress}</p>
                    <p>Stake: {extendedStake.principal}</p>
                    <p>Stake Active Epoch: {extendedStake.stakeActiveEpoch}</p>
                    <p>Stake Request Epoch: {extendedStake.stakeRequestEpoch}</p>
                    {extendedStake.status === 'Active' && (
                        <p>Estimated reward: {extendedStake.estimatedReward}</p>
                    )}
                    <p>Status: {extendedStake.status}</p>
                </div>
            </div>
            <div className="flex justify-between gap-2">
                <Button onClick={onUnstake} disabled={extendedStake.status !== 'Active'}>
                    Unstake
                </Button>
                <Button onClick={() => console.log('Stake more')}>Stake more</Button>
            </div>
        </>
    );
}
