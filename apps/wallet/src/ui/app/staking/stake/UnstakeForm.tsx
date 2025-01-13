// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createUnstakeTransaction,
    TimeUnit,
    useFormatCoin,
    useGetTimeBeforeEpochNumber,
    useTimeAgo,
    GAS_SYMBOL,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Form } from 'formik';
import { useMemo } from 'react';

import { useActiveAddress, useTransactionGasBudget } from '_hooks';
import { Divider, KeyValueInfo, Panel } from '@iota/apps-ui-kit';

export interface StakeFromProps {
    stakedIotaId: string;
    coinBalance: bigint;
    coinType: string;
    stakingReward?: string;
    epoch: number;
}

export function UnStakeForm({
    stakedIotaId,
    coinBalance,
    coinType,
    stakingReward,
    epoch,
}: StakeFromProps) {
    const [rewards, rewardSymbol] = useFormatCoin(stakingReward, IOTA_TYPE_ARG);
    const [totalIota] = useFormatCoin(BigInt(stakingReward || 0) + coinBalance, IOTA_TYPE_ARG);
    const [tokenBalance] = useFormatCoin(coinBalance, coinType);

    const transaction = useMemo(() => createUnstakeTransaction(stakedIotaId), [stakedIotaId]);
    const activeAddress = useActiveAddress();
    const { data: gasBudget } = useTransactionGasBudget(activeAddress, transaction);

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

    return (
        <Form className="flex flex-1 flex-col flex-nowrap gap-y-md" autoComplete="off" noValidate>
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
                        value={gasBudget || '-'}
                        supportingLabel={GAS_SYMBOL}
                        fullwidth
                    />
                </div>
            </Panel>
        </Form>
    );
}
