// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterAmountView, SelectValidatorView } from './views';

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
                <SelectValidatorView
                    validators={HARDCODED_VALIDATORS}
                    onSelect={handleValidatorSelect}
                />
            )}
            {step === Steps.EnterAmount && (
                <EnterAmountView
                    selectedValidator={selectedValidator}
                    amount={amount}
                    onChange={(e) => setAmount(e.target.value)}
                    onBack={handleBack}
                    onStake={handleStake}
                    isStakeDisabled={!amount}
                />
            )}
        </div>
    );
}

export default NewStakePopup;
