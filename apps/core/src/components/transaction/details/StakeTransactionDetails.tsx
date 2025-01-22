// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CardType } from '@iota/apps-ui-kit';
import { IotaEvent } from '@iota/iota-sdk/client';
import { formatPercentageDisplay, getStakeDetailsFromEvents } from '../../../utils';
import { useGetValidatorsApy } from '../../../hooks';
import { TransactionAmount } from '../amount';
import { StakeTransactionInfo } from '../info';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Validator } from '../../../';
import type { GasSummaryType, RenderExplorerLink } from '../../../types';

interface StakeTransactionDetailsProps {
    events: IotaEvent[];
    activeAddress: string | null;
    renderExplorerLink: RenderExplorerLink;
    gasSummary?: GasSummaryType;
}

export function StakeTransactionDetails({
    events,
    gasSummary,
    activeAddress,
    renderExplorerLink,
}: StakeTransactionDetailsProps) {
    const stakeDetails = getStakeDetailsFromEvents(events);

    const { totalStakedAmount, validatorAddress, epoch } = stakeDetails;
    const { data: rollingAverageApys } = useGetValidatorsApy();
    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
    };
    const stakedEpoch = Number(epoch || '0');

    return (
        <div className="flex flex-col gap-y-md">
            {validatorAddress && (
                <Validator
                    address={validatorAddress}
                    showActiveStatus
                    type={CardType.Filled}
                    activeEpoch={epoch}
                    isSelected
                />
            )}
            {totalStakedAmount && (
                <TransactionAmount
                    amount={totalStakedAmount}
                    coinType={IOTA_TYPE_ARG}
                    subtitle="Stake"
                />
            )}

            <StakeTransactionInfo
                activeAddress={activeAddress}
                startEpoch={stakedEpoch}
                apy={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                gasSummary={gasSummary}
                renderExplorerLink={renderExplorerLink}
            />
        </div>
    );
}
