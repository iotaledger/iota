// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CardType } from '@iota/apps-ui-kit';
import { IotaEvent } from '@iota/iota-sdk/client';
import { formatPercentageDisplay } from '../../../utils';
import { useGetValidatorsApy } from '../../../hooks';
import { TransactionAmount } from '../amount';
import { StakeTransactionInfo } from '../info';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { ValidatorLogo } from '../../../';
import type { GasSummaryType, RenderExplorerLink } from '../../../types';

interface StakeTransactionDetailsProps {
    event: IotaEvent;
    activeAddress: string | null;
    renderExplorerLink: RenderExplorerLink;
    gasSummary?: GasSummaryType;
}

export function StakeTransactionDetails({
    event,
    gasSummary,
    activeAddress,
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
                    validatorAddress={validatorAddress}
                    showActiveStatus
                    activeEpoch={json.epoch}
                    type={CardType.Filled}
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
