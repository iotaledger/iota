// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Divider, KeyValueInfo, Panel, TooltipPosition } from '@iota/apps-ui-kit';
import { type GasSummaryType, useStakeTxnInfo, GasSummary } from '../../../';
import { RenderExplorerLink } from '../../../types';

interface StakeTransactionInfoProps {
    activeAddress: string | null;
    renderExplorerLink: RenderExplorerLink;
    apy?: string;
    startEpoch?: string | number;
    gasSummary?: GasSummaryType;
}

export function StakeTransactionInfo({
    apy,
    startEpoch,
    gasSummary,
    activeAddress,
    renderExplorerLink,
}: StakeTransactionInfoProps) {
    const { stakedRewardsStartEpoch, timeBeforeStakeRewardsRedeemableAgoDisplay } =
        useStakeTxnInfo(startEpoch);
    return (
        <Panel hasBorder>
            <div className="flex flex-col gap-y-sm p-md">
                {apy && (
                    <KeyValueInfo
                        keyText="APY"
                        value={apy}
                        tooltipText="This is the Annualized Percentage Yield of the a specific validator’s past operations. Note there is no guarantee this APY will be true in the future."
                        tooltipPosition={TooltipPosition.Right}
                        fullwidth
                    />
                )}
                <KeyValueInfo
                    keyText="Staking Rewards Start"
                    value={stakedRewardsStartEpoch}
                    fullwidth
                />
                <KeyValueInfo
                    keyText="Redeem Rewards"
                    value={timeBeforeStakeRewardsRedeemableAgoDisplay}
                    fullwidth
                />
                <Divider />
                <GasSummary
                    activeAddress={activeAddress}
                    gasSummary={gasSummary}
                    renderExplorerLink={renderExplorerLink}
                />
            </div>
        </Panel>
    );
}
