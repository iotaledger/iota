// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ImageIcon, ImageIconSize, formatPercentageDisplay, useValidatorInfo } from '@iota/core';
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
    isSelected: boolean;
    address: string;
    showActiveStatus?: boolean;
    onClick?: (address: string) => void;
    showAction?: boolean;
    activeEpoch?: string;
}

export function Validator({
    address,
    showActiveStatus,
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
    } = useValidatorInfo({
        validatorAddress: address,
    });

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

    const handleClick = onClick ? () => onClick(address) : undefined;

    return (
        <Card type={isSelected ? CardType.Filled : CardType.Default} onClick={handleClick}>
            <CardImage>
                <ImageIcon
                    src={validatorSummary?.imageUrl ?? null}
                    label={validatorDisplayName}
                    fallback={validatorDisplayName}
                    size={ImageIconSize.Large}
                />
            </CardImage>
            <CardBody title={validatorDisplayName} subtitle={subtitle} isTextTruncated />
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
