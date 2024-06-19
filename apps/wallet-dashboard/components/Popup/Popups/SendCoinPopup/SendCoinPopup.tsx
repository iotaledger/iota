// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterValuesFormView, ReviewValuesFormView } from './views';
import { CoinStruct } from '@iota/iota.js/client';
import { useSendCoinTransaction } from '@/hooks';
import { useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';
import { useGetAllCoins } from '@iota/core';

export interface FormDataValues {
    amount: string;
    recipientAddress: string;
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
    });
    const { data: coins } = useGetAllCoins(coin.coinType, senderAddress);
    const totalCoins = coins?.reduce((partialSum, c) => partialSum + BigInt(c.balance), BigInt(0));

    const {
        mutateAsync: signAndExecuteTransactionBlock,
        error,
        isPending,
    } = useSignAndExecuteTransactionBlock();
    const { data: sendCoinData } = useSendCoinTransaction(
        coin,
        senderAddress,
        formData.recipientAddress,
        formData.amount,
        totalCoins === BigInt(formData.amount),
    );

    const handleTransfer = async () => {
        if (!sendCoinData?.transaction) return;
        signAndExecuteTransactionBlock({
            transactionBlock: sendCoinData.transaction,
        })
            .then(() => {
                onClose();
            })
            .catch(() => {}); // Avoid unhandled exceptions but handle error with hook
    };

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
                    gasBudget={sendCoinData?.gasBudget?.toString() || '--'}
                    setFormData={setFormData}
                />
            )}
            {step === FormStep.ReviewValues && (
                <ReviewValuesFormView
                    formData={formData}
                    handleBack={handleBack}
                    executeTransfer={handleTransfer}
                    senderAddress={senderAddress}
                    gasBudget={sendCoinData?.gasBudget?.toString() || '--'}
                    error={error?.message}
                    isPending={isPending}
                />
            )}
        </>
    );
}

export default SendCoinPopup;
