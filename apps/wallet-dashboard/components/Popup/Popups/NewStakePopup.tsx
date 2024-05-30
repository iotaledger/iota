// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { Button, Input } from '@/components';

interface NewStakePopupProps {
    onClose: () => void;
}

enum Steps {
    SelectValidator,
    EnterAmount,
}

const HARDCODED_VALIDATORS = ['Validator 1', 'Validator 2', 'Validator 3'];

function NewStakePopup({ onClose }: NewStakePopupProps): JSX.Element {
    const [step, setStep] = useState<Steps>(Steps.SelectValidator);
    const [selectedValidator, setSelectedValidator] = useState<string | null>(null);
    const [amount, setAmount] = useState<string>('');

    const handleNext = () => {
        setStep(Steps.EnterAmount);
    };

    const handleBack = () => {
        setStep(Steps.SelectValidator);
    };

    const handleValidatorSelect = (validator: string) => {
        setSelectedValidator(validator);
        handleNext();
    };

    const handleStake = () => {
        console.log(`Staking ${amount} with ${selectedValidator}`);
        onClose();
    };

    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            {step === Steps.SelectValidator && (
                <div>
                    <h2>Select Validator</h2>
                    <div className="flex flex-col items-start gap-2">
                        {HARDCODED_VALIDATORS.map((validator) => (
                            <Button
                                key={validator}
                                onClick={() => handleValidatorSelect(validator)}
                            >
                                {validator}
                            </Button>
                        ))}
                    </div>
                </div>
            )}
            {step === Steps.EnterAmount && (
                <div className="flex flex-col items-start gap-2">
                    <p>Selected Validator: {selectedValidator}</p>
                    <h2>Enter Amount</h2>
                    <Input
                        value={amount}
                        onChange={(e) => setAmount(e.target.value)}
                        placeholder="Enter amount to stake"
                    />
                    <div className="flex w-full justify-between gap-2">
                        <Button onClick={handleBack}>Back</Button>
                        <Button onClick={handleStake} disabled={!amount}>
                            Stake
                        </Button>
                    </div>
                </div>
            )}
        </div>
    );
}

export default NewStakePopup;
