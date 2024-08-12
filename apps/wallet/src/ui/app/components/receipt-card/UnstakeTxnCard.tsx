// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ValidatorLogo } from '_app/staking/validators/ValidatorLogo';
import { TxnAmount } from '_components/receipt-card/TxnAmount';
import { Text } from '_src/ui/app/shared/text';
import { useFormatCoin } from '@iota/core';
import type { IotaEvent } from '@iota/iota.js/client';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';

import { Card } from '_app/shared/transaction-summary/Card';

interface UnStakeTxnCardProps {
    event: IotaEvent;
}

export function UnStakeTxnCard({ event }: UnStakeTxnCardProps) {
    const json = event.parsedJson as {
        principal_amount?: number;
        reward_amount?: number;
        validator_address?: string;
    };
    const principalAmount = json?.principal_amount || 0;
    const rewardAmount = json?.reward_amount || 0;
    const validatorAddress = json?.validator_address;
    const totalAmount = Number(principalAmount) + Number(rewardAmount);
    const [formatPrinciple, symbol] = useFormatCoin(principalAmount, IOTA_TYPE_ARG);
    const [formatRewards] = useFormatCoin(rewardAmount || 0, IOTA_TYPE_ARG);

    return (
        <Card>
            <div className="divide-gray-40 flex flex-col divide-x-0 divide-y divide-solid">
                {validatorAddress && (
                    <div className="mb-3.5 w-full">
                        <ValidatorLogo
                            validatorAddress={validatorAddress}
                            showAddress
                            iconSize="md"
                            size="body"
                        />
                    </div>
                )}
                {totalAmount && (
                    <TxnAmount amount={totalAmount} coinType={IOTA_TYPE_ARG} label="Total" />
                )}

                <div className="flex w-full justify-between py-3.5">
                    <div className="text-steel flex items-baseline gap-1">
                        <Text variant="body" weight="medium" color="steel-darker">
                            Your IOTA Stake
                        </Text>
                    </div>

                    <div className="text-steel flex items-baseline gap-1">
                        <Text variant="body" weight="medium" color="steel-darker">
                            {formatPrinciple} {symbol}
                        </Text>
                    </div>
                </div>

                <div className="flex w-full justify-between py-3.5">
                    <div className="text-steel flex items-baseline gap-1">
                        <Text variant="body" weight="medium" color="steel-darker">
                            Staking Rewards Earned
                        </Text>
                    </div>

                    <div className="text-steel flex items-baseline gap-1">
                        <Text variant="body" weight="medium" color="steel-darker">
                            {formatRewards} {symbol}
                        </Text>
                    </div>
                </div>
            </div>
        </Card>
    );
}
