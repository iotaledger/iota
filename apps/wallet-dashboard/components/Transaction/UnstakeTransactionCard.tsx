// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import type { IotaEvent } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Box } from '..';
import { TransactionAmount } from './';

interface UnstakeTransactionCardProps {
    event: IotaEvent;
}

export default function UnstakeTransactionCard({ event }: UnstakeTransactionCardProps) {
    const eventJson = event.parsedJson as {
        principal_amount?: number;
        reward_amount?: number;
        validator_address?: string;
    };
    const principalAmount = eventJson?.principal_amount || 0;
    const rewardAmount = eventJson?.reward_amount || 0;
    const totalAmount = Number(principalAmount) + Number(rewardAmount);
    const [formatPrinciple, symbol] = useFormatCoin(principalAmount, IOTA_TYPE_ARG);
    const [formattedRewards] = useFormatCoin(rewardAmount || 0, IOTA_TYPE_ARG);

    return (
        <Box>
            <div className="divide-gray-40 flex flex-col divide-x-0 divide-y divide-solid">
                {totalAmount && (
                    <TransactionAmount
                        amount={totalAmount}
                        coinType={IOTA_TYPE_ARG}
                        label="Total"
                    />
                )}

                <div className="flex w-full justify-between py-3.5">
                    <div className="text-steel flex items-baseline gap-1">Your SUI Stake</div>

                    <div className="text-steel flex items-baseline gap-1">
                        {formatPrinciple} {symbol}
                    </div>
                </div>

                <div className="flex w-full justify-between py-3.5">
                    <div className="text-steel flex items-baseline gap-1">
                        Staking Rewards Earned
                    </div>

                    <div className="text-steel flex items-baseline gap-1">
                        {formattedRewards} {symbol}
                    </div>
                </div>
            </div>
        </Box>
    );
}
