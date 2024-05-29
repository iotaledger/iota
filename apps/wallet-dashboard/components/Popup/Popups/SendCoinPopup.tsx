// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { Button } from '@/components';
import { CoinStruct } from '@mysten/sui.js/client';

interface SendCoinPopupProps {
    coin: CoinStruct;
    onClose: () => void;
}

function SendCoinPopup({
    coin: { balance, coinObjectId },
    onClose,
}: SendCoinPopupProps): JSX.Element {
    const [recipientAddress, setRecipientAddress] = useState<string>('');
    const [amount, setAmount] = useState<string>('');

    function onReview(): void {
        console.log('Review coins');
    }

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
                <Button onClick={onReview}>Review</Button>
            </div>
        </div>
    );
}

export default SendCoinPopup;
