// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components/index';
import { usePopups } from '@/hooks';
import UnstakePopup from './UnstakePopup';
import { ExtendedDelegatedStake } from '@iota/core';
import { useNotifications } from '@/hooks/useNotifications';

interface StakeDetailsPopupProps {
    stake: ExtendedDelegatedStake;
    onClose: () => void;
}

function StakeDetailsPopup({ stake, onClose }: StakeDetailsPopupProps): JSX.Element {
    const { openPopup, closePopup } = usePopups();
    const { addNotification } = useNotifications();

    const handleCloseUnstakePopup = () => {
        closePopup();
        onClose();
        addNotification('Unstake transaction has been sent');
    };

    const openUnstakePopup = () => {
        openPopup(<UnstakePopup stake={stake} closePopup={handleCloseUnstakePopup} />);
    };

    return (
        <div className="flex min-w-[400px] flex-col gap-2">
            <p>Stake ID: {stake.stakedIotaId}</p>
            <p>Validator: {stake.validatorAddress}</p>
            <p>Stake: {stake.principal}</p>
            <p>Stake Active Epoch: {stake.stakeActiveEpoch}</p>
            <p>Stake Request Epoch: {stake.stakeRequestEpoch}</p>
            {stake.status === 'Active' && <p>Estimated reward: {stake.estimatedReward}</p>}
            <p>Status: {stake.status}</p>
            <div className="flex justify-between gap-2">
                <Button onClick={openUnstakePopup} disabled={stake.status !== 'Active'}>
                    Unstake
                </Button>
                <Button onClick={() => console.log('Stake more')}>Stake more</Button>
            </div>
        </div>
    );
}

export default StakeDetailsPopup;
