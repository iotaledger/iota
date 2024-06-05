// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMutation, useQueryClient } from '@tanstack/react-query';
import { FormDataValues } from './SendCoinPopup';
import Button from '@/components/Button';
import { useEffect, useMemo, useState } from 'react';
import { CoinMetadata, CoinStruct } from '@mysten/sui.js/client';
import { COIN_DECIMALS } from '@/lib/constants';
import { createTokenTransferTransaction } from '@/lib/utils';
import { useSuiClient, useSignAndExecuteTransactionBlock } from '@mysten/dapp-kit';

interface ReviewValuesProps {
    formData: FormDataValues;
    coin: CoinStruct;
    handleBack: () => void;
}

function ReviewValuesForm({
    formData: { amount, senderAddress, recipientAddress },
    coin,
    handleBack,
}: ReviewValuesProps): JSX.Element {
    const coinType = coin.coinType;
    const { getCoinMetadata } = useSuiClient();
    const { mutateAsync: signAndExecuteTransactionBlock } = useSignAndExecuteTransactionBlock();
    const queryClient = useQueryClient();
    const [coinMetadata, setCoinMetadata] = useState<CoinMetadata | null>(null);
    const [error, setError] = useState<string | null>(null);

    const transaction = useMemo(() => {
        if (!coinType || !amount || !recipientAddress) return null;

        return createTokenTransferTransaction({
            coinType,
            coinDecimals: COIN_DECIMALS,
            recipientAddress,
            amount,
            coins: [coin],
        });
    }, [amount, coinType, recipientAddress, coinMetadata?.decimals]);

    const executeTransfer = useMutation({
        mutationFn: async () => {
            if (!transaction) {
                throw new Error('Missing data');
            }
            setError(null);

            return signAndExecuteTransactionBlock({
                transactionBlock: transaction,
                options: {
                    showInput: true,
                    showEffects: true,
                    showEvents: true,
                },
            });
        },
        onSuccess: (response) => {
            queryClient.invalidateQueries({ queryKey: ['get-coins'] });
            queryClient.invalidateQueries({ queryKey: ['coin-balance'] });

            const receiptUrl = `/receipt?txdigest=${encodeURIComponent(
                response.digest,
            )}&from=transactions`;
            console.log('receiptUrl', receiptUrl);

            return;
            // return navigate(receiptUrl);
        },
        onError: (error) => {
            console.log('error', error);
            setError(error.message);
        },
    });

    useEffect(() => {
        async () => {
            const _coinMetadata = await getCoinMetadata({ coinType: coin.coinType });
            console.log('_coinMetadata', _coinMetadata);

            setCoinMetadata(_coinMetadata);
        };
    }, [coin]);

    return (
        <div className="flex flex-col gap-4">
            <h1 className="mb-4 text-center text-xl">Review & Send</h1>
            <div className="flex flex-col gap-4">
                <p>Sending: {amount}</p>
                <p>From: {senderAddress}</p>
                <p>To: {recipientAddress}</p>
            </div>
            {error ? <span className="text-red-700">{error}</span> : null}
            <div className="mt-4 flex justify-around">
                <Button onClick={handleBack}>Back</Button>
                <Button onClick={() => executeTransfer.mutateAsync()}>Send now</Button>
            </div>
        </div>
    );
}
export default ReviewValuesForm;
