// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { TransactionAmount } from '../amount';
import type { IotaEvent } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import type { GasSummaryType, RenderExplorerLink, RenderValidatorLogo } from '../../../types';
import { useFormatCoin } from '../../../hooks';
import { Divider, KeyValueInfo, Panel } from '@iota/apps-ui-kit';
import { GasSummary } from '../../..';
import { useCurrentAccount } from '@iota/dapp-kit';

interface UnstakeTransactionInfoProps {
    event: IotaEvent;
    gasSummary?: GasSummaryType;
    renderValidatorLogo: RenderValidatorLogo;
    renderExplorerLink: RenderExplorerLink;
}

export function UnstakeTransactionInfo({
    event,
    gasSummary,
    renderValidatorLogo: ValidatorLogo,
    renderExplorerLink,
}: UnstakeTransactionInfoProps) {
    const activeAddress = useCurrentAccount()?.address;
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
            {validatorAddress && <ValidatorLogo address={validatorAddress} isSelected />}
            {totalAmount && (
                <TransactionAmount amount={totalAmount} coinType={IOTA_TYPE_ARG} subtitle="Total" />
            )}
            <Panel hasBorder>
                <div className="flex flex-col gap-y-sm p-md">
                    <KeyValueInfo
                        keyText="Your Stake"
                        value={`${formatPrinciple} ${symbol}`}
                        fullwidth
                    />
                    <KeyValueInfo
                        keyText="Rewards Earned"
                        value={`${formatRewards} ${symbol}`}
                        fullwidth
                    />
                    <Divider />
                    <GasSummary
                        gasSummary={gasSummary}
                        activeAddress={activeAddress}
                        renderExplorerLink={renderExplorerLink}
                    />
                </div>
            </Panel>
        </div>
    );
}
