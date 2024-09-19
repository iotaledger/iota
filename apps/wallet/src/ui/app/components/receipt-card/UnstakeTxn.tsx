// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ValidatorLogo } from '_app/staking/validators/ValidatorLogo';
import { TxnAmount } from '_components';
import { type GasSummaryType, useFormatCoin } from '@iota/core';
import type { IotaEvent } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { CardType, Divider, KeyValueInfo, Panel } from '@iota/apps-ui-kit';
import { GasSummary } from '../../shared/transaction-summary/cards/GasSummary';

interface UnStakeTxnProps {
    event: IotaEvent;
    gasSummary?: GasSummaryType;
}

export function UnStakeTxn({ event, gasSummary }: UnStakeTxnProps) {
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
                <ValidatorLogo validatorAddress={validatorAddress} type={CardType.Filled} />
            )}
            {totalAmount && (
                <TxnAmount amount={totalAmount} coinType={IOTA_TYPE_ARG} subtitle="Total" />
            )}
            <Panel hasBorder>
                <div className="flex flex-col gap-y-sm p-md">
                    <KeyValueInfo keyText="Your Stake" value={`${formatPrinciple} ${symbol}`} />
                    <KeyValueInfo keyText="Rewards Earned" value={`${formatRewards} ${symbol}`} />
                    <Divider />
                    <GasSummary gasSummary={gasSummary} />
                </div>
            </Panel>
        </div>
    );
}
