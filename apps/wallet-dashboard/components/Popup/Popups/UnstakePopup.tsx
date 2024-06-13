// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components';
import { Stake } from '@/lib/types';

interface UnstakePopupProps {
    stake: Stake;
    onUnstake: (id: string) => void;
}

function UnstakePopup({ stake, onUnstake }: UnstakePopupProps): JSX.Element {
    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <p>{stake.validator}</p>
            <p>Stake: {stake.stake}</p>
            {stake.status === 'Active' && <p>Estimated reward: {stake.estimatedReward}</p>}
            <Button onClick={() => onUnstake(stake.id)}>Confirm Unstake</Button>
        </div>
    );
}

export default UnstakePopup;
