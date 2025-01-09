// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { Coin } from '_src/ui/app/redux/slices/iota-objects/coin';
import { ampli } from '_src/shared/analytics/ampli';
import {
    createStakeTransaction,
    createUnstakeTransaction,
    parseAmount,
    useBalance,
    useCoinMetadata,
    useGetDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    getStakeIotaByIotaId,
    createValidationSchema,
    MIN_NUMBER_IOTA_TO_STAKE,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import type { StakeObject } from '@iota/iota-sdk/client';
import { NANOS_PER_IOTA, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import * as Sentry from '@sentry/react';
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
import { StakeForm } from './StakeForm';
import { UnStakeForm } from './UnstakeForm';
import { ValidatorFormDetail } from './ValidatorFormDetail';
import {
    Button,
    ButtonType,
    CardType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
} from '@iota/apps-ui-kit';
import { ValidatorLogo } from '../validators/ValidatorLogo';
import { Info, Loader } from '@iota/ui-icons';

const INITIAL_VALUES = {
    amount: '',
};

export type FormValues = typeof INITIAL_VALUES;

export function StakingCard() {
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

    const { data: system, isPending: validatorsIsPending } = useIotaClientQuery(
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

    const coinSymbol = (coinType && Coin.getCoinSymbol(coinType)) || '';

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
    const delegationId = stakeData?.stakedIotaId;

    const navigate = useNavigate();
    const signer = useSigner(activeAccount);

    const { mutateAsync: stakeTokenMutateAsync, isPending: isStakeTokenTransactionPending } =
        useMutation({
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

                const sentryTransaction = Sentry.startTransaction({
                    name: 'stake',
                });
                try {
                    const transactionBlock = createStakeTransaction(amount, validatorAddress);
                    const tx = await signer.signAndExecuteTransaction({
                        transactionBlock,
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
                    sentryTransaction.finish();
                }
            },
            onSuccess: (_, { amount, validatorAddress }) => {
                ampli.stakedIota({
                    stakedAmount: Number(amount / NANOS_PER_IOTA),
                    validatorAddress: validatorAddress,
                });
            },
        });

    const { mutateAsync: unStakeTokenMutateAsync, isPending: isUnstakeTokenTransactionPending } =
        useMutation({
            mutationFn: async ({ stakedIotaId }: { stakedIotaId: string }) => {
                if (!stakedIotaId || !signer) {
                    throw new Error('Failed, missing required field.');
                }

                const sentryTransaction = Sentry.startTransaction({
                    name: 'stake',
                });
                try {
                    const transactionBlock = createUnstakeTransaction(stakedIotaId);
                    const tx = await signer.signAndExecuteTransaction({
                        transactionBlock,
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
                    sentryTransaction.finish();
                }
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
                let response;
                let txDigest;
                if (unstake) {
                    // check for delegation data
                    if (!stakeData || !stakeIotaIdParams) {
                        return;
                    }
                    response = await unStakeTokenMutateAsync({
                        stakedIotaId: stakeIotaIdParams,
                    });

                    txDigest = response.digest;
                } else {
                    const bigIntAmount = parseAmount(amount, coinDecimals);
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

    if (!coinType || !validatorAddress || (!validatorsIsPending && !system)) {
        return <Navigate to="/" replace={true} />;
    }
    return (
        <div className="flex h-full w-full flex-grow flex-col flex-nowrap">
            <Loading
                loading={
                    isPending ||
                    validatorsIsPending ||
                    loadingIotaBalances ||
                    isStakeTokenTransactionPending ||
                    isUnstakeTokenTransactionPending
                }
            >
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
                                {unstake && Number(iotaEarned) == 0 && (
                                    <InfoBox
                                        supportingText="You have not earned any rewards yet"
                                        icon={<Info />}
                                        type={InfoBoxType.Default}
                                        style={InfoBoxStyle.Elevated}
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
                                    icon={
                                        isSubmitting ? (
                                            <Loader
                                                className="animate-spin"
                                                data-testid="loading-indicator"
                                            />
                                        ) : null
                                    }
                                    iconAfterText
                                />
                            </div>
                        </>
                    )}
                </Formik>
            </Loading>
        </div>
    );
}
