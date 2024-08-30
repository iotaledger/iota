// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE,
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_STARTS,
} from '_src/shared/constants';
import {
    createStakeTransaction,
    parseAmount,
    TimeUnit,
    useCoinMetadata,
    useFormatCoin,
    useGetTimeBeforeEpochNumber,
    useTimeAgo,
} from '@iota/core';
import { Field, Form, useFormikContext } from 'formik';
import { memo, useCallback, useMemo } from 'react';
import { useActiveAddress, useTransactionGasBudget } from '../../hooks';
import { type FormValues } from './StakingCard';
import { ButtonPill, Input, InputType, KeyValueInfo, Panel } from '@iota/apps-ui-kit';

export interface StakeFromProps {
    validatorAddress: string;
    coinBalance: bigint;
    coinType: string;
    epoch?: string | number;
}

function StakeForm({ validatorAddress, coinBalance, coinType, epoch }: StakeFromProps) {
    const { values, setFieldValue, errors } = useFormikContext<FormValues>();

    const { data: metadata } = useCoinMetadata(coinType);
    const decimals = metadata?.decimals ?? 0;
    const [maxToken, symbol, queryResult] = useFormatCoin(coinBalance, coinType);

    const transaction = useMemo(() => {
        if (!values.amount || !decimals) return null;
        if (Number(values.amount) < 0) return null;
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

    const timeBeforeStakeRewardsStartsAgo = useTimeAgo({
        timeFrom: timeBeforeStakeRewardsStarts,
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });
    const stakedRewardsStartEpoch =
        timeBeforeStakeRewardsStarts > 0
            ? `${timeBeforeStakeRewardsStartsAgo === '--' ? '' : 'in'} ${timeBeforeStakeRewardsStartsAgo}`
            : epoch
              ? `Epoch #${Number(startEarningRewardsEpoch)}`
              : '--';

    const { data: timeBeforeStakeRewardsRedeemable } =
        useGetTimeBeforeEpochNumber(redeemableRewardsEpoch);
    const timeBeforeStakeRewardsRedeemableAgo = useTimeAgo({
        timeFrom: timeBeforeStakeRewardsRedeemable,
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });
    const timeBeforeStakeRewardsRedeemableAgoDisplay =
        timeBeforeStakeRewardsRedeemable > 0
            ? `${timeBeforeStakeRewardsRedeemableAgo === '--' ? '' : 'in'} ${timeBeforeStakeRewardsRedeemableAgo}`
            : epoch
              ? `Epoch #${Number(redeemableRewardsEpoch)}`
              : '--';

    return (
        <Form
            className="flex w-full flex-1 flex-col flex-nowrap items-center gap-md"
            autoComplete="off"
        >
            <Field
                name="amount"
                render={({ field }: { field: FormValues }) => (
                    <Input
                        {...field}
                        type={InputType.Number}
                        name="amount"
                        placeholder="0 IOTA"
                        caption={coinBalance ? `${maxToken} ${symbol} Available` : ''}
                        trailingElement={
                            <ButtonPill onClick={setMaxToken} disabled={queryResult.isPending}>
                                Max
                            </ButtonPill>
                        }
                        errorMessage={errors.amount}
                        label="Amount"
                    />
                )}
            />
            <Panel hasBorder>
                <div className="flex flex-col gap-y-sm p-md">
                    <KeyValueInfo
                        keyText="Staking Rewards Start"
                        valueText={stakedRewardsStartEpoch}
                    />
                    <KeyValueInfo
                        keyText="Redeem Rewards"
                        valueText={timeBeforeStakeRewardsRedeemableAgoDisplay}
                    />
                    <KeyValueInfo
                        keyText="Gas fee"
                        valueText={gasBudget}
                        supportingLabel={symbol}
                    />
                </div>
            </Panel>
        </Form>
    );
}

export default memo(StakeForm);
