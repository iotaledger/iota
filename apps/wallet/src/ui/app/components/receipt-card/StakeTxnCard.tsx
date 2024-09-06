// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ValidatorLogo } from '_app/staking/validators/ValidatorLogo';
import { TxnAmount } from '_components';

import { formatPercentageDisplay, type GasSummaryType, useGetValidatorsApy } from '@iota/core';
import type { IotaEvent } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { CardType } from '@iota/apps-ui-kit';
import { StakeTxnInfo } from './StakeTxnInfo';

interface StakeTxnCardProps {
    event: IotaEvent;
    gasSummary?: GasSummaryType;
}

// For Staked Transaction use moveEvent Field to get the validator address, delegation amount, epoch
export function StakeTxnCard({ event, gasSummary }: StakeTxnCardProps) {
    const json = event.parsedJson as {
        amount: string;
        validator_address: string;
        epoch: string;
    };
    const validatorAddress = json?.validator_address;
    const stakedAmount = json?.amount;
    const stakedEpoch = Number(json?.epoch || '0');

    const { data: rollingAverageApys } = useGetValidatorsApy();

    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
    };

    return (
        <div className="flex flex-col gap-y-md">
            {validatorAddress && (
                <ValidatorLogo
                    validatorAddress={validatorAddress}
                    type={CardType.Filled}
                    showActiveStatus
                    activeEpoch={json?.epoch}
                />
            )}
            {stakedAmount && (
                <TxnAmount amount={stakedAmount} coinType={IOTA_TYPE_ARG} subtitle="Stake" />
            )}
            <StakeTxnInfo
                startEpoch={stakedEpoch}
                apy={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                gasSummary={gasSummary}
            />
        </div>
    );
}
