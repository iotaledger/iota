// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button, Input } from '@/components';

interface EnterAmountViewProps {
    selectedValidator: string | null;
    amount: string;
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
    onBack: () => void;
    onStake: () => void;
    isStakeDisabled: boolean;
}

function EnterAmountView({
    selectedValidator,
    amount,
    onChange,
    onBack,
    onStake,
    isStakeDisabled,
}: EnterAmountViewProps): JSX.Element {
    return (
        <div className="flex flex-col items-start gap-2">
            <p>Selected Validator: {selectedValidator}</p>
            <Input
                label="Amount"
                value={amount}
                onChange={onChange}
                placeholder="Enter amount to stake"
            />
            <div className="flex w-full justify-between gap-2">
                <Button onClick={onBack}>Back</Button>
                <Button onClick={onStake} disabled={isStakeDisabled}>
                    Stake
                </Button>
            </div>
        </div>
    );
}

export default EnterAmountView;
