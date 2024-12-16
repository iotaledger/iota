// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExternalImage } from '@/components';
import { useGetCurrentEpochEndTimestamp } from '@/hooks/useGetCurrentEpochEndTimestamp';
import { MIGRATION_OBJECT_WITHOUT_UC_KEY } from '@/lib/constants';
import { CommonMigrationObjectType } from '@/lib/enums';
import { ResolvedObjectTypes } from '@/lib/types';
import { Card, CardBody, CardImage, ImageShape, LabelText, LabelTextSize } from '@iota/apps-ui-kit';
import { MILLISECONDS_PER_SECOND, TimeUnit, useFormatCoin, useTimeAgo } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Assets, DataStack, IotaLogoMark } from '@iota/ui-icons';
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
    const [balance, token] = useFormatCoin(migrationObject.balance, coinType);

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
                    subtitle="Visual Assets"
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
            <CardBody title={title} subtitle={subtitle} />
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
    const { data: currentEpochEndTimestampMs, isLoading } = useGetCurrentEpochEndTimestamp();

    const epochEndMs = currentEpochEndTimestampMs ?? 0;
    const currentDateMs = Date.now();

    const unlockConditionTimestampMs = parseInt(groupKey) * MILLISECONDS_PER_SECOND;
    // TODO: https://github.com/iotaledger/iota/issues/4369
    const isInAFutureEpoch = !isLoading && unlockConditionTimestampMs > epochEndMs;

    const outputTimestampMs = isInAFutureEpoch ? unlockConditionTimestampMs : epochEndMs;

    const timeLabel = useTimeAgo({
        timeFrom: outputTimestampMs,
        shortedTimeLabel: true,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_DAY,
    });

    const showLabel = outputTimestampMs > currentDateMs;

    return (
        <div className="ml-auto h-full whitespace-nowrap">
            {showLabel && (
                <LabelText
                    size={LabelTextSize.Small}
                    text={timeLabel}
                    label={isTimelocked ? 'Unlocks in' : 'Expires in'}
                />
            )}
        </div>
    );
}
