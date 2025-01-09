// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    Badge,
    BadgeType,
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    type CardType,
} from '@iota/apps-ui-kit';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { formatAddress } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';
import { formatPercentageDisplay, useGetValidatorsApy, ImageIcon, ImageIconSize } from '@iota/core';

interface ValidatorLogoProps {
    validatorAddress: string;
    type?: CardType;
    showApy?: boolean;
    showActiveStatus?: boolean;
    activeEpoch?: string;
    onClick?(): void;
}

export function ValidatorLogo({
    validatorAddress,
    type,
    showApy,
    showActiveStatus = false,
    activeEpoch,
    onClick,
}: ValidatorLogoProps) {
    const { data: system, isPending } = useIotaClientQuery('getLatestIotaSystemState');
    const { data: rollingAverageApys } = useGetValidatorsApy();
    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
    };

    const validatorMeta = useMemo(() => {
        if (!system) return null;

        return (
            system.activeValidators.find(
                (validator) => validator.iotaAddress === validatorAddress,
            ) || null
        );
    }, [validatorAddress, system]);

    const stakingPoolActivationEpoch = Number(validatorMeta?.stakingPoolActivationEpoch || 0);
    const currentEpoch = Number(system?.epoch || 0);

    // flag as new validator if the validator was activated in the last epoch
    // for genesis validators, this will be false
    const newValidator = currentEpoch - stakingPoolActivationEpoch <= 1 && currentEpoch !== 0;

    // flag if the validator is at risk of being removed from the active set
    const isAtRisk = system?.atRiskValidators.some((item) => item[0] === validatorAddress);

    if (isPending) {
        return <div className="flex items-center justify-center">...</div>;
    }
    // for inactive validators, show the epoch number
    const fallBackText = activeEpoch
        ? `Staked ${Number(system?.epoch) - Number(activeEpoch)} epochs ago`
        : '--';
    const validatorName = validatorMeta?.name || fallBackText;

    const subtitle = validatorAddress ? (
        showActiveStatus ? (
            <div className="flex items-center gap-1">
                {formatAddress(validatorAddress)}
                {newValidator && <Badge label="New" type={BadgeType.PrimarySoft} />}
                {isAtRisk && <Badge label="At Risk" type={BadgeType.PrimarySolid} />}
            </div>
        ) : (
            formatAddress(validatorAddress)
        )
    ) : (
        '--'
    );
    return (
        <>
            <Card type={type} onClick={onClick}>
                <CardImage>
                    <ImageIcon
                        src={validatorMeta?.imageUrl ?? null}
                        label={validatorName}
                        fallback={validatorName}
                        size={ImageIconSize.Large}
                    />
                </CardImage>
                <CardBody title={validatorName} subtitle={subtitle} isTextTruncated />
                {showApy && (
                    <CardAction
                        type={CardActionType.SupportingText}
                        title={formatPercentageDisplay(apy, '-', isApyApproxZero)}
                    />
                )}
            </Card>
        </>
    );
}
