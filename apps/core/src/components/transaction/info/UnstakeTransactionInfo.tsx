// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionAmount } from '../amount';
import type { IotaEvent } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import type { GasSummaryType, RenderExplorerLink } from '../../../types';
import { useFormatCoin } from '../../../hooks';
import { Divider, KeyValueInfo, Panel, CardType } from '@iota/apps-ui-kit';
import { GasSummary, getUnstakeDetailsFromEvent, Validator } from '../../..';

interface UnstakeTransactionInfoProps {
    activeAddress: string | null;
    event: IotaEvent;
    renderExplorerLink: RenderExplorerLink;
    gasSummary?: GasSummaryType;
}

export function UnstakeTransactionInfo({
    activeAddress,
    event,
    gasSummary,
    renderExplorerLink,
}: UnstakeTransactionInfoProps) {
    const { principalAmount, rewardAmount, totalAmount, validatorAddress } =
        getUnstakeDetailsFromEvent(event);

    const [formatPrinciple, symbol] = useFormatCoin(principalAmount, IOTA_TYPE_ARG);
    const [formatRewards] = useFormatCoin(rewardAmount || 0, IOTA_TYPE_ARG);

    return (
        <div className="flex flex-col gap-y-md">
            {validatorAddress && <Validator address={validatorAddress} type={CardType.Filled} />}
            {totalAmount !== 0n && (
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
