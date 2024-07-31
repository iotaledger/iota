// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card } from '_app/shared/card';
import { Text } from '_app/shared/text';
import NumberInput from '_components/number-input';
import {
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE,
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_STARTS,
} from '_src/shared/constants';
import { CountDownTimer } from '_src/ui/app/shared/countdown-timer';
import {
    createStakeTransaction,
    parseAmount,
    useCoinMetadata,
    useFormatCoin,
    useGetTimeBeforeEpochNumber,
} from '@iota/core';
import { Field, Form, useFormikContext } from 'formik';
import { memo, useCallback, useMemo } from 'react';

import { useActiveAddress, useTransactionGasBudget } from '../../hooks';
import { type FormValues } from './StakingCard';

const HIDE_MAX = true;

export interface StakeFromProps {
    validatorAddress: string;
    coinBalance: bigint;
    coinType: string;
    epoch?: string | number;
}

function StakeForm({ validatorAddress, coinBalance, coinType, epoch }: StakeFromProps) {
    const { values, setFieldValue } = useFormikContext<FormValues>();

    const { data: metadata } = useCoinMetadata(coinType);
    const decimals = metadata?.decimals ?? 0;
    const [maxToken, symbol, queryResult] = useFormatCoin(coinBalance, coinType);

    const transaction = useMemo(() => {
        if (!values.amount || !decimals) return null;
        const amountWithoutDecimals = parseAmount(values.amount, decimals);
        return createStakeTransaction(amountWithoutDecimals, validatorAddress);
    }, [values.amount, validatorAddress, decimals]);

    const activeAddress = useActiveAddress();
    const { data: gasBudget } = useTransactionGasBudget(activeAddress, transaction);

    const setMaxToken = useCallback(() => {
        if (!maxToken) return;
        setFieldValue('amount', maxToken);
    }, [maxToken, setFieldValue]);

    // Reward will be available after 2 epochs
    const startEarningRewardsEpoch =
        Number(epoch || 0) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_STARTS;

    const redeemableRewardsEpoch =
        Number(epoch || 0) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE;

    const { data: timeBeforeStakeRewardsStarts } =
        useGetTimeBeforeEpochNumber(startEarningRewardsEpoch);

    const { data: timeBeforeStakeRewardsRedeemable } =
        useGetTimeBeforeEpochNumber(redeemableRewardsEpoch);

    return (
        <Form className="flex flex-1 flex-col flex-nowrap items-center" autoComplete="off">
            <div className="mb-3 mt-3.5 flex w-full flex-col items-center justify-between gap-1.5">
                <Text variant="caption" color="gray-85" weight="semibold">
                    Enter the amount of IOTA to stake
                </Text>
                <Text variant="bodySmall" color="steel" weight="medium">
                    Available - {maxToken} {symbol}
                </Text>
            </div>
            <Card
                variant="gray"
                titleDivider
                header={
                    <div className="flex w-full bg-white p-2.5">
                        <Field
                            data-testid="stake-amount-input"
                            component={NumberInput}
                            allowNegative={false}
                            name="amount"
                            className="text-hero-dark placeholder:text-gray-70 w-full border-none bg-white text-heading4 font-semibold placeholder:font-semibold"
                            decimals
                            suffix={` ${symbol}`}
                            autoFocus
                        />
                        {!HIDE_MAX ? (
                            <button
                                className="border-gray-60 text-steel-darker hover:border-steel-dark hover:text-steel-darker flex h-6 w-11 cursor-pointer items-center justify-center rounded-2xl border border-solid bg-white text-bodySmall font-medium disabled:cursor-auto disabled:opacity-50"
                                onClick={setMaxToken}
                                disabled={queryResult.isPending}
                                type="button"
                            >
                                Max
                            </button>
                        ) : null}
                    </div>
                }
                footer={
                    <div className="flex w-full justify-between py-px">
                        <Text variant="body" weight="medium" color="steel-darker">
                            Gas Fees
                        </Text>
                        <Text variant="body" weight="medium" color="steel-darker">
                            {gasBudget} {symbol}
                        </Text>
                    </div>
                }
            >
                <div className="flex w-full justify-between pb-3.75">
                    <Text variant="body" weight="medium" color="steel-darker">
                        Staking Rewards Start
                    </Text>
                    {timeBeforeStakeRewardsStarts > 0 ? (
                        <CountDownTimer
                            timestamp={timeBeforeStakeRewardsStarts}
                            variant="body"
                            color="steel-darker"
                            weight="semibold"
                            label="in"
                            endLabel="--"
                        />
                    ) : (
                        <Text variant="body" weight="medium" color="steel-darker">
                            {epoch ? `Epoch #${Number(startEarningRewardsEpoch)}` : '--'}
                        </Text>
                    )}
                </div>
                <div className="item-center flex w-full justify-between pb-3.75">
                    <div className="flex-1">
                        <Text variant="pBody" weight="medium" color="steel-darker">
                            Staking Rewards Redeemable
                        </Text>
                    </div>
                    <div className="flex flex-1 items-center justify-end gap-1">
                        {timeBeforeStakeRewardsRedeemable > 0 ? (
                            <CountDownTimer
                                timestamp={timeBeforeStakeRewardsRedeemable}
                                variant="body"
                                color="steel-darker"
                                weight="semibold"
                                label="in"
                                endLabel="--"
                            />
                        ) : (
                            <Text variant="body" weight="medium" color="steel-darker">
                                {epoch ? `Epoch #${Number(redeemableRewardsEpoch)}` : '--'}
                            </Text>
                        )}
                    </div>
                </div>
            </Card>
        </Form>
    );
}

export default memo(StakeForm);
