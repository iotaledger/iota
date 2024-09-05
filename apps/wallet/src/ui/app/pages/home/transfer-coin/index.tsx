// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinIcon, Loading, Overlay } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useActiveAccount } from '_src/ui/app/hooks/useActiveAccount';
import { useSigner } from '_src/ui/app/hooks/useSigner';
import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import {
    createTokenTransferTransaction,
    filterAndSortTokenBalances,
    useCoinMetadata,
    useFormatCoin,
} from '@iota/core';
// import * as Sentry from '@sentry/react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { toast } from 'react-hot-toast';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';

import { PreviewTransfer } from './PreviewTransfer';
import { SendTokenForm, type SubmitProps } from './SendTokenForm';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Select, Button, type SelectOption, ButtonType } from '@iota/apps-ui-kit';
import { useActiveAddress, useCoinsReFetchingConfig } from '_src/ui/app/hooks';
import { useIotaClientQuery } from '@iota/dapp-kit';
import type { CoinBalance } from '@iota/iota-sdk/client';
import { ImageIconSize } from '_src/ui/app/shared/image-icon';

function TransferCoinPage() {
    const [searchParams] = useSearchParams();
    const coinType = searchParams.get('type');
    const [showTransactionPreview, setShowTransactionPreview] = useState<boolean>(false);
    const [formData, setFormData] = useState<SubmitProps>();
    const navigate = useNavigate();
    const { data: coinMetadata } = useCoinMetadata(coinType);
    const activeAccount = useActiveAccount();
    const signer = useSigner(activeAccount);
    const address = activeAccount?.address;
    const queryClient = useQueryClient();

    const transaction = useMemo(() => {
        if (!coinType || !signer || !formData || !address) return null;

        return createTokenTransferTransaction({
            coinType,
            coinDecimals: coinMetadata?.decimals ?? 0,
            ...formData,
        });
    }, [formData, signer, coinType, address, coinMetadata?.decimals]);

    const executeTransfer = useMutation({
        mutationFn: async () => {
            if (!transaction || !signer) {
                throw new Error('Missing data');
            }

            // const sentryTransaction = Sentry.startTransaction({
            // 	name: 'send-tokens',
            // });
            return signer.signAndExecuteTransactionBlock({
                transactionBlock: transaction,
                options: {
                    showInput: true,
                    showEffects: true,
                    showEvents: true,
                },
            });

            // finally {
            // sentryTransaction.finish();
            // }
        },
        onSuccess: (response) => {
            queryClient.invalidateQueries({ queryKey: ['get-coins'] });
            queryClient.invalidateQueries({ queryKey: ['coin-balance'] });

            ampli.sentCoins({
                coinType: coinType!,
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
            );
        },
    });

    if (useUnlockedGuard()) {
        return null;
    }

    if (!coinType) {
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
                                coinType={coinType}
                                amount={formData.amount}
                                to={formData.to}
                                approximation={formData.isPayAllIota}
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
                            disabled={coinType === null || executeTransfer.isPending}
                        />
                    </div>
                ) : (
                    <>
                        <CoinSelector
                            onActiveCoinChange={() => setFormData(undefined)}
                            activeCoinType={coinType}
                        />

                        <SendTokenForm
                            onSubmit={(formData) => {
                                setFormData(formData);
                                setShowTransactionPreview(true);
                            }}
                            key={coinType}
                            coinType={coinType}
                            initialAmount={formData?.amount || ''}
                            initialTo={formData?.to || ''}
                        />
                    </>
                )}
            </div>
        </Overlay>
    );
}

function CoinSelector({
    activeCoinType = IOTA_TYPE_ARG,
    onActiveCoinChange,
}: {
    activeCoinType: string;
    onActiveCoinChange: () => void;
}) {
    const selectedAddress = useActiveAddress();
    const navigate = useNavigate();

    const { staleTime, refetchInterval } = useCoinsReFetchingConfig();
    const { data: coins, isPending } = useIotaClientQuery(
        'getAllBalances',
        { owner: selectedAddress! },
        {
            enabled: !!selectedAddress,
            refetchInterval,
            staleTime,
            select: filterAndSortTokenBalances,
        },
    );

    if (!coins?.length) {
        return <Navigate to="/" replace={true} />;
    }

    const activeCoin = coins?.find(({ coinType }) => coinType === activeCoinType) ?? coins?.[0];
    const initialValue = activeCoin?.coinType;
    const coinsOptions: SelectOption[] =
        coins?.map((coin) => ({
            id: coin.coinType,
            renderLabel: () => <CoinSelectOption coin={coin} />,
        })) || [];

    return (
        <Loading loading={isPending}>
            <Select
                label="Select Coins"
                value={initialValue}
                options={coinsOptions}
                onValueChange={(coinType) => {
                    onActiveCoinChange();
                    navigate(`/send?${new URLSearchParams({ type: coinType }).toString()}`);
                }}
            />
        </Loading>
    );
}

function CoinSelectOption({
    coin: { coinType, totalBalance },
    size,
}: {
    coin: CoinBalance;
    size?: ImageIconSize;
}) {
    const [formatted, symbol, { data: coinMeta }] = useFormatCoin(totalBalance, coinType);
    const isIota = coinType === IOTA_TYPE_ARG;

    return (
        <div className="flex w-full flex-row items-center justify-between">
            <div className="flex flex-row items-center gap-x-md">
                <div className={size}>
                    <CoinIcon size={ImageIconSize.Small} coinType={coinType} />
                </div>
                <span className="text-body-lg text-neutral-10">
                    {isIota ? (coinMeta?.name || '').toUpperCase() : coinMeta?.name || symbol}
                </span>
            </div>
            <span className="text-label-lg text-neutral-60">
                {formatted} {symbol}
            </span>
        </div>
    );
}

export default TransferCoinPage;
