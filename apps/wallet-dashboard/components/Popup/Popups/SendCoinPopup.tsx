// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { Button } from '@/components';
import { CoinStruct } from '@mysten/sui.js/client';

interface SendCoinPopupProps {
    coin: CoinStruct;
    senderAddress: string;
    onClose: () => void;
}

enum Steps {
    EnterValues,
    ReviewValues,
}

function SendCoinPopup({
    coin: { balance, coinObjectId },
    senderAddress,
    onClose,
}: SendCoinPopupProps): JSX.Element {
    const [recipientAddress, setRecipientAddress] = useState<string>('');
    const [amount, setAmount] = useState<string>('');
    const [step, setStep] = useState<Steps>(Steps.EnterValues);

    function onSend(): void {
        console.log('Sending coins');
    }

    const handleNext = () => {
        setStep(Steps.ReviewValues);
    };

    const handleBack = () => {
        setStep(Steps.EnterValues);
    };

    return (
        <div>
            {step === Steps.EnterValues && (
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
                            onChange={(e) => setAmount(e.target.value)}
                            placeholder="Enter the amount to send coins"
                        />
                        <label htmlFor="address">Address: </label>
                        <input
                            type="text"
                            id="address"
                            value={recipientAddress}
                            onChange={(e) => setRecipientAddress(e.target.value)}
                            placeholder="Enter the address to send coins"
                        />
                    </div>
                    <div className="mt-4 flex justify-around">
                        <Button onClick={onClose}>Cancel</Button>
                        <Button onClick={handleNext}>Next</Button>
                    </div>
                </div>
            )}
            {step === Steps.ReviewValues && (
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
            )}
        </div>
    );
}

export default SendCoinPopup;
