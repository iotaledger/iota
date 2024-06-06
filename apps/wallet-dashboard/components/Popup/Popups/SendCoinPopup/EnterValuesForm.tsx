// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinStruct } from '@mysten/sui.js/client';
import { FormDataValues } from './SendCoinPopup';
import Button from '@/components/Button';

interface EnterValuesProps {
    coin: CoinStruct;
    formData: FormDataValues;
    gasBudget: string;
    setFormData: React.Dispatch<React.SetStateAction<FormDataValues>>;
    onClose: () => void;
    handleNext: () => void;
}

function EnterValuesForm({
    coin,
    formData: { amount, recipientAddress },
    gasBudget,
    setFormData,
    onClose,
    handleNext,
}: EnterValuesProps): JSX.Element {
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
                <p>Coin: {coin.coinObjectId}</p>
                <p>Balance: {coin.balance}</p>
                <label htmlFor="amount">Coin amount to send: </label>
                <input
                    type="number"
                    id="amount"
                    name="amount"
                    min="1"
                    value={amount}
                    onChange={handleChange}
                    placeholder="Enter the amount to send coins"
                />
                <label htmlFor="address">Address: </label>
                <input
                    type="text"
                    id="recipientAddress"
                    name="recipientAddress"
                    value={recipientAddress}
                    onChange={handleChange}
                    placeholder="Enter the address to send coins"
                />
                <p>Gas fee: {gasBudget || '--'}</p>
            </div>
            <div className="mt-4 flex justify-around">
                <Button onClick={onClose}>Cancel</Button>
                <Button onClick={handleNext} disabled={!recipientAddress || !amount}>
                    Next
                </Button>
            </div>
        </div>
    );
}

export default EnterValuesForm;
