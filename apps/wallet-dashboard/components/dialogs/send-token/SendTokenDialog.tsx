// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { EnterValuesFormView, ReviewValuesFormView, TransactionDetailsView } from './views';
import { CoinBalance } from '@iota/iota-sdk/client';
import { useGetAllCoins, useSendCoinTransaction } from '@iota/core';
import { Dialog, DialogContent, DialogPosition } from '@iota/apps-ui-kit';
import { FormDataValues } from './interfaces';
import { INITIAL_VALUES } from './constants';
import { useTransferTransactionMutation } from '@/hooks';
import toast from 'react-hot-toast';
import { ampli } from '@/lib/utils/analytics';
import { useQueryClient } from '@tanstack/react-query';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface SendCoinDialogProps {
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
}: SendCoinDialogProps): React.JSX.Element {
    const [step, setStep] = useState<FormStep>(FormStep.EnterValues);
    const [selectedCoin, setSelectedCoin] = useState<CoinBalance>(coin);
    const [formData, setFormData] = useState<FormDataValues>(INITIAL_VALUES);
    const { data: coinsData } = useGetAllCoins(selectedCoin.coinType, activeAddress);
    const queryClient = useQueryClient();

    const isPayAllIota =
        selectedCoin.totalBalance === formData.amount && selectedCoin.coinType === IOTA_TYPE_ARG;

    const { data: transactionData } = useSendCoinTransaction({
        coins: coinsData || [],
        coinType: selectedCoin.coinType,
        senderAddress: activeAddress,
        recipientAddress: formData.to,
        amount: formData.amount,
    });

    const {
        mutate: transfer,
        data,
        isPending: isLoadingTransfer,
    } = useTransferTransactionMutation();

    async function handleTransfer() {
        if (!transactionData?.transaction) {
            toast.error('There was an error with the transaction');
            return;
        }

        transfer(transactionData.transaction, {
            onSuccess: () => {
                queryClient.invalidateQueries({ queryKey: [activeAddress] });

                setStep(FormStep.TransactionDetails);
                toast.success('Transfer transaction has been sent');
                ampli.sentCoins({
                    coinType: selectedCoin.coinType,
                });
            },
            onError: (error) => {
                setOpen(false);
                toast.error(error?.message ?? 'Transfer transaction failed');
            },
        });
    }

    function onNext(): void {
        setStep(FormStep.ReviewValues);
    }

    function onBack(): void {
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

export function SendTokenDialog(props: SendCoinDialogProps) {
    return (
        <Dialog open={props.open} onOpenChange={props.setOpen}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <SendTokenDialogBody {...props} />
            </DialogContent>
        </Dialog>
    );
}
