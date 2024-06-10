// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterValuesForm, ReviewValuesForm } from './';
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

enum Steps {
    EnterValues,
    ReviewValues,
}

function SendCoinPopup({ coin, senderAddress, onClose }: SendCoinPopupProps): JSX.Element {
    const [step, setStep] = useState<Steps>(Steps.EnterValues);
    const [formData, setFormData] = useState<FormDataValues>({
        amount: '',
        recipientAddress: '',
        senderAddress,
    });

    const handleNext = () => {
        setStep(Steps.ReviewValues);
    };

    const handleBack = () => {
        setStep(Steps.EnterValues);
    };

    return (
        <>
            {step === Steps.EnterValues && (
                <EnterValuesForm
                    coin={coin}
                    onClose={onClose}
                    handleNext={handleNext}
                    formData={formData}
                    setFormData={setFormData}
                />
            )}
            {step === Steps.ReviewValues && (
                <ReviewValuesForm formData={formData} handleBack={handleBack} />
            )}
        </>
    );
}

export default SendCoinPopup;
