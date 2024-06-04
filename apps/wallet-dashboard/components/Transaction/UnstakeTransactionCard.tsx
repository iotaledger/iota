// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@mysten/core';
import type { SuiEvent } from '@mysten/sui.js/client';
import { SUI_TYPE_ARG } from '@mysten/sui.js/utils';
import { Box } from '..';
import { TransactionAmount } from './';

interface UnstakeTransactionCardProps {
    event: SuiEvent;
}

export default function UnstakeTransactionCard({ event }: UnstakeTransactionCardProps) {
    const json = event.parsedJson as {
        principal_amount?: number;
        reward_amount?: number;
        validator_address?: string;
    };
    const principalAmount = json?.principal_amount || 0;
    const rewardAmount = json?.reward_amount || 0;
    const totalAmount = Number(principalAmount) + Number(rewardAmount);
    const [formatPrinciple, symbol] = useFormatCoin(principalAmount, SUI_TYPE_ARG);
    const [formattedRewards] = useFormatCoin(rewardAmount || 0, SUI_TYPE_ARG);

    return (
        <Box>
            <div className="divide-gray-40 flex flex-col divide-x-0 divide-y divide-solid">
                {totalAmount && (
                    <TransactionAmount amount={totalAmount} coinType={SUI_TYPE_ARG} label="Total" />
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
