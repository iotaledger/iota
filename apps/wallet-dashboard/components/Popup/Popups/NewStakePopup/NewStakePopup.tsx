// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterAmountView, SelectValidatorView } from './views';

interface NewStakePopupProps {
    onClose: () => void;
}

enum Step {
    SelectValidator,
    EnterAmount,
}

const HARDCODED_VALIDATORS = ['Validator 1', 'Validator 2', 'Validator 3'];

function NewStakePopup({ onClose }: NewStakePopupProps): JSX.Element {
    const [step, setStep] = useState<Step>(Step.SelectValidator);
    const [selectedValidator, setSelectedValidator] = useState<string | null>(null);
    const [amount, setAmount] = useState<string>('');

    const handleNext = () => {
        setStep(Step.EnterAmount);
    };

    const handleBack = () => {
        setStep(Step.SelectValidator);
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
            {step === Step.SelectValidator && (
                <SelectValidatorView
                    validators={HARDCODED_VALIDATORS}
                    onSelect={handleValidatorSelect}
                />
            )}
            {step === Step.EnterAmount && (
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
