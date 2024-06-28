// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterValuesFormView, ReviewValuesFormView } from './views';
import { CoinBalance } from '@iota/iota.js/client';
import { useSendCoinTransaction, useNotifications } from '@/hooks';
import { useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';
import { useGetAllCoins } from '@iota/core';
import { NotificationType } from '@/stores/notificationStore';

export interface FormDataValues {
    amount: string;
    recipientAddress: string;
}

interface SendCoinPopupProps {
    coin: CoinBalance;
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
    const { addNotification } = useNotifications();
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
                addNotification('Transfer transaction has been sent');
            })
            .catch(() => {
                addNotification('Transfer transaction was not sent', NotificationType.Error);
            });
    };

    const onNext = () => {
        setStep(FormStep.ReviewValues);
    };

    const onBack = () => {
        setStep(FormStep.EnterValues);
    };

    return (
        <>
            {step === FormStep.EnterValues && (
                <EnterValuesFormView
                    coin={coin}
                    onClose={onClose}
                    onNext={onNext}
                    formData={formData}
                    gasBudget={sendCoinData?.gasBudget?.toString() || '--'}
                    setFormData={setFormData}
                />
            )}
            {step === FormStep.ReviewValues && (
                <ReviewValuesFormView
                    formData={formData}
                    onBack={onBack}
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
