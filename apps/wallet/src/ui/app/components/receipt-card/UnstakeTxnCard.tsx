// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ValidatorLogo } from '_app/staking/validators/ValidatorLogo';
import { TxnAmount } from '_components';
import { type GasSummaryType, useFormatCoin } from '@iota/core';
import type { IotaEvent } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { CardType, Divider, KeyValueInfo } from '@iota/apps-ui-kit';
import { GasSummary } from '../../shared/transaction-summary/cards/GasSummary';

interface UnStakeTxnCardProps {
    event: IotaEvent;
    gasSummary?: GasSummaryType;
}

export function UnStakeTxnCard({ event, gasSummary }: UnStakeTxnCardProps) {
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
        <div className="flex flex-col gap-y-md">
            {validatorAddress && (
                <div className="mb-3.5 w-full">
                    <ValidatorLogo validatorAddress={validatorAddress} type={CardType.Filled} />
                </div>
            )}
            {totalAmount && (
                <TxnAmount amount={totalAmount} coinType={IOTA_TYPE_ARG} subtitle="Total" />
            )}
            <KeyValueInfo keyText="Your Stake" valueText={`${formatPrinciple} ${symbol}`} />
            <KeyValueInfo keyText="Rewards Earned" valueText={`${formatRewards} ${symbol}`} />
            <Divider />
            <GasSummary gasSummary={gasSummary} />
        </div>
    );
}
