// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    TimeUnit,
    useFormatCoin,
    useGetTimeBeforeEpochNumber,
    useTimeAgo,
    GAS_SYMBOL,
    useNewUnstakeTransaction,
    CoinFormat,
    useGetDelegatedStake,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    getStakeIotaByIotaId,
    getDelegationDataByStakeId,
    Validator,
} from '@iota/core';
import { useMemo } from 'react';
import { useActiveAccount, useSigner } from '_hooks';
import {
    Button,
    ButtonType,
    CardType,
    Divider,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    KeyValueInfo,
    Panel,
} from '@iota/apps-ui-kit';
import { useMutation } from '@tanstack/react-query';
import * as Sentry from '@sentry/react';
import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '../../helpers';
import toast from 'react-hot-toast';
import { Info, Loader } from '@iota/apps-ui-icons';
import { type IotaTransactionBlockResponse, type StakeObject } from '@iota/iota-sdk/client';
import { ValidatorFormDetail } from './ValidatorFormDetail';

export interface StakeFromProps {
    stakedIotaId: string;
    validatorAddress: string;
    epoch: number;
    onSuccess: (response: IotaTransactionBlockResponse) => void;
}

export function UnStakeForm({ stakedIotaId, validatorAddress, epoch, onSuccess }: StakeFromProps) {
    const activeAccount = useActiveAccount();
    const activeAddress = activeAccount?.address ?? '';
    const signer = useSigner(activeAccount);

    const { data: allDelegation, isPending } = useGetDelegatedStake({
        address: activeAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const totalTokenBalance = useMemo(() => {
        if (!allDelegation) return 0n;
        // return only the total amount of tokens staked for a specific stakeId
        return getStakeIotaByIotaId(allDelegation, stakedIotaId);
    }, [allDelegation, stakedIotaId]);

    const stakeData = useMemo(() => {
        if (!allDelegation || !stakedIotaId) return null;
        // return delegation data for a specific stakeId
        return getDelegationDataByStakeId(allDelegation, stakedIotaId);
    }, [allDelegation, stakedIotaId]);

    const iotaEarned =
        (stakeData as Extract<StakeObject, { estimatedReward: string }>)?.estimatedReward || '0';
    const [rewards, rewardSymbol] = useFormatCoin({ balance: iotaEarned });
    const [totalIota] = useFormatCoin({ balance: BigInt(iotaEarned || 0) + totalTokenBalance });
    const [tokenBalance] = useFormatCoin({ balance: totalTokenBalance });

    const {
        data: unstakeData,
        isLoading: isUnstakeTokenTransactionLoading,
        isError,
    } = useNewUnstakeTransaction(activeAddress, stakedIotaId);
    const transaction = unstakeData?.transaction;

    const [formattedGas, gasSymbol] = useFormatCoin({
        balance: unstakeData?.gasSummary?.totalGas,
        format: CoinFormat.FULL,
    });
    const { data: currentEpochEndTime } = useGetTimeBeforeEpochNumber(epoch + 1 || 0);
    const currentEpochEndTimeAgo = useTimeAgo({
        timeFrom: currentEpochEndTime,
        endLabel: '--',
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });

    const currentEpochEndTimeFormatted =
        currentEpochEndTime > 0 ? currentEpochEndTimeAgo : `Epoch #${epoch}`;

    const { mutateAsync: unStakeTokenMutateAsync, isPending: isUnstakeTokenTransactionPending } =
        useMutation({
            mutationFn: async () => {
                if (!transaction || !signer) {
                    throw new Error('Failed, missing required field.');
                }

                return Sentry.startSpan(
                    {
                        name: 'unstake',
                    },
                    async (span) => {
                        try {
                            const tx = await signer.signAndExecuteTransaction({
                                transactionBlock: transaction,
                                options: {
                                    showInput: true,
                                    showEffects: true,
                                    showEvents: true,
                                },
                            });
                            await signer.client.waitForTransaction({
                                digest: tx.digest,
                            });
                            return tx;
                        } finally {
                            span?.end();
                        }
                    },
                );
            },
            onSuccess: () => {
                ampli.unstakedIota({
                    validatorAddress: validatorAddress!,
                });
            },
        });
    const handleSubmit = async () => {
        try {
            const response = await unStakeTokenMutateAsync();
            onSuccess(response);
        } catch (error) {
            toast.error(
                <div className="flex max-w-xs flex-col overflow-hidden">
                    <strong>Unstake failed</strong>
                    <small className="overflow-hidden text-ellipsis">
                        {getSignerOperationErrorMessage(error)}
                    </small>
                </div>,
            );
        }
    };

    const isLoading =
        isPending || isUnstakeTokenTransactionPending || isUnstakeTokenTransactionLoading;

    return (
        <>
            <div className="flex flex-1 flex-col flex-nowrap gap-y-md overflow-auto">
                <Validator address={validatorAddress} type={CardType.Filled} />
                <ValidatorFormDetail validatorAddress={validatorAddress} unstake={true} />
                <Panel hasBorder>
                    <div className="flex flex-col gap-y-sm p-md">
                        <KeyValueInfo
                            keyText="Current Epoch Ends"
                            value={currentEpochEndTimeFormatted}
                            fullwidth
                        />
                        <Divider />
                        <KeyValueInfo
                            keyText="Your Stake"
                            value={tokenBalance}
                            supportingLabel={GAS_SYMBOL}
                            fullwidth
                        />
                        <KeyValueInfo
                            keyText="Rewards Earned"
                            value={rewards}
                            supportingLabel={rewardSymbol}
                            fullwidth
                        />
                        <Divider />
                        <KeyValueInfo
                            keyText="Total unstaked IOTA"
                            value={totalIota}
                            supportingLabel={GAS_SYMBOL}
                            fullwidth
                        />
                    </div>
                </Panel>
                <Panel hasBorder>
                    <div className="flex flex-col gap-y-sm p-md">
                        <KeyValueInfo
                            keyText="Gas Fees"
                            value={formattedGas || '-'}
                            supportingLabel={gasSymbol}
                            fullwidth
                        />
                    </div>
                </Panel>
            </div>
            {Number(iotaEarned) == 0 && (
                <div className="pt-sm">
                    <InfoBox
                        supportingText="You have not earned any rewards yet"
                        icon={<Info />}
                        type={InfoBoxType.Default}
                        style={InfoBoxStyle.Elevated}
                    />
                </div>
            )}
            <div className="pt-sm">
                <Button
                    type={ButtonType.Primary}
                    fullWidth
                    onClick={handleSubmit}
                    disabled={isError || isLoading}
                    text="Unstake"
                    icon={
                        isLoading ? (
                            <Loader className="animate-spin" data-testid="loading-indicator" />
                        ) : null
                    }
                    iconAfterText
                />
            </div>
        </>
    );
}
