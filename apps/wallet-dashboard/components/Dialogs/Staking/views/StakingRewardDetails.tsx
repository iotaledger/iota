// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Divider, KeyValueInfo, Panel } from '@iota/apps-ui-kit';
import { formatApy, useFormatCoin, useStakeTxnInfo, ValidatorApyData } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface StakingRewardDetailsProps {
    gasBudget: string | number | null | undefined;
    validatorApy: ValidatorApyData;
}

export function StakingRewardDetails({
    gasBudget,
    validatorApy: { apy, isApyApproxZero },
}: StakingRewardDetailsProps): React.JSX.Element {
    const [gas, gasSymbol] = useFormatCoin(gasBudget, IOTA_TYPE_ARG);
    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');

    const { stakedRewardsStartEpoch, timeBeforeStakeRewardsRedeemableAgoDisplay } = useStakeTxnInfo(
        system?.epoch,
    );

    return (
        <Panel hasBorder>
            <div className="flex flex-col gap-y-sm p-md">
                {apy !== null && apy !== undefined ? (
                    <KeyValueInfo keyText="APY" value={formatApy(apy, isApyApproxZero)} fullwidth />
                ) : null}

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
                <KeyValueInfo
                    keyText="Gas fee"
                    value={gas || '--'}
                    supportingLabel={gasSymbol}
                    fullwidth
                />
            </div>
        </Panel>
    );
}
