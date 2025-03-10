// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionAmount } from '../amount';
import type { IotaEvent } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import type { GasSummaryType, RenderExplorerLink } from '../../../types';
import { useFormatCoin } from '../../../hooks';
import { Divider, KeyValueInfo, Panel, CardType } from '@iota/apps-ui-kit';
import { GasSummary, getUnstakeDetailsFromEvents, Validator } from '../../..';

interface UnstakeTransactionInfoProps {
    activeAddress: string | null;
    events: IotaEvent[];
    renderExplorerLink: RenderExplorerLink;
    gasSummary?: GasSummaryType;
}

export function UnstakeTransactionInfo({
    activeAddress,
    events,
    gasSummary,
    renderExplorerLink,
}: UnstakeTransactionInfoProps) {
    const unstakeDetails = getUnstakeDetailsFromEvents(events);
    const { totalUnstakeAmount, validatorAddress, unstakeAmount, unstakeRewards } = unstakeDetails;

    const [formatTotalAmountWithoutRewards, symbol] = useFormatCoin({ balance: unstakeAmount });
    const [formatRewards] = useFormatCoin({ balance: unstakeRewards || 0 });
    return (
        <div className="flex flex-col gap-y-md">
            {validatorAddress && <Validator address={validatorAddress} type={CardType.Filled} />}
            {totalUnstakeAmount !== 0n && (
                <TransactionAmount
                    amount={totalUnstakeAmount}
                    coinType={IOTA_TYPE_ARG}
                    subtitle="Total"
                />
            )}
            <Panel hasBorder>
                <div className="flex flex-col gap-y-sm p-md">
                    <KeyValueInfo
                        keyText="Your Stake"
                        value={`${formatTotalAmountWithoutRewards} ${symbol}`}
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
