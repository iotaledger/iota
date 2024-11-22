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
}

export function Validator({
    address,
    showActiveStatus,
    onClick,
    isSelected,
    showAction = true,
}: ValidatorProps) {
    const { name, newValidator, isAtRisk, apy, isApyApproxZero } = useValidatorInfo({
        validatorAddress: address,
    });

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
                <ImageIcon src={null} label={name} fallback={name} size={ImageIconSize.Large} />
            </CardImage>
            <CardBody title={name} subtitle={subtitle} isTextTruncated />
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
