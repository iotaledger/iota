// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaEvent } from '@iota/iota-sdk/client';
import { formatPercentageDisplay, getStakeDetailsFromEvent } from '../../../utils';
import { useGetValidatorsApy } from '../../../hooks';
import { TransactionAmount } from '../amount';
import { StakeTransactionInfo } from '../info';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import type { GasSummaryType, RenderExplorerLink, RenderValidatorLogo } from '../../../types';

interface StakeTransactionDetailsProps {
    event: IotaEvent;
    activeAddress: string | null;
    renderExplorerLink: RenderExplorerLink;
    renderValidatorLogo: RenderValidatorLogo;
    gasSummary?: GasSummaryType;
}

export function StakeTransactionDetails({
    event,
    gasSummary,
    activeAddress,
    renderValidatorLogo: ValidatorLogo,
    renderExplorerLink,
}: StakeTransactionDetailsProps) {
    const { stakedAmount, validatorAddress, epoch } = getStakeDetailsFromEvent(event);
    const { data: rollingAverageApys } = useGetValidatorsApy();
    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
    };
    const stakedEpoch = Number(epoch || '0');

    return (
        <div className="flex flex-col gap-y-md">
            {validatorAddress && (
                <ValidatorLogo
                    address={validatorAddress}
                    showActiveStatus
                    activeEpoch={epoch.toString()}
                    isSelected
                />
            )}
            {stakedAmount && (
                <TransactionAmount
                    amount={stakedAmount}
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
