// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterValuesFormView, ReviewValuesFormView, TransactionDetailsView } from './views';
import { CoinBalance } from '@iota/iota-sdk/client';
import { useSendCoinTransaction, useNotifications } from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';
import { CoinFormat, useFormatCoin, useGetAllCoins } from '@iota/core';
import { Dialog, DialogContent, DialogPosition } from '@iota/apps-ui-kit';
import { FormDataValues } from './interfaces';
import { INITIAL_VALUES } from './constants';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useTransferTransactionMutation } from '@/hooks';

interface SendCoinPopupProps {
    coin: CoinBalance;
    activeAddress: string;
    setOpen: (bool: boolean) => void;
    open: boolean;
}

enum FormStep {
    EnterValues,
    ReviewValues,
    TransactionDetails,
}

function SendTokenDialogBody({
    coin,
    activeAddress,
    setOpen,
}: SendCoinPopupProps): React.JSX.Element {
    const [step, setStep] = useState<FormStep>(FormStep.EnterValues);
    const [selectedCoin, setSelectedCoin] = useState<CoinBalance>(coin);
    const [formData, setFormData] = useState<FormDataValues>(INITIAL_VALUES);
    const [digest, setDigest] = useState<string>('');
    const [fullAmount] = useFormatCoin(formData.amount, selectedCoin.coinType, CoinFormat.FULL);
    const { data: coinsData } = useGetAllCoins(selectedCoin.coinType, activeAddress);

    const { addNotification } = useNotifications();
    const isPayAllIota =
        selectedCoin.totalBalance === formData.amount && selectedCoin.coinType === IOTA_TYPE_ARG;

    const { data: transaction } = useSendCoinTransaction(
        coinsData || [],
        selectedCoin.coinType,
        activeAddress,
        formData.to,
        formData.amount,
        isPayAllIota,
    );

    const {
        mutate: transfer,
        data,
        isPending: isLoadingTransfer,
    } = useTransferTransactionMutation();

    async function handleTransfer() {
        if (!transaction) {
            addNotification('There was an error with the transaction', NotificationType.Error);
            return;
        }

        transfer(transaction, {
            onSuccess: () => {
                setStep(FormStep.TransactionDetails);
                addNotification('Transfer transaction has been sent', NotificationType.Success);
            },
            onError: () => {
                setOpen(false);
                addNotification('Transfer transaction failed', NotificationType.Error);
            },
        });
    }

    function onNext(): void {
        setStep(FormStep.ReviewValues);
    }

    function onBack(): void {
        // The amount is formatted when submitting the enterValuesForm, so it is necessary to return to the previous value when backing out
        setFormData({
            ...formData,
            amount: fullAmount,
        });
        setStep(FormStep.EnterValues);
    }

    return (
        <>
            {step === FormStep.EnterValues && (
                <EnterValuesFormView
                    coin={selectedCoin}
                    activeAddress={activeAddress}
                    setSelectedCoin={setSelectedCoin}
                    onNext={onNext}
                    onClose={() => setOpen(false)}
                    setFormData={setFormData}
                    initialFormValues={formData}
                />
            )}
            {step === FormStep.ReviewValues && (
                <ReviewValuesFormView
                    formData={formData}
                    executeTransfer={handleTransfer}
                    senderAddress={activeAddress}
                    isPending={isLoadingTransfer}
                    coinType={selectedCoin.coinType}
                    isPayAllIota={isPayAllIota}
                    onClose={() => setOpen(false)}
                    onBack={onBack}
                />
            )}
            {step === FormStep.TransactionDetails && data?.digest && (
                <TransactionDetailsView
                    digest={data.digest}
                    onClose={() => {
                        setOpen(false);
                        setStep(FormStep.EnterValues);
                    }}
                />
            )}
        </>
    );
}

export function SendTokenDialog(props: SendCoinPopupProps) {
    return (
        <Dialog open={props.open} onOpenChange={props.setOpen}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <SendTokenDialogBody {...props} />
            </DialogContent>
        </Dialog>
    );
}
