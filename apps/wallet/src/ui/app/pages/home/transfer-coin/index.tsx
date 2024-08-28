// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { ActiveCoinsCard, Overlay } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useActiveAccount } from '_src/ui/app/hooks/useActiveAccount';
import { useSigner } from '_src/ui/app/hooks/useSigner';
import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import { createTokenTransferTransaction, useCoinMetadata } from '@iota/core';
// import * as Sentry from '@sentry/react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { toast } from 'react-hot-toast';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { Button, ButtonType } from '@iota/apps-ui-kit';

import { PreviewTransfer } from './PreviewTransfer';
import type { SubmitProps } from './SendTokenForm';
import { SendTokenForm } from './SendTokenForm';

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
            showModal
            title={showTransactionPreview ? 'Review & Send' : 'Send Coins'}
            closeOverlay={() => navigate('/')}
            showBackButton
        >
            <div className="flex h-full w-full flex-col">
                {showTransactionPreview && formData ? (
                    <>
                        <div className="flex flex-1">
                            <PreviewTransfer
                                coinType={coinType}
                                amount={formData.amount}
                                to={formData.to}
                                approximation={formData.isPayAllIota}
                                gasBudget={formData.gasBudgetEst}
                            />
                        </div>
                        <div className="pt-sm">
                            <Button
                                fullWidth
                                type={ButtonType.Primary}
                                onClick={() => executeTransfer.mutateAsync()}
                                text="Send Now"
                                disabled={coinType === null}
                            />
                        </div>
                    </>
                ) : (
                    <>
                        <div className="mb-7 flex flex-col gap-2.5">
                            <div className="pl-1.5">
                                <Text variant="caption" color="steel" weight="semibold">
                                    Select all Coins
                                </Text>
                            </div>
                            <ActiveCoinsCard activeCoinType={coinType} />
                        </div>

                        <SendTokenForm
                            onSubmit={(formData) => {
                                setShowTransactionPreview(true);
                                setFormData(formData);
                            }}
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

export default TransferCoinPage;
