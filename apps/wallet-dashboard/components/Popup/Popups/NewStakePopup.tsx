// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';

interface NewStakePopupProps {
    onClose: () => void;
}

enum Steps {
    SELECT_VALIDATOR,
    ENTER_AMOUNT,
}

const HARCODED_VALIDATORS = ['Validator 1', 'Validator 2', 'Validator 3'];

function NewStakePopup({ onClose }: NewStakePopupProps): JSX.Element {
    const [step, setStep] = useState<Steps>(Steps.SELECT_VALIDATOR);
    const [selectedValidator, setSelectedValidator] = useState<string | null>(null);
    const [amount, setAmount] = useState<string>('');

    const handleNext = () => {
        setStep(Steps.ENTER_AMOUNT);
    };

    const handleBack = () => {
        setStep(Steps.SELECT_VALIDATOR);
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
            {step === Steps.SELECT_VALIDATOR && (
                <div>
                    <h2>Select Validator</h2>
                    <div className="flex flex-col items-start gap-2">
                        {HARCODED_VALIDATORS.map((validator) => (
                            <button
                                key={validator}
                                onClick={() => handleValidatorSelect(validator)}
                            >
                                {validator}
                            </button>
                        ))}
                    </div>
                </div>
            )}
            {step === Steps.ENTER_AMOUNT && (
                <div className="flex flex-col items-start gap-2">
                    <p>Selected Validator: {selectedValidator}</p>
                    <h2>Enter Amount</h2>
                    <input
                        type="text"
                        value={amount}
                        onChange={(e) => setAmount(e.target.value)}
                        placeholder="Enter amount to stake"
                    />
                    <div className="flex w-full justify-between gap-2">
                        <button onClick={handleBack}>Back</button>
                        <button onClick={handleStake} disabled={!amount}>
                            Stake
                        </button>
                    </div>
                </div>
            )}
        </div>
    );
}

export default NewStakePopup;
