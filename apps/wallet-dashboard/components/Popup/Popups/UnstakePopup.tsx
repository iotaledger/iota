// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components';

interface UnstakePopupProps {
    stake: {
        id: string;
        validator: string;
        stake: string;
        rewards: string;
    };
    onUnstake: (id: string) => void;
}

function UnstakePopup({
    stake: { id, validator, stake, rewards },
    onUnstake,
}: UnstakePopupProps): JSX.Element {
    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            <p>{validator}</p>
            <p>Stake: {stake}</p>
            <p>Rewards: {rewards}</p>
            <Button onClick={() => onUnstake(id)}>Confirm Unstake</Button>
        </div>
    );
}

export default UnstakePopup;
