// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button, UnstakePopup } from '@/components';
import { usePopups } from '@/hooks';
import { ExtendedDelegatedStake } from '@iota/core';
import { useNotifications } from '@/hooks/useNotifications';

interface StakeDetailsPopupProps {
    extendedStake: ExtendedDelegatedStake;
    onClose: () => void;
}

function StakeDetailsPopup({ extendedStake, onClose }: StakeDetailsPopupProps): JSX.Element {
    const { openPopup, closePopup } = usePopups();
    const { addNotification } = useNotifications();

    function handleCloseUnstakePopup(): void {
        closePopup();
        onClose();
        addNotification('Unstake transaction has been sent');
    }

    function openUnstakePopup(): void {
        openPopup(
            <UnstakePopup extendedStake={extendedStake} closePopup={handleCloseUnstakePopup} />,
        );
    }

    return (
        <div className="flex min-w-[400px] flex-col gap-2">
            <p>Stake ID: {extendedStake.stakedIotaId}</p>
            <p>Validator: {extendedStake.validatorAddress}</p>
            <p>Stake: {extendedStake.principal}</p>
            <p>Stake Active Epoch: {extendedStake.stakeActiveEpoch}</p>
            <p>Stake Request Epoch: {extendedStake.stakeRequestEpoch}</p>
            {extendedStake.status === 'Active' && (
                <p>Estimated reward: {extendedStake.estimatedReward}</p>
            )}
            <p>Status: {extendedStake.status}</p>
            <div className="flex justify-between gap-2">
                <Button onClick={openUnstakePopup} disabled={extendedStake.status !== 'Active'}>
                    Unstake
                </Button>
                <Button onClick={() => console.log('Stake more')}>Stake more</Button>
            </div>
        </div>
    );
}

export default StakeDetailsPopup;
