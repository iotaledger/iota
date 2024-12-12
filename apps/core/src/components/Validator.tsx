// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React from 'react';
import { ImageIcon, ImageIconSize, formatPercentageDisplay, useValidatorInfo } from '../';
import {
    Card,
    CardBody,
    CardImage,
    CardAction,
    CardActionType,
    CardType,
    Badge,
    BadgeType,
} from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';

interface ValidatorProps {
    isSelected?: boolean;
    validatorAddress: string;
    type?: CardType;
    showApy?: boolean;
    showActiveStatus?: boolean;
    onClick?(): void;
    showAction?: boolean;
    activeEpoch?: string;
}

export function Validator({
    validatorAddress: address,
    type,
    showApy,
    showActiveStatus = false,
    onClick,
    isSelected,
    showAction = true,
    activeEpoch,
}: ValidatorProps) {
    const {
        name: validatorName,
        newValidator,
        isAtRisk,
        apy,
        isApyApproxZero,
        validatorSummary,
        system,
        isPendingValidators,
    } = useValidatorInfo({
        validatorAddress: address,
    });

    if (isPendingValidators) {
        return <div className="flex items-center justify-center">...</div>;
    }
    // for inactive validators, show the epoch number
    const fallBackText = activeEpoch
        ? `Staked ${Number(system?.epoch) - Number(activeEpoch)} epochs ago`
        : '';

    const validatorDisplayName = validatorName || fallBackText;

    const subtitle = showActiveStatus ? (
        <div className="flex items-center gap-1">
            {formatAddress(address)}
            {newValidator && <Badge label="New" type={BadgeType.PrimarySoft} />}
            {isAtRisk && <Badge label="At Risk" type={BadgeType.PrimarySolid} />}
        </div>
    ) : (
        formatAddress(address)
    );
    return (
        <Card type={type || isSelected ? CardType.Filled : CardType.Default} onClick={onClick}>
            <CardImage>
                <ImageIcon
                    src={validatorSummary?.imageUrl ?? null}
                    label={validatorDisplayName}
                    fallback={validatorDisplayName}
                    size={ImageIconSize.Large}
                />
            </CardImage>
            <CardBody title={validatorDisplayName} subtitle={subtitle} isTextTruncated />
            {showApy && (
                <CardAction
                    type={CardActionType.SupportingText}
                    title={formatPercentageDisplay(apy, '-', isApyApproxZero)}
                />
            )}
            {showAction && (
                <CardAction
                    type={CardActionType.SupportingText}
                    title={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                    iconAfterText
                />
            )}
        </Card>
    );
}
