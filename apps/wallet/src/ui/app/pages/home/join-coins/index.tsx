// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Overlay } from '_components';
import { useActiveAccount, useSigner, useUnlockedGuard } from '_hooks';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { getGasSummary, useFormatCoin, toast } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { Button, ButtonType, KeyValueInfo, LoadingIndicator } from '@iota/apps-ui-kit';
import { Loader } from '@iota/apps-ui-icons';
import { Transaction } from '@iota/iota-sdk/transactions';
import { CoinFormat, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';
import { useNavigate } from 'react-router-dom';

export function JoinCoinsPage() {
    const navigate = useNavigate();
    const activeAccount = useActiveAccount();
    const address = activeAccount?.address;
    const signer = useSigner(activeAccount);
    const client = useIotaClient();
    const queryClient = useQueryClient();

    const isGuardLoading = useUnlockedGuard();

    // Fetch all IOTA coin objects in a single call
    const { data: coinsResult, isPending: isLoadingCoins } = useQuery({
        queryKey: ['join-coins-fetch', address],
        queryFn: () =>
            client.getCoins({
                owner: address!,
                coinType: IOTA_TYPE_ARG,
                limit: 1000,
            }),
        enabled: !!address,
        meta: { skipPersistedCache: true },
    });

    const coins = coinsResult?.data ?? [];
    const canJoin = coins.length >= 3;

    // Stable cache key
    const coinIdsKey = coins.map((c) => c.coinObjectId).join(',');

    // Sort and split inside a single memo so both dryRunTx and realTx
    // always operate on the exact same coin objects.
    const { gasCoin, sourceCoins } = useMemo(() => {
        const sorted = [...coins].sort((a, b) => Number(BigInt(b.balance) - BigInt(a.balance)));
        const [gasCoin, ...sourceCoins] = sorted;
        return { gasCoin, sourceCoins };
    }, [coinIdsKey]);

    // Dry-run tx — naive, just to measure gas cost. Uses the same merge shape.
    const dryRunTx = useMemo(() => {
        if (!canJoin || !address || !gasCoin || sourceCoins.length === 0) return null;
        const tx = new Transaction();
        tx.setSender(address);
        tx.mergeCoins(
            tx.gas,
            sourceCoins.map((c) => tx.object(c.coinObjectId)),
        );
        return tx;
    }, [canJoin, address, coinIdsKey]);

    const { data: dryRunResult, isLoading: isDryRunLoading } = useQuery({
        queryKey: ['join-coins-dry-run', address, coinIdsKey],
        queryFn: () => signer!.dryRunTransactionBlock({ transactionBlock: dryRunTx! }),
        enabled: !!dryRunTx && !!signer,
        meta: { skipPersistedCache: true },
    });

    const gasSummary = dryRunResult ? getGasSummary(dryRunResult) : null;

    // Use computationCost + storageCost as the budget (gross cost before rebate).
    // totalGas subtracts the rebate and can be negative, which is invalid as a u64 budget.
    const gasBudget = dryRunResult?.effects?.gasUsed
        ? BigInt(dryRunResult.effects.gasUsed.computationCost) +
          BigInt(dryRunResult.effects.gasUsed.storageCost)
        : null;

    const [formattedGas, gasSymbol] = useFormatCoin({
        balance: gasSummary?.totalGas,
        format: CoinFormat.Full,
    });

    // Real tx — built only after we know the gas cost from the dry-run.
    // Explicitly sets the largest coin as gas payment so we can merge all others
    // into it, even when there are only 2 coins total.
    const realTx = useMemo(() => {
        if (!gasBudget || !gasCoin || sourceCoins.length === 0) return null;
        const tx = new Transaction();
        tx.setSender(address!);
        tx.setGasPayment([
            {
                objectId: gasCoin.coinObjectId,
                version: gasCoin.version,
                digest: gasCoin.digest,
            },
        ]);
        tx.setGasBudget(gasBudget * 2n);
        tx.mergeCoins(
            tx.gas,
            sourceCoins.map((c) => tx.object(c.coinObjectId)),
        );
        return tx;
    }, [gasBudget, coinIdsKey]);

    // Execute
    const joinMutation = useMutation({
        mutationFn: async () => {
            if (!realTx || !signer) throw new Error('Missing data');
            return signer.signAndExecuteTransaction({
                transactionBlock: realTx,
                options: { showInput: true, showEffects: true, showEvents: true },
            });
        },
        onSuccess: (response) => {
            navigate(`/receipt?txdigest=${encodeURIComponent(response.digest)}&from=transactions`);
            setTimeout(() => {
                queryClient.invalidateQueries({ queryKey: ['get-coins'] });
                queryClient.invalidateQueries({ queryKey: ['coin-balance'] });
            }, 3_000);
        },
        onError: (error) => {
            toast.error(
                <div className="flex max-w-xs flex-col overflow-hidden">
                    <small className="overflow-hidden text-ellipsis">
                        {getSignerOperationErrorMessage(error)}
                    </small>
                </div>,
                { duration: 10000 },
            );
        },
    });

    if (isGuardLoading) return null;

    return (
        <Overlay
            showModal
            title="Join Coins"
            closeOverlay={() => navigate('/tokens')}
            showBackButton
            onBack={() => navigate('/tokens')}
        >
            <div className="flex h-full w-full flex-col justify-between gap-md">
                {isLoadingCoins ? (
                    <div className="flex flex-1 items-center justify-center">
                        <LoadingIndicator />
                    </div>
                ) : canJoin ? (
                    <>
                        <div className="flex flex-col gap-md">
                            <p className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                                You have{' '}
                                <span className="font-semibold text-iota-neutral-10 dark:text-iota-neutral-92">
                                    {coins.length} IOTA coin objects
                                </span>
                                . Joining them into one reduces future gas costs.
                            </p>
                            <KeyValueInfo
                                keyText="Est. Gas Fee"
                                value={
                                    isDryRunLoading || !gasSummary ? (
                                        <LoadingIndicator />
                                    ) : (
                                        formattedGas
                                    )
                                }
                                supportingLabel={gasSummary ? gasSymbol : undefined}
                                fullwidth
                            />
                        </div>
                        <Button
                            type={ButtonType.Primary}
                            text="Join All Coins"
                            onClick={() => joinMutation.mutate()}
                            disabled={joinMutation.isPending || isDryRunLoading || !realTx}
                            icon={
                                joinMutation.isPending ? (
                                    <Loader className="animate-spin" />
                                ) : undefined
                            }
                            iconAfterText
                        />
                    </>
                ) : (
                    <p className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                        Your IOTA coins are already consolidated into a single object.
                    </p>
                )}
            </div>
        </Overlay>
    );
}
