// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    ImageIcon,
    ImageIconSize,
    formatPercentageDisplay,
    useGetInactiveValidator,
    useValidatorInfo,
} from '../';
import {
    Card,
    CardBody,
    CardImage,
    CardAction,
    CardActionType,
    CardType,
    Badge,
    BadgeType,
    ImageShape,
    Skeleton,
} from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';

interface ValidatorProps {
    isSelected?: boolean;
    address: string;
    type?: CardType;
    showActiveStatus?: boolean;
    onClick?: () => void;
    showApy?: boolean;
    activeEpoch?: number;
}

export function Validator({
    address,
    type,
    showActiveStatus = false,
    onClick,
    isSelected,
    showApy = true,
    activeEpoch,
}: ValidatorProps) {
    const {
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
        return (
            <Card>
                <CardImage shape={ImageShape.Rounded}>
                    <Skeleton widthClass="w-10" heightClass="h-10" />
                </CardImage>
                <div className="flex flex-col gap-y-xs">
                    <Skeleton widthClass="w-40" heightClass="h-3.5" />
                    <Skeleton widthClass="w-32" heightClass="h-3" hasSecondaryColors />
                </div>
                <div className="ml-auto flex flex-col gap-y-xs">
                    <Skeleton widthClass="w-20" heightClass="h-3.5" />
                </div>
            </Card>
        );
    }
    // for inactive validators, show the epoch number
    const fallBackText = activeEpoch
        ? `Staked ${Number(system?.epoch) - Number(activeEpoch)} epochs ago`
        : '';

    const { data: inactiveValidatorSummary } = useGetInactiveValidator(address);
    const validatorDisplayName =
        validatorSummary?.name || inactiveValidatorSummary?.name || fallBackText;

    const validatorDisplayLogo =
        validatorSummary?.imageUrl || inactiveValidatorSummary?.imageUrl || null;

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
                    src={validatorDisplayLogo}
                    label={validatorDisplayName}
                    fallback={validatorDisplayName}
                    size={ImageIconSize.Large}
                />
            </CardImage>
            <CardBody title={validatorDisplayName} subtitle={subtitle} isTextTruncated />
            {showApy && (
                <CardAction
                    type={CardActionType.SupportingText}
                    title={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                    iconAfterText
                />
            )}
        </Card>
    );
}
