// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Box } from '..';
import { TransactionAmount } from './';
import { formatPercentageDisplay, useGetValidatorsApy } from '@iota/core';
import type { IotaEvent } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface StakeTransactionCardProps {
    event: IotaEvent;
}

export default function StakeTransactionCard({ event }: StakeTransactionCardProps) {
    const json = event.parsedJson as { amount: string; validator_address: string; epoch: string };
    const validatorAddress = json?.validator_address;
    const stakedAmount = json?.amount;

    const { data: rollingAverageApys } = useGetValidatorsApy();

    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
    };

    return (
        <Box>
            <div className="divide-gray-40 flex flex-col divide-x-0 divide-y divide-solid">
                {stakedAmount && (
                    <TransactionAmount
                        amount={stakedAmount}
                        coinType={IOTA_TYPE_ARG}
                        label="Stake"
                    />
                )}
                <div className="flex flex-col">
                    <div className="flex w-full justify-between py-3.5">
                        <div className="text-steel flex items-baseline justify-center gap-1">
                            APY
                        </div>
                        {formatPercentageDisplay(apy, '--', isApyApproxZero)}
                    </div>
                </div>
            </div>
        </Box>
    );
}
