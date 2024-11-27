// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaEvent } from '@iota/iota-sdk/client';
import { formatPercentageDisplay } from '../../../utils';
import { useGetValidatorsApy } from '../../../hooks';
import { TransactionAmount } from '../amount';
import { StakeTransactionInfo } from '../info';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import type { GasSummaryType, RenderExplorerLink, RenderValidatorLogo } from '../../../types';

interface StakeTransactionDetailsProps {
    event: IotaEvent;
    gasSummary?: GasSummaryType;
    activeAddress: string | null;
    renderExplorerLink: RenderExplorerLink;
    renderValidatorLogo: RenderValidatorLogo;
}

export function StakeTransactionDetails({
    event,
    gasSummary,
    activeAddress,
    renderValidatorLogo: ValidatorLogo,
    renderExplorerLink,
}: StakeTransactionDetailsProps) {
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
                    address={validatorAddress}
                    showActiveStatus
                    activeEpoch={json.epoch}
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
