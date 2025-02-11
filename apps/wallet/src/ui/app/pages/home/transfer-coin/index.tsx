// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Overlay } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useSigner, useActiveAccount, useUnlockedGuard } from '_hooks';
import {
    COINS_QUERY_REFETCH_INTERVAL,
    COINS_QUERY_STALE_TIME,
    CoinSelector,
    createTokenTransferTransaction,
    filterAndSortTokenBalances,
    parseAmount,
    useCoinMetadata,
} from '@iota/core';
import * as Sentry from '@sentry/react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { toast } from 'react-hot-toast';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { PreviewTransfer } from './PreviewTransfer';
import { SendTokenForm, type SubmitProps } from './SendTokenForm';
import { Button, ButtonType, LoadingIndicator } from '@iota/apps-ui-kit';
import { Loader } from '@iota/apps-ui-icons';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export function TransferCoinPage() {
    const [searchParams] = useSearchParams();
    const selectedCoinType = searchParams.get('type');
    const [showTransactionPreview, setShowTransactionPreview] = useState<boolean>(false);
    const [formData, setFormData] = useState<SubmitProps>();
    const navigate = useNavigate();
    const { data: coinMetadata } = useCoinMetadata(selectedCoinType);
    const activeAccount = useActiveAccount();
    const signer = useSigner(activeAccount);
    const address = activeAccount?.address;
    const queryClient = useQueryClient();

    const { data: coinsBalance, isPending: coinsBalanceIsPending } = useIotaClientQuery(
        'getAllBalances',
        { owner: address! },
        {
            enabled: !!address,
            refetchInterval: COINS_QUERY_REFETCH_INTERVAL,
            staleTime: COINS_QUERY_STALE_TIME,
            select: filterAndSortTokenBalances,
        },
    );
    const coinBalance = coinsBalance?.find(
        (coin) => coin.coinType === selectedCoinType,
    )?.totalBalance;

    const selectedAmount = formData?.amount;
    const selectedCoinDecimals = coinMetadata?.decimals;
    const hasSelectedMaxCoinBalance =
        selectedAmount && selectedCoinDecimals && coinBalance
            ? parseAmount(selectedAmount, coinMetadata.decimals) === BigInt(coinBalance)
            : false;

    if (coinsBalanceIsPending) {
        return (
            <div className="flex h-full w-full items-center justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    const isPayAllIota: boolean =
        (hasSelectedMaxCoinBalance && selectedCoinType === IOTA_TYPE_ARG) ?? false;

    const transaction = useMemo(() => {
        if (!selectedCoinType || !signer || !formData || !address) return null;

        return createTokenTransferTransaction({
            coinType: selectedCoinType,
            coinDecimals: coinMetadata?.decimals ?? 0,
            isPayAllIota,
            ...formData,
        });
    }, [formData, signer, selectedCoinType, address, coinMetadata?.decimals]);

    const executeTransfer = useMutation({
        mutationFn: async () => {
            if (!transaction || !signer) {
                throw new Error('Missing data');
            }
            return Sentry.startSpan(
                {
                    name: 'send-tokens',
                },
                (span) => {
                    try {
                        return signer.signAndExecuteTransaction({
                            transactionBlock: transaction,
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

    if (!selectedCoinType || !coinsBalance) {
        return <Navigate to="/" replace={true} />;
    }

    return (
        <Overlay
            showModal={true}
            title={showTransactionPreview ? 'Review & Send' : 'Send'}
            closeOverlay={() => navigate('/')}
            showBackButton
            onBack={showTransactionPreview ? () => setShowTransactionPreview(false) : undefined}
        >
            <div className="flex h-full w-full flex-col gap-md">
                {showTransactionPreview && formData ? (
                    <div className="flex h-full flex-col">
                        <div className="h-full flex-1">
                            <PreviewTransfer
                                coinType={selectedCoinType}
                                amount={formData.amount}
                                to={formData.to}
                                approximation={isPayAllIota}
                                gasBudget={formData.gasBudgetEst}
                            />
                        </div>
                        <Button
                            type={ButtonType.Primary}
                            onClick={() => {
                                setFormData(formData);
                                executeTransfer.mutateAsync();
                            }}
                            text="Send Now"
                            disabled={selectedCoinType === null || executeTransfer.isPending}
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
                            coins={coinsBalance || []}
                            onClick={(coinType) => {
                                setFormData(undefined);
                                navigate(
                                    `/send?${new URLSearchParams({ type: coinType }).toString()}`,
                                );
                            }}
                        />

                        <SendTokenForm
                            onSubmit={(formData) => {
                                setFormData(formData);
                                setShowTransactionPreview(true);
                            }}
                            key={selectedCoinType}
                            coinType={selectedCoinType}
                            initialAmount={formData?.amount || ''}
                            initialTo={formData?.to || ''}
                        />
                    </>
                )}
            </div>
        </Overlay>
    );
}
