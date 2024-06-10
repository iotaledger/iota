// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterValuesFormView, ReviewValuesFormView } from './views';
import { CoinStruct } from '@iota/iota.js/client';

export interface FormDataValues {
    amount: string;
    recipientAddress: string;
    senderAddress: string;
}

interface SendCoinPopupProps {
    coin: CoinStruct;
    senderAddress: string;
    onClose: () => void;
}

enum FormStep {
    EnterValues,
    ReviewValues,
}

function SendCoinPopup({ coin, senderAddress, onClose }: SendCoinPopupProps): JSX.Element {
    const [step, setStep] = useState<FormStep>(FormStep.EnterValues);
    const [formData, setFormData] = useState<FormDataValues>({
        amount: '',
        recipientAddress: '',
        senderAddress,
    });

    const handleNext = () => {
        setStep(FormStep.ReviewValues);
    };

    const handleBack = () => {
        setStep(FormStep.EnterValues);
    };

    return (
        <>
            {step === FormStep.EnterValues && (
                <EnterValuesFormView
                    coin={coin}
                    onClose={onClose}
                    handleNext={handleNext}
                    formData={formData}
                    setFormData={setFormData}
                />
            )}
            {step === FormStep.ReviewValues && (
                <ReviewValuesFormView formData={formData} handleBack={handleBack} />
            )}
        </>
    );
}

export default SendCoinPopup;
