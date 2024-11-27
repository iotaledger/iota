// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KeyValueInfo, Panel, TooltipPosition } from '@iota/apps-ui-kit';
import { formatPercentageDisplay, useGetStakingValidatorDetails } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';

interface ValidatorStakingDataProps {
    validatorAddress: string;
    stakeId: string;
    isUnstake: boolean;
}

export function ValidatorStakingData({
    validatorAddress,
    stakeId,
    isUnstake,
}: ValidatorStakingDataProps) {
    const account = useCurrentAccount();

    const {
        validatorApy: { apy, isApyApproxZero },
        totalStakePercentage,
    } = useGetStakingValidatorDetails({
        accountAddress: account?.address || '',
        validatorAddress,
        stakeId,
        unstake: isUnstake,
    });

    return (
        <Panel hasBorder>
            <div className="flex flex-col gap-y-sm p-md">
                <KeyValueInfo
                    keyText="Staking APY"
                    tooltipPosition={TooltipPosition.Right}
                    tooltipText="Annualized percentage yield based on past validator performance. Future APY may vary"
                    value={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                    fullwidth
                />
                <KeyValueInfo
                    keyText="Stake Share"
                    tooltipPosition={TooltipPosition.Right}
                    tooltipText="Stake percentage managed by this validator."
                    value={formatPercentageDisplay(totalStakePercentage)}
                    fullwidth
                />
            </div>
        </Panel>
    );
}
