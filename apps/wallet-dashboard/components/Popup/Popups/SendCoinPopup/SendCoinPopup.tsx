// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterValuesForm, ReviewValuesForm } from './';
import { CoinStruct } from '@mysten/sui.js/client';
import { useSignAndExecuteTransactionBlock, useSuiClient } from '@mysten/dapp-kit';
import { useCoinMetadata } from '@mysten/core';
import { useQuery } from '@tanstack/react-query';
import { createTokenTransferTransaction } from '@/lib/utils';
import { COIN_DECIMALS } from '@/lib/constants';

export interface FormDataValues {
    amount: string;
    recipientAddress: string;
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
    const client = useSuiClient();
    const [step, setStep] = useState<Steps>(Steps.EnterValues);
    const [isPending, setIsPending] = useState<boolean>(false);
    const [formData, setFormData] = useState<FormDataValues>({
        amount: '',
        recipientAddress: '',
    });
    const { data: coinMetadata } = useCoinMetadata();
    const { mutateAsync: signAndExecuteTransactionBlock, error } =
        useSignAndExecuteTransactionBlock();

    const { data: transaction } = useQuery({
        queryKey: [
            'token-transfer-transaction',
            formData.recipientAddress,
            formData.amount,
            coin,
            coinMetadata?.decimals,
            senderAddress,
            client,
        ],
        queryFn: async () => {
            const transaction = createTokenTransferTransaction({
                coinType: coin.coinType,
                coinDecimals: coinMetadata?.decimals || COIN_DECIMALS,
                recipientAddress: formData.recipientAddress,
                amount: formData.amount,
                coins: [coin],
            });

            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!formData.recipientAddress && !!formData.amount && !!coin && !!senderAddress,
        gcTime: 0,
    });

    const gasBudget = transaction?.blockData.gasConfig.budget?.toString() || '';

    async function executeTransfer() {
        if (!transaction) return;
        setIsPending(true);
        const response = await signAndExecuteTransactionBlock({
            transactionBlock: transaction,
        });
        setIsPending(false);

        if (response) {
            onClose();
        }
    }

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
                    gasBudget={gasBudget}
                    setFormData={setFormData}
                />
            )}
            {step === Steps.ReviewValues && (
                <ReviewValuesForm
                    formData={formData}
                    handleBack={handleBack}
                    executeTransfer={executeTransfer}
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
