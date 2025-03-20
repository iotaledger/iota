// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExternalImage } from '@/components';
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { useGetCurrentEpochEndTimestamp } from '@/hooks/useGetCurrentEpochEndTimestamp';
import { MIGRATION_OBJECT_WITHOUT_UC_KEY } from '@/lib/constants';
import { CommonMigrationObjectType } from '@/lib/enums';
import { ResolvedObjectTypes } from '@/lib/types';
import {
    Card,
    CardBody,
    CardImage,
    ImageShape,
    LabelText,
    LabelTextSize,
    Tooltip,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import {
    MILLISECONDS_PER_SECOND,
    SECONDS_PER_DAY,
    useCountdownByTimestamp,
    useFormatCoin,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Assets, DataStack, IotaLogoMark, Info } from '@iota/apps-ui-icons';
import { useState } from 'react';

interface MigrationObjectDetailsCardProps {
    migrationObject: ResolvedObjectTypes;
    isTimelocked: boolean;
}
export function MigrationObjectDetailsCard({
    migrationObject: { unlockConditionTimestamp, ...migrationObject },
    isTimelocked: isTimelocked,
}: MigrationObjectDetailsCardProps) {
    const coinType = 'coinType' in migrationObject ? migrationObject.coinType : IOTA_TYPE_ARG;
    const [balance, token] = useFormatCoin({ balance: migrationObject.balance, coinType });

    switch (migrationObject.commonObjectType) {
        case CommonMigrationObjectType.Basic:
            return (
                <MigrationObjectCard
                    title={`${balance} ${token}`}
                    subtitle="IOTA Tokens"
                    unlockConditionTimestamp={unlockConditionTimestamp}
                    image={<IotaLogoMark />}
                    isTimelocked={isTimelocked}
                />
            );
        case CommonMigrationObjectType.Nft:
            return (
                <MigrationObjectCard
                    title={migrationObject.name}
                    subtitle="Visual Asset"
                    unlockConditionTimestamp={unlockConditionTimestamp}
                    image={
                        <ExternalImageWithFallback
                            src={migrationObject.image_url}
                            alt={migrationObject.name}
                            fallback={<Assets />}
                        />
                    }
                    isTimelocked={isTimelocked}
                />
            );
        case CommonMigrationObjectType.NativeToken:
            return (
                <MigrationObjectCard
                    isTimelocked={isTimelocked}
                    title={`${balance} ${token}`}
                    subtitle="Native Tokens"
                    unlockConditionTimestamp={unlockConditionTimestamp}
                    image={<DataStack />}
                />
            );
        default:
            return null;
    }
}

interface ExternalImageWithFallbackProps {
    src: string;
    alt: string;
    fallback: React.ReactNode;
}
function ExternalImageWithFallback({ src, alt, fallback }: ExternalImageWithFallbackProps) {
    const [errored, setErrored] = useState(false);
    function handleError() {
        setErrored(true);
    }
    return !errored ? <ExternalImage src={src} alt={alt} onError={handleError} /> : fallback;
}

interface MigrationObjectCardProps {
    title: string;
    subtitle: string;
    unlockConditionTimestamp: string;
    isTimelocked: boolean;
    image?: React.ReactNode;
}

function MigrationObjectCard({
    title,
    subtitle,
    unlockConditionTimestamp,
    isTimelocked,
    image,
}: MigrationObjectCardProps) {
    const hasUnlockConditionTimestamp =
        unlockConditionTimestamp !== MIGRATION_OBJECT_WITHOUT_UC_KEY;
    return (
        <Card>
            <CardImage shape={ImageShape.SquareRounded}>{image}</CardImage>
            <CardBody title={title} subtitle={subtitle} isTextTruncated />
            {hasUnlockConditionTimestamp && (
                <UnlockConditionLabel
                    groupKey={unlockConditionTimestamp}
                    isTimelocked={isTimelocked}
                />
            )}
        </Card>
    );
}

interface UnlockConditionLabelProps {
    groupKey: string;
    isTimelocked: boolean;
}
function UnlockConditionLabel({ groupKey, isTimelocked: isTimelocked }: UnlockConditionLabelProps) {
    const { data: currentEpochStartTimestampMs, isLoading: isLoadingEpochStart } =
        useGetCurrentEpochStartTimestamp();
    const { data: currentEpochEndTimestampMs, isLoading: isLoadingEpochEnd } =
        useGetCurrentEpochEndTimestamp();

    const epochStartMs = currentEpochStartTimestampMs ?? 0;
    const epochEndMs = currentEpochEndTimestampMs ?? 0;

    const unlockConditionTimestampMs = parseInt(groupKey) * MILLISECONDS_PER_SECOND;
    const isUnlockConditionExpired =
        !isLoadingEpochStart && unlockConditionTimestampMs <= epochStartMs;
    const isInAFutureEpoch = !isLoadingEpochEnd && unlockConditionTimestampMs > epochEndMs;
    // If the unlock condition is within the current epoch, we can show a better estimated time
    // as the current epoch end time + buffer time.
    // Else, we add 24 hours to the expiration time of the special UC because
    // with a confidence interval of 99.99% we know the time will expire by unlock_time + 24h
    const outputTimestampMs = isInAFutureEpoch
        ? unlockConditionTimestampMs + SECONDS_PER_DAY
        : epochEndMs;

    const outputTimestampMsCountdown = useCountdownByTimestamp(Number(outputTimestampMs), {
        showSeconds: false,
    });
    const tooltipText = isInAFutureEpoch
        ? `${new Date(unlockConditionTimestampMs).toLocaleString('en-GB', {
              year: 'numeric',
              month: 'long',
              day: 'numeric',
              hour12: false,
              hour: '2-digit',
              minute: '2-digit',
          })} plus up to 24 hours more`
        : 'At the beginning of the next epoch';

    return (
        !isUnlockConditionExpired && (
            <div className="align-center flex h-full w-1/4 gap-2 whitespace-nowrap">
                <LabelText
                    size={LabelTextSize.Small}
                    text={`~${outputTimestampMsCountdown}`}
                    label={isTimelocked ? 'Unlocks in' : 'Expires in'}
                />
                <Tooltip
                    maxWidth="max-w-none
"
                    position={TooltipPosition.Left}
                    text={tooltipText}
                >
                    <Info />
                </Tooltip>
            </div>
        )
    );
}
