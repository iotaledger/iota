// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components';
import { ExtendedDelegatedStake } from '@iota/core';

interface UnstakePopupProps {
    stake: ExtendedDelegatedStake;
    onUnstake: (id: string) => void;
}

function UnstakePopup({ stake, onUnstake }: UnstakePopupProps): JSX.Element {
    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <p>{stake.validatorAddress}</p>
            <p>Stake: {stake.principal}</p>
            {stake.status === 'Active' && <p>Estimated reward: {stake.estimatedReward}</p>}
            <Button onClick={() => onUnstake(stake.stakedIotaId)}>Confirm Unstake</Button>
        </div>
    );
}

export default UnstakePopup;
