// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterValuesFormView, ReviewValuesFormView, SentSuccessView } from './views';
import { CoinBalance } from '@iota/iota-sdk/client';
import { useSendCoinTransaction, useNotifications } from '@/hooks';
import { useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { NotificationType } from '@/stores/notificationStore';
import { CoinFormat, useFormatCoin, useGetAllCoins } from '@iota/core';
import { Dialog, DialogContent, DialogPosition } from '@iota/apps-ui-kit';
import { FormDataValues } from './interfaces';
import { INITIAL_VALUES } from './constants';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface SendCoinPopupProps {
    coin: CoinBalance;
    activeAddress: string;
    setOpen: (bool: boolean) => void;
    open: boolean;
}

enum FormStep {
    EnterValues,
    ReviewValues,
    SentSuccess,
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
    const { addNotification } = useNotifications();
    const iotaClient = useIotaClient();
    const [isReviewing, setIsReviewing] = useState(false);
    const { data: coinsData } = useGetAllCoins(selectedCoin.coinType, activeAddress);

    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
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

    function handleTransfer() {
        if (!transaction) {
            addNotification('There was an error with the transaction', NotificationType.Error);
            return;
        } else {
            setIsReviewing(true);
            signAndExecuteTransaction({
                transaction,
            })
                .then((d) =>
                    iotaClient.waitForTransaction({
                        digest: d.digest,
                    }),
                )
                .then((transaction) => {
                    setDigest(transaction.digest);
                    setStep(FormStep.SentSuccess);
                    addNotification('Transfer transaction has been sent');
                })
                .catch(handleTransactionError)
                .finally(() => setIsReviewing(false));
        }
    }

    function handleTransactionError() {
        setOpen(false);
        addNotification('There was an error with the transaction', NotificationType.Error);
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
                    isPending={isReviewing}
                    coinType={selectedCoin.coinType}
                    isPayAllIota={isPayAllIota}
                    onClose={() => setOpen(false)}
                    onBack={onBack}
                />
            )}
            {step === FormStep.SentSuccess && (
                <SentSuccessView
                    digest={digest}
                    onClose={() => {
                        setOpen(false);
                        setStep(FormStep.EnterValues);
                    }}
                />
            )}
        </>
    );
}

export function SendTokenDialog(props: SendCoinPopupProps): React.JSX.Element {
    return (
        <Dialog open={props.open} onOpenChange={props.setOpen}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <SendTokenDialogBody {...props} />
            </DialogContent>
        </Dialog>
    );
}
