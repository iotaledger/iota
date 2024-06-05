// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinStruct } from '@mysten/sui.js/client';
import { FormDataValues } from './SendCoinPopup';
import Button from '@/components/Button';
import { useSuiClient } from '@mysten/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { createTokenTransferTransaction } from '@/lib/utils';
import { SUI_TYPE_ARG } from '@mysten/sui.js/utils';
import { useEffect } from 'react';
import { COIN_DECIMALS } from '@/lib/constants';

interface EnterValuesProps {
    coin: CoinStruct;
    formData: FormDataValues;
    setFormData: React.Dispatch<React.SetStateAction<FormDataValues>>;
    onClose: () => void;
    handleNext: () => void;
}

function EnterValuesForm({
    coin,
    formData: { amount, recipientAddress, senderAddress, gasBudget },
    setFormData,
    onClose,
    handleNext,
}: EnterValuesProps): JSX.Element {
    const client = useSuiClient();
    const { data: _gasBudget } = useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'transaction-gas-budget-estimate',
            {
                recipientAddress,
                amount,
                coin,
                senderAddress,
                coinDecimals: COIN_DECIMALS,
            },
        ],
        queryFn: async () => {
            if (!amount || !recipientAddress || !coin || !senderAddress) {
                return null;
            }

            const tx = createTokenTransferTransaction({
                recipientAddress,
                amount,
                coinType: SUI_TYPE_ARG,
                coinDecimals: COIN_DECIMALS,
                coins: [coin],
            });

            tx.setSender(senderAddress);
            await tx.build({ client });
            return tx.blockData.gasConfig.budget;
        },
    });

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const { name, value } = e.target;
        setFormData((prevFormData) => ({
            ...prevFormData,
            [name]: value,
        }));
    };

    useEffect(() => {
        setFormData((prevFormData) => ({
            ...prevFormData,
            gasBudget: _gasBudget?.toString() || '',
        }));
    }, [_gasBudget, setFormData, amount]);

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
                {gasBudget ? (
                    <div className="my-2 flex w-full justify-between gap-2 px-2">
                        <label htmlFor="address">Estimated Gas Fees: </label>
                        <input
                            type="text"
                            id="gasBudget"
                            name="gasBudget"
                            value={gasBudget}
                            onChange={handleChange}
                        />
                    </div>
                ) : null}
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
