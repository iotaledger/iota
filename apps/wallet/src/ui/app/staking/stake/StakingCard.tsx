// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { Coin } from '_redux/slices/iota-objects/Coin';
import { ampli } from '_src/shared/analytics/ampli';
import { MIN_NUMBER_IOTA_TO_STAKE } from '_src/shared/constants';
import { Feature } from '_src/shared/experimentation/features';
import { useFeatureIsOn } from '@growthbook/growthbook-react';
import {
    createStakeTransaction,
    createUnstakeTransaction,
    parseAmount,
    useBalance,
    useCoinMetadata,
    useGetDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import type { StakeObject } from '@iota/iota-sdk/client';
import { NANO_PER_IOTA, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
// import * as Sentry from '@sentry/react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Formik } from 'formik';
import type { FormikHelpers } from 'formik';
import { useCallback, useMemo } from 'react';
import { toast } from 'react-hot-toast';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { getSignerOperationErrorMessage } from '../../helpers/errorMessages';
import { useActiveAccount } from '../../hooks/useActiveAccount';
import { useSigner } from '../../hooks/useSigner';
import { getDelegationDataByStakeId } from '../getDelegationByStakeId';
import { getStakeIotaByIotaId } from '../getStakeIotaByIotaId';
import StakeForm from './StakeForm';
import { UnStakeForm } from './UnstakeForm';
import { createValidationSchema } from './utils/validation';
import { ValidatorFormDetail } from './ValidatorFormDetail';
import { Button, ButtonType, CardType } from '@iota/apps-ui-kit';
import { ValidatorLogo } from '../validators/ValidatorLogo';

const INITIAL_VALUES = {
    amount: '',
};

export type FormValues = typeof INITIAL_VALUES;

function StakingCard() {
    const coinType = IOTA_TYPE_ARG;
    const activeAccount = useActiveAccount();
    const accountAddress = activeAccount?.address;
    const { data: iotaBalance, isPending: loadingIotaBalances } = useBalance(accountAddress!);

    const coinBalance = BigInt(iotaBalance?.totalBalance || 0);
    const [searchParams] = useSearchParams();
    const validatorAddress = searchParams.get('address');
    const stakeIotaIdParams = searchParams.get('staked');
    const unstake = searchParams.get('unstake') === 'true';
    const { data: allDelegation, isPending } = useGetDelegatedStake({
        address: accountAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });
    const effectsOnlySharedTransactions = useFeatureIsOn(
        Feature.WalletEffectsOnlySharedTransaction as string,
    );

    const { data: system, isPending: validatorsisPending } = useIotaClientQuery(
        'getLatestIotaSystemState',
    );

    const totalTokenBalance = useMemo(() => {
        if (!allDelegation) return 0n;
        // return only the total amount of tokens staked for a specific stakeId
        return getStakeIotaByIotaId(allDelegation, stakeIotaIdParams);
    }, [allDelegation, stakeIotaIdParams]);

    const stakeData = useMemo(() => {
        if (!allDelegation || !stakeIotaIdParams) return null;
        // return delegation data for a specific stakeId
        return getDelegationDataByStakeId(allDelegation, stakeIotaIdParams);
    }, [allDelegation, stakeIotaIdParams]);

    const coinSymbol = useMemo(() => (coinType && Coin.getCoinSymbol(coinType)) || '', [coinType]);

    const iotaEarned =
        (stakeData as Extract<StakeObject, { estimatedReward: string }>)?.estimatedReward || '0';

    const { data: metadata } = useCoinMetadata(coinType);
    const coinDecimals = metadata?.decimals ?? 0;
    // set minimum stake amount to 1 IOTA
    const minimumStake = parseAmount(MIN_NUMBER_IOTA_TO_STAKE.toString(), coinDecimals);

    const validationSchema = useMemo(
        () => createValidationSchema(coinBalance, coinSymbol, coinDecimals, unstake, minimumStake),
        [coinBalance, coinSymbol, coinDecimals, unstake, minimumStake],
    );

    const queryClient = useQueryClient();
    const delegationId = useMemo(() => {
        if (!stakeData || stakeData.status === 'Pending') return null;
        return stakeData.stakedIotaId;
    }, [stakeData]);

    const navigate = useNavigate();
    const signer = useSigner(activeAccount);

    const { mutateAsync: stakeTokenMutateAsync } = useMutation({
        mutationFn: async ({
            tokenTypeArg,
            amount,
            validatorAddress,
        }: {
            tokenTypeArg: string;
            amount: bigint;
            validatorAddress: string;
        }) => {
            if (!validatorAddress || !amount || !tokenTypeArg || !signer) {
                throw new Error('Failed, missing required field');
            }

            // const sentryTransaction = Sentry.startTransaction({
            // 	name: 'stake',
            // });
            try {
                const transactionBlock = createStakeTransaction(amount, validatorAddress);
                return await signer.signAndExecuteTransaction({
                    transactionBlock,
                    requestType: effectsOnlySharedTransactions
                        ? 'WaitForEffectsCert'
                        : 'WaitForLocalExecution',
                    options: {
                        showInput: true,
                        showEffects: true,
                        showEvents: true,
                    },
                });
            } finally {
                // sentryTransaction.finish();
            }
        },
        onSuccess: (_, { amount, validatorAddress }) => {
            ampli.stakedIota({
                stakedAmount: Number(amount / NANO_PER_IOTA),
                validatorAddress: validatorAddress,
            });
        },
    });

    const { mutateAsync: unStakeTokenMutateAsync } = useMutation({
        mutationFn: async ({ stakedIotaId }: { stakedIotaId: string }) => {
            if (!stakedIotaId || !signer) {
                throw new Error('Failed, missing required field.');
            }

            // const sentryTransaction = Sentry.startTransaction({
            // 	name: 'stake',
            // });
            const transactionBlock = createUnstakeTransaction(stakedIotaId);
            return await signer.signAndExecuteTransaction({
                transactionBlock,
                requestType: effectsOnlySharedTransactions
                    ? 'WaitForEffectsCert'
                    : 'WaitForLocalExecution',
                options: {
                    showInput: true,
                    showEffects: true,
                    showEvents: true,
                },
            });
            // finally {
            // 	sentryTransaction.finish();
            // }
        },
        onSuccess: () => {
            ampli.unstakedIota({
                validatorAddress: validatorAddress!,
            });
        },
    });

    const onSubmit = useCallback(
        async ({ amount }: FormValues, { resetForm }: FormikHelpers<FormValues>) => {
            if (coinType === null || validatorAddress === null) {
                return;
            }
            try {
                const bigIntAmount = parseAmount(amount, coinDecimals);
                let response;
                let txDigest;
                if (unstake) {
                    // check for delegation data
                    if (!stakeData || !stakeIotaIdParams || stakeData.status === 'Pending') {
                        return;
                    }
                    response = await unStakeTokenMutateAsync({
                        stakedIotaId: stakeIotaIdParams,
                    });

                    txDigest = response.digest;
                } else {
                    response = await stakeTokenMutateAsync({
                        amount: bigIntAmount,
                        tokenTypeArg: coinType,
                        validatorAddress: validatorAddress,
                    });
                    txDigest = response.digest;
                }

                // Invalidate the react query for system state and validator
                Promise.all([
                    queryClient.invalidateQueries({
                        queryKey: ['system', 'state'],
                    }),
                    queryClient.invalidateQueries({
                        queryKey: ['delegated-stakes'],
                    }),
                ]);
                resetForm();

                navigate(
                    `/receipt?${new URLSearchParams({
                        txdigest: txDigest,
                        from: 'tokens',
                    }).toString()}`,
                    response?.transaction
                        ? {
                              state: {
                                  response,
                              },
                          }
                        : undefined,
                );
            } catch (error) {
                toast.error(
                    <div className="flex max-w-xs flex-col overflow-hidden">
                        <strong>{unstake ? 'Unstake' : 'Stake'} failed</strong>
                        <small className="overflow-hidden text-ellipsis">
                            {getSignerOperationErrorMessage(error)}
                        </small>
                    </div>,
                );
            }
        },
        [
            coinType,
            validatorAddress,
            coinDecimals,
            unstake,
            queryClient,
            navigate,
            stakeData,
            stakeIotaIdParams,
            unStakeTokenMutateAsync,
            stakeTokenMutateAsync,
        ],
    );

    if (!coinType || !validatorAddress || (!validatorsisPending && !system)) {
        return <Navigate to="/" replace={true} />;
    }
    return (
        <div className="flex h-full w-full flex-grow flex-col flex-nowrap">
            <Loading loading={isPending || validatorsisPending || loadingIotaBalances}>
                <Formik
                    initialValues={INITIAL_VALUES}
                    validationSchema={validationSchema}
                    onSubmit={onSubmit}
                    validateOnMount
                >
                    {({ isSubmitting, isValid, submitForm }) => (
                        <>
                            <div className="flex h-full flex-col gap-md overflow-auto">
                                <ValidatorLogo
                                    validatorAddress={validatorAddress}
                                    type={CardType.Filled}
                                />
                                <ValidatorFormDetail
                                    validatorAddress={validatorAddress}
                                    unstake={unstake}
                                />
                                {unstake ? (
                                    <UnStakeForm
                                        stakedIotaId={stakeIotaIdParams!}
                                        coinBalance={totalTokenBalance}
                                        coinType={coinType}
                                        stakingReward={iotaEarned}
                                        epoch={Number(system?.epoch || 0)}
                                    />
                                ) : (
                                    <StakeForm
                                        validatorAddress={validatorAddress}
                                        coinBalance={coinBalance}
                                        coinType={coinType}
                                        epoch={system?.epoch}
                                    />
                                )}
                            </div>
                            <div className="pt-sm">
                                <Button
                                    type={unstake ? ButtonType.Secondary : ButtonType.Primary}
                                    fullWidth
                                    onClick={submitForm}
                                    disabled={
                                        !isValid || isSubmitting || (unstake && !delegationId)
                                    }
                                    text={unstake ? 'Unstake' : 'Stake'}
                                />
                            </div>
                        </>
                    )}
                </Formik>
            </Loading>
        </div>
    );
}

export default StakingCard;
