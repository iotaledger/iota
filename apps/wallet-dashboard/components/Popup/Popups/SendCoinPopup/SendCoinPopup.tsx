// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { useSignAndExecuteTransactionBlock, useIotaClient } from '@iota/dapp-kit';
import { useCoinMetadata } from '@iota/core';
import { useQuery } from '@tanstack/react-query';
import { createTokenTransferTransaction } from '@/lib/utils';
import { COIN_DECIMALS } from '@/lib/constants';
import { EnterValuesFormView, ReviewValuesFormView } from './views';
import { CoinStruct } from '@iota/iota.js/client';

export interface FormDataValues {
    amount: string;
    recipientAddress: string;
}

interface SendCoinPopupProps {
    coin: CoinStruct;
    senderAddress: string;
    onClose: () => void;
}

enum FormSteps {
    EnterValues,
    ReviewValues,
}

function SendCoinPopup({ coin, senderAddress, onClose }: SendCoinPopupProps): JSX.Element {
    const client = useIotaClient();
    const [step, setStep] = useState<FormSteps>(FormSteps.EnterValues);
    const [formData, setFormData] = useState<FormDataValues>({
        amount: '',
        recipientAddress: '',
    });
    const { data: coinMetadata } = useCoinMetadata();
    const {
        mutateAsync: signAndExecuteTransactionBlock,
        error,
        isPending,
    } = useSignAndExecuteTransactionBlock();
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

        const response = await signAndExecuteTransactionBlock({
            transactionBlock: transaction,
        });

        if (response) {
            onClose();
        }
    }

    const handleNext = () => {
        setStep(FormSteps.ReviewValues);
    };

    const handleBack = () => {
        setStep(FormSteps.EnterValues);
    };

    return (
        <>
            {step === FormSteps.EnterValues && (
                <EnterValuesFormView
                    coin={coin}
                    onClose={onClose}
                    handleNext={handleNext}
                    formData={formData}
                    gasBudget={gasBudget}
                    setFormData={setFormData}
                />
            )}
            {step === FormSteps.ReviewValues && (
                <ReviewValuesFormView
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
