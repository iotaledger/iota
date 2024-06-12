// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterValuesFormView, ReviewValuesFormView } from './views';
import { CoinStruct } from '@iota/iota.js/client';
import { useSendCoinTransaction } from '@/hooks';

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
    const { gasBudget, executeTransfer, error, isPending } = useSendCoinTransaction(
        coin,
        formData.senderAddress,
        formData.recipientAddress,
        formData.amount,
    );

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
                    gasBudget={gasBudget}
                    setFormData={setFormData}
                />
            )}
            {step === FormStep.ReviewValues && (
                <ReviewValuesFormView
                    formData={formData}
                    handleBack={handleBack}
                    executeTransfer={() => executeTransfer(onClose)}
                    senderAddress={senderAddress}
                    gasBudget={gasBudget}
                    error={error?.message}
                    isPending={isPending}
                />
            )}
        </>
    );
}

export default SendCoinPopup;
