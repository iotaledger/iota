// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { Button } from '@/components';
import { CoinStruct } from '@mysten/sui.js/client';

interface FormDataValues {
    amount: string;
    recipientAddress: string;
    senderAddress: string;
}

interface EnterValuesProps {
    coin: CoinStruct;
    formData: FormDataValues;
    setFormData: React.Dispatch<React.SetStateAction<FormDataValues>>;
    onClose: () => void;
    handleNext: () => void;
}

const EnterValuesForm = ({
    coin: { balance, coinObjectId },
    formData: { amount, recipientAddress },
    setFormData,
    onClose,
    handleNext,
}: EnterValuesProps) => {
    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const { name, value } = e.target;
        setFormData((prevFormData) => ({
            ...prevFormData,
            [name]: value,
        }));
    };

    return (
        <div className="flex flex-col gap-4">
            <h1 className="mb-4 text-center text-xl">Send coins</h1>
            <div className="flex flex-col gap-4">
                <p>Coin: {coinObjectId}</p>
                <p>Balance: {balance}</p>
                <label htmlFor="amount">Coin amount to send: </label>
                <input
                    type="number"
                    id="amount"
                    min="1"
                    value={amount}
                    onChange={handleChange}
                    placeholder="Enter the amount to send coins"
                />
                <label htmlFor="address">Address: </label>
                <input
                    type="text"
                    id="address"
                    value={recipientAddress}
                    onChange={handleChange}
                    placeholder="Enter the address to send coins"
                />
            </div>
            <div className="mt-4 flex justify-around">
                <Button onClick={onClose}>Cancel</Button>
                <Button onClick={handleNext} disabled={!recipientAddress || !amount}>
                    Next
                </Button>
            </div>
        </div>
    );
};

interface ReviewValuesProps {
    formData: FormDataValues;
    handleBack: () => void;
}

const ReviewValuesForm = ({
    formData: { amount, senderAddress, recipientAddress },
    handleBack,
}: ReviewValuesProps) => {
    function onSend(): void {
        console.log('Sending coins');
    }

    return (
        <div className="flex flex-col gap-4">
            <h1 className="mb-4 text-center text-xl">Review & Send</h1>
            <div className="flex flex-col gap-4">
                <p>Sending: {amount}</p>
                <p>From: {senderAddress}</p>
                <p>To: {recipientAddress}</p>
            </div>
            <div className="mt-4 flex justify-around">
                <Button onClick={handleBack}>Back</Button>
                <Button onClick={onSend}>Send now</Button>
            </div>
        </div>
    );
};

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
