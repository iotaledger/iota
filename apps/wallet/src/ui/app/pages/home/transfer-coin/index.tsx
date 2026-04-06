// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Overlay } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import {
    useSigner,
    useActiveAccount,
    useUnlockedGuard,
    usePinnedCoinTypes,
    useAppSelector,
} from '_hooks';
import {
    CoinSelector,
    useSortedCoinsByCategories,
    useSendCoinTransaction,
    toast,
    useGetAllBalances,
    useGetAllCoins,
    sumCoinBalances,
    useCoinMetadata,
    createValidationSchemaSendTokenForm,
    type SendTokenFormValues,
    useFeatureEnabledByNetwork,
    Feature,
} from '@iota/core';
import * as Sentry from '@sentry/react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { PreviewTransfer } from './PreviewTransfer';
import { SendTokenForm } from './SendTokenForm';
import { Button, ButtonType, LoadingIndicator } from '@iota/apps-ui-kit';
import { Loader } from '@iota/apps-ui-icons';
import { FormikProvider, useFormik } from 'formik';
import { shouldResolveInputAsName } from '@iota/core/utils/validation/names';

const INITIAL_VALUES: SendTokenFormValues = {
    to: '',
    amount: '',
    resolvedAddress: '',
};

export function TransferCoinPage() {
    const [searchParams] = useSearchParams();
    const [showTransactionPreview, setShowTransactionPreview] = useState<boolean>(false);

    const navigate = useNavigate();
    const activeAccount = useActiveAccount();
    const signer = useSigner(activeAccount);
    const address = activeAccount?.address || '';
    const queryClient = useQueryClient();

    const { data: coinsBalance, isPending: coinsBalanceIsPending } = useGetAllBalances(address);
    const selectedCoinType = searchParams.get('type') || coinsBalance?.[0]?.coinType || '';

    // Get all coins of the type
    const selectedCoinsQuery = useGetAllCoins(selectedCoinType, activeAccount?.address);
    const { data: selectedCoins = [] } = selectedCoinsQuery;

    const coinBalance = sumCoinBalances(selectedCoins);

    const coinMetadata = useCoinMetadata(selectedCoinType);
    const coinDecimals = coinMetadata.data?.decimals ?? 0;

    const network = useAppSelector((state) => state.app.network);
    const isNameResolutionEnabled = useFeatureEnabledByNetwork(Feature.IotaNames, network);

    const validationSchemaStepOne = useMemo(
        () =>
            createValidationSchemaSendTokenForm(
                isNameResolutionEnabled,
                coinBalance,
                coinMetadata.data?.symbol ?? '',
                coinDecimals,
            ),
        [isNameResolutionEnabled, coinBalance, coinMetadata.data, coinDecimals],
    );

    const formik = useFormik<SendTokenFormValues>({
        initialValues: INITIAL_VALUES,
        validationSchema: validationSchemaStepOne,
        validateOnChange: false,
        validateOnBlur: false,
        onSubmit: () => {},
    });

    const [pinnedCoinTypes] = usePinnedCoinTypes();
    const { recognized, pinned, unrecognized } = useSortedCoinsByCategories(
        coinsBalance || [],
        pinnedCoinTypes,
    );
    const sortedCoinsBalance = [...recognized, ...pinned, ...unrecognized];

    const totalCoinBalance =
        coinsBalance?.find((coin) => coin.coinType === selectedCoinType)?.totalBalance || '0';

    const isNameInput = shouldResolveInputAsName(formik.values.to);

    const sendCoinTransactionQuery = useSendCoinTransaction({
        coins: selectedCoins,
        coinType: selectedCoinType,
        senderAddress: address,
        recipientAddress: isNameInput ? (formik.values.resolvedAddress ?? '') : formik.values.to,
        amount: formik.values.amount,
    });
    const { data: transactionData, isPending } = sendCoinTransactionQuery;

    const executeTransfer = useMutation({
        mutationFn: async () => {
            if (!transactionData?.transaction || !signer) {
                throw new Error('Missing data');
            }
            return Sentry.startSpan(
                {
                    name: 'send-tokens',
                },
                (span) => {
                    try {
                        return signer.signAndExecuteTransaction({
                            transactionBlock: transactionData.transaction,
                            options: {
                                showInput: true,
                                showEffects: true,
                                showEvents: true,
                            },
                        });
                    } finally {
                        span?.end();
                    }
                },
            );
        },
        onSuccess: (response) => {
            queryClient.invalidateQueries({ queryKey: ['get-coins'] });
            queryClient.invalidateQueries({ queryKey: ['coin-balance'] });

            ampli.sentCoins({
                coinType: selectedCoinType!,
                amount: Number(formik.values.amount),
            });

            const receiptUrl = `/receipt?txdigest=${encodeURIComponent(
                response.digest,
            )}&from=transactions`;
            return navigate(receiptUrl);
        },
        onError: (error) => {
            toast.error(
                <div className="flex max-w-xs flex-col overflow-hidden">
                    <small className="overflow-hidden text-ellipsis">
                        {getSignerOperationErrorMessage(error)}
                    </small>
                </div>,
                {
                    duration: 10000,
                },
            );
        },
    });

    if (useUnlockedGuard()) {
        return null;
    }

    if (coinsBalanceIsPending) {
        return (
            <div className="flex h-full w-full items-center justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    if (!coinsBalance) {
        return <Navigate to="/" replace={true} />;
    }

    return (
        <Overlay
            showModal={true}
            title={showTransactionPreview ? 'Review & Send' : 'Send'}
            closeOverlay={() => navigate('/tokens')}
            showBackButton
            onBack={() =>
                showTransactionPreview ? setShowTransactionPreview(false) : navigate('/')
            }
        >
            <div className="flex h-full w-full flex-col gap-md">
                {showTransactionPreview && formik.values ? (
                    <div className="flex h-full flex-col">
                        <div className="h-full flex-1">
                            <PreviewTransfer
                                coinType={selectedCoinType}
                                amount={formik.values.amount}
                                to={formik.values.to}
                                coinBalance={totalCoinBalance}
                                gasBudget={transactionData?.gasSummary?.totalGas}
                            />
                        </div>
                        <Button
                            type={ButtonType.Primary}
                            onClick={() => {
                                executeTransfer.mutateAsync();
                            }}
                            text="Send Now"
                            disabled={
                                selectedCoinType === null || executeTransfer.isPending || isPending
                            }
                            icon={
                                executeTransfer.isPending ? (
                                    <Loader className="animate-spin" />
                                ) : undefined
                            }
                            iconAfterText
                        />
                    </div>
                ) : (
                    <>
                        <CoinSelector
                            activeCoinType={selectedCoinType}
                            coins={sortedCoinsBalance}
                            onClick={(coinType) => {
                                formik.resetForm();
                                navigate(
                                    `/send?${new URLSearchParams({ type: coinType }).toString()}`,
                                );
                            }}
                        />

                        <FormikProvider value={formik} key={selectedCoinType}>
                            <SendTokenForm
                                onNext={() => {
                                    setShowTransactionPreview(true);
                                }}
                                coinType={selectedCoinType}
                                sendCoinTransactionQuery={sendCoinTransactionQuery}
                                selectedCoinsQuery={selectedCoinsQuery}
                            />
                        </FormikProvider>
                    </>
                )}
            </div>
        </Overlay>
    );
}
