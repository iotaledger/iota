// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable @typescript-eslint/no-explicit-any */

import { roundFloat, useGetValidatorsApy, useGetValidatorsEvents } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Heading } from '@iota/ui';
import { lazy, Suspense, useMemo } from 'react';
import { generateValidatorsTableData } from '~/lib/ui/utils';

import { DelegationAmount, ErrorBoundary, PageLayout } from '~/components';
import { Banner, Card, PlaceholderTable, Stats, TableCard, TableHeader } from '~/components/ui';

const ValidatorMap = lazy(() => import('../../components/validator-map/ValidatorMap'));

function ValidatorPageResult(): JSX.Element {
    const { data, isPending, isSuccess, isError } = useIotaClientQuery('getLatestIotaSystemState');

    const numberOfValidators = data?.activeValidators.length || 0;

    const {
        data: validatorEvents,
        isPending: validatorsEventsLoading,
        isError: validatorEventError,
    } = useGetValidatorsEvents({
        limit: numberOfValidators,
        order: 'descending',
    });

    const { data: validatorsApy } = useGetValidatorsApy();

    const totalStaked = useMemo(() => {
        if (!data) return 0;
        const validators = data.activeValidators;

        return validators.reduce((acc, cur) => acc + Number(cur.stakingPoolIotaBalance), 0);
    }, [data]);

    const averageAPY = useMemo(() => {
        if (!validatorsApy || Object.keys(validatorsApy)?.length === 0) return null;

        // if all validators have isApyApproxZero, return ~0
        if (Object.values(validatorsApy)?.every(({ isApyApproxZero }) => isApyApproxZero)) {
            return '~0';
        }

        // exclude validators with no apy
        const apys = Object.values(validatorsApy)?.filter((a) => a.apy > 0 && !a.isApyApproxZero);
        const averageAPY = apys?.reduce((acc, cur) => acc + cur.apy, 0);
        // in case of no apy, return 0
        return apys.length > 0 ? roundFloat(averageAPY / apys.length) : 0;
    }, [validatorsApy]);

    const lastEpochRewardOnAllValidators = useMemo(() => {
        if (!validatorEvents) return null;
        let totalRewards = 0;

        validatorEvents.forEach(({ parsedJson }) => {
            totalRewards += Number(
                (parsedJson as { pool_staking_reward: string }).pool_staking_reward,
            );
        });

        return totalRewards;
    }, [validatorEvents]);

    const validatorsTable = useMemo(() => {
        if (!data || !validatorEvents) return null;
        return generateValidatorsTableData({
            validators: data.activeValidators,
            atRiskValidators: data.atRiskValidators,
            validatorEvents,
            rollingAverageApys: validatorsApy || null,
        });
    }, [data, validatorEvents, validatorsApy]);

    return (
        <PageLayout
            content={
                isError || validatorEventError ? (
                    <Banner variant="error" fullWidth>
                        Validator data could not be loaded
                    </Banner>
                ) : (
                    <>
                        <div className="grid gap-5 md:grid-cols-2">
                            <Card spacing="lg">
                                <div className="flex w-full basis-full flex-col gap-8">
                                    <Heading
                                        as="div"
                                        variant="heading4/semibold"
                                        color="steel-darker"
                                    >
                                        Validators
                                    </Heading>

                                    <div className="flex flex-col gap-8 md:flex-row">
                                        <div className="flex flex-col gap-8">
                                            <Stats
                                                label="Participation"
                                                tooltip="Coming soon"
                                                unavailable
                                            />

                                            <Stats
                                                label="Last Epoch Rewards"
                                                tooltip="The stake rewards collected during the last epoch."
                                                unavailable={
                                                    lastEpochRewardOnAllValidators === null
                                                }
                                            >
                                                <DelegationAmount
                                                    amount={
                                                        typeof lastEpochRewardOnAllValidators ===
                                                        'number'
                                                            ? lastEpochRewardOnAllValidators
                                                            : 0n
                                                    }
                                                    isStats
                                                />
                                            </Stats>
                                        </div>
                                        <div className="flex flex-col gap-8">
                                            <Stats
                                                label="Total IOTA Staked"
                                                tooltip="The total IOTA staked on the network by validators and delegators to validate the network and earn rewards."
                                                unavailable={totalStaked <= 0}
                                            >
                                                <DelegationAmount
                                                    amount={totalStaked || 0n}
                                                    isStats
                                                />
                                            </Stats>
                                            <Stats
                                                label="AVG APY"
                                                tooltip="The global average of annualized percentage yield of all participating validators."
                                                unavailable={averageAPY === null}
                                            >
                                                {averageAPY}%
                                            </Stats>
                                        </div>
                                    </div>
                                </div>
                            </Card>

                            <ErrorBoundary>
                                <Suspense fallback={null}>
                                    <ValidatorMap minHeight={230} />
                                </Suspense>
                            </ErrorBoundary>
                        </div>
                        <div className="mt-8">
                            <ErrorBoundary>
                                <TableHeader>All Validators</TableHeader>
                                {(isPending || validatorsEventsLoading) && (
                                    <PlaceholderTable
                                        rowCount={20}
                                        rowHeight="13px"
                                        colHeadings={['Name', 'Address', 'Stake']}
                                    />
                                )}

                                {isSuccess && validatorsTable?.data && (
                                    <TableCard
                                        data={validatorsTable.data}
                                        columns={validatorsTable.columns}
                                        areHeadersCentered={false}
                                    />
                                )}
                            </ErrorBoundary>
                        </div>
                    </>
                )
            }
        />
    );
}

export { ValidatorPageResult };
