// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterValuesFormView, ReviewValuesFormView } from './views';
import { CoinBalance } from '@iota/iota-sdk/client';
import { useSendCoinTransaction, useNotifications } from '@/hooks';
import { useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';
import { NotificationType } from '@/stores/notificationStore';
import { Dropdown } from '@/components';
import { useGetAllCoins } from '@iota/core';

export interface FormDataValues {
    amount: string;
    recipientAddress: string;
}

interface SendCoinPopupProps {
    coin: CoinBalance;
    senderAddress: string;
    onClose: () => void;
    coins: CoinBalance[];
}

enum FormStep {
    EnterValues,
    ReviewValues,
}

function SendCoinPopup({
    coin,
    senderAddress,
    onClose,
    coins,
}: SendCoinPopupProps): React.JSX.Element {
    const [step, setStep] = useState<FormStep>(FormStep.EnterValues);
    const [selectedCoin, setCoin] = useState<CoinBalance>(coin);
    const [formData, setFormData] = useState<FormDataValues>({
        amount: '',
        recipientAddress: '',
    });
    const { addNotification } = useNotifications();

    const { data: coinsData } = useGetAllCoins(selectedCoin.coinType, senderAddress);

    const {
        mutateAsync: signAndExecuteTransactionBlock,
        error,
        isPending,
    } = useSignAndExecuteTransactionBlock();
    const { data: sendCoinData } = useSendCoinTransaction(
        coinsData || [],
        selectedCoin.coinType,
        senderAddress,
        formData.recipientAddress,
        formData.amount,
        selectedCoin.totalBalance === formData.amount,
    );

    function handleTransfer() {
        if (!sendCoinData?.transaction) {
            addNotification('There was an error with the transaction', NotificationType.Error);
            return;
        } else {
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
        }
    }

    function onNext(): void {
        setStep(FormStep.ReviewValues);
    }

    function onBack(): void {
        setStep(FormStep.EnterValues);
    }

    function handleSelectedCoin(coin: CoinBalance): void {
        setCoin(coin);
        setFormData({
            amount: '',
            recipientAddress: '',
        });
    }

    return (
        <>
            <Dropdown
                options={coins}
                selectedOption={selectedCoin}
                onChange={handleSelectedCoin}
                placeholder="Select a coin to send"
                disabled={step !== FormStep.EnterValues}
                getOptionId={(_selectedCoin) => _selectedCoin.coinType}
            />
            {step === FormStep.EnterValues && (
                <EnterValuesFormView
                    coin={selectedCoin}
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
