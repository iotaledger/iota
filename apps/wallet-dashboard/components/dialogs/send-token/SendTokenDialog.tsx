// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState } from 'react';
import { EnterValuesFormView, ReviewValuesFormView, TransactionDetailsView } from './views';
import { CoinBalance, getNetwork } from '@iota/iota-sdk/client';
import {
    useGetAllCoins,
    useSendCoinTransaction,
    toast,
    useCoinMetadata,
    createValidationSchemaSendTokenForm,
    sumCoinBalances,
    SendTokenFormValues,
    useFeatureEnabledByNetwork,
    Feature,
} from '@iota/core';
import { Dialog, DialogContent, DialogPosition } from '@iota/apps-ui-kit';
import { INITIAL_VALUES } from './constants';
import { useTransferTransactionMutation } from '@/hooks';
import { ampli } from '@/lib/utils/analytics';
import { useQueryClient } from '@tanstack/react-query';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { FormikProvider, useFormik } from 'formik';
import { shouldResolveInputAsName } from '@iota/core/utils/validation/names';
import { useNetwork } from '@iota/core/src/hooks/useNetwork';

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
    const { data: coins = [], isLoading: isLoadingCoins } = useGetAllCoins(
        selectedCoin.coinType,
        activeAddress,
    );
    const { data: iotaCoins = [], isLoading: isLoadingIotaCoins } = useGetAllCoins(
        IOTA_TYPE_ARG,
        activeAddress,
    );
    const queryClient = useQueryClient();

    const coinBalance = sumCoinBalances(coins);
    const iotaBalance = sumCoinBalances(iotaCoins);
    const selectedCoinMetadata = useCoinMetadata(selectedCoin.coinType);
    const coinDecimals = selectedCoinMetadata.data?.decimals ?? 0;
    const coinSymbol = selectedCoinMetadata.data?.symbol ?? '';

    const networkId = useNetwork();
    const network = getNetwork(networkId).id;

    const isFeatureEnabled = useFeatureEnabledByNetwork(Feature.IotaNames, network);

    const validationSchemaStepOne = useMemo(
        () =>
            createValidationSchemaSendTokenForm(
                isFeatureEnabled,
                coinBalance,
                coinSymbol,
                coinDecimals,
            ),
        [isFeatureEnabled, coinBalance, coinSymbol, coinDecimals],
    );

    const formik = useFormik<SendTokenFormValues>({
        initialValues: INITIAL_VALUES,
        validationSchema: validationSchemaStepOne,
        onSubmit: () => {},
        validateOnChange: false,
        validateOnBlur: false,
    });

    const isNameInput = shouldResolveInputAsName(formik.values.to);

    const sendCoinQuery = useSendCoinTransaction({
        coins,
        coinType: selectedCoin.coinType,
        senderAddress: activeAddress,
        recipientAddress: isNameInput ? (formik.values.resolvedAddress ?? '') : formik.values.to,
        amount: formik.values.amount,
    });

    const { data: transactionData } = sendCoinQuery;

    const isPayAllIota =
        selectedCoin.totalBalance === formik.values.amount &&
        selectedCoin.coinType === IOTA_TYPE_ARG;

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
                    amount: Number(formik.values.amount),
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
            <FormikProvider value={formik}>
                {step === FormStep.EnterValues && (
                    <EnterValuesFormView
                        coin={selectedCoin}
                        activeAddress={activeAddress}
                        onCoinSelect={(newCoin) => {
                            if (newCoin !== selectedCoin) {
                                setSelectedCoin(newCoin);
                                formik.resetForm();
                            }
                        }}
                        onNext={onNext}
                        onClose={() => setOpen(false)}
                        sendCoinTransactionQuery={sendCoinQuery}
                        coinBalance={coinBalance}
                        iotaBalance={iotaBalance}
                        showLoading={isLoadingCoins || isLoadingIotaCoins}
                    />
                )}
                {step === FormStep.ReviewValues && (
                    <ReviewValuesFormView
                        formData={formik.values}
                        executeTransfer={handleTransfer}
                        senderAddress={activeAddress}
                        isPending={isLoadingTransfer}
                        coinType={selectedCoin.coinType}
                        isPayAllIota={isPayAllIota}
                        onClose={() => setOpen(false)}
                        onBack={onBack}
                        totalGas={transactionData?.gasSummary?.totalGas}
                    />
                )}
            </FormikProvider>

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
