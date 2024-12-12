// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ExternalImage from '@/components/ExternalImage';
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY } from '@/lib/constants';
import { CommonMigrationObjectType } from '@/lib/enums';
import { ResolvedObjectTypes } from '@/lib/types';
import { Card, CardBody, CardImage, ImageShape, LabelText, LabelTextSize } from '@iota/apps-ui-kit';
import { TimeUnit, useFormatCoin, useTimeAgo } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Assets, DataStack, IotaLogoMark } from '@iota/ui-icons';
import { useState } from 'react';

export function ResolvedObjectCard({
    migrationObject,
    isTimelockedObjects,
}: {
    migrationObject: ResolvedObjectTypes;
    isTimelockedObjects: boolean;
}) {
    const coinType = 'coinType' in migrationObject ? migrationObject.coinType : IOTA_TYPE_ARG;
    const [balance, token] = useFormatCoin(migrationObject.balance, coinType);

    switch (migrationObject.commonObjectType) {
        case CommonMigrationObjectType.Basic:
            return (
                <MigrationObjectCard
                    title={`${balance} ${token}`}
                    subtitle="IOTA Tokens"
                    expirationKey={migrationObject.expirationKey}
                    image={<IotaLogoMark />}
                    isTimelockedObjects={isTimelockedObjects}
                />
            );
        case CommonMigrationObjectType.Nft:
            return (
                <MigrationObjectCard
                    title={migrationObject.name}
                    subtitle="Visual Assets"
                    expirationKey={migrationObject.expirationKey}
                    image={
                        <ExternalImageWithFallback
                            src={migrationObject.image_url}
                            alt={migrationObject.name}
                            fallback={<Assets />}
                        />
                    }
                    isTimelockedObjects={isTimelockedObjects}
                />
            );
        case CommonMigrationObjectType.NativeToken:
            return (
                <MigrationObjectCard
                    isTimelockedObjects={isTimelockedObjects}
                    title={`${balance} ${token}`}
                    subtitle="Native Tokens"
                    expirationKey={migrationObject.expirationKey}
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
    expirationKey: string;
    image?: React.ReactNode;
    isTimelockedObjects: boolean;
}

function MigrationObjectCard({
    title,
    subtitle,
    image,
    expirationKey,
    isTimelockedObjects,
}: MigrationObjectCardProps) {
    const hasExpiration = expirationKey !== MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY;
    return (
        <Card>
            <CardImage shape={ImageShape.SquareRounded}>{image}</CardImage>
            <CardBody title={title} subtitle={subtitle} />
            {hasExpiration && (
                <ExpirationDate
                    groupKey={expirationKey}
                    isTimelockedObjects={isTimelockedObjects}
                />
            )}
        </Card>
    );
}

function ExpirationDate({
    groupKey,
    isTimelockedObjects,
}: {
    groupKey: string;
    isTimelockedObjects: boolean;
}) {
    const { data: epochTimestamp } = useGetCurrentEpochStartTimestamp();
    const timeAgo = useTimeAgo({
        timeFrom: Number(groupKey) * 1000,
        shortedTimeLabel: true,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_DAY,
    });

    const showTimestamp = Number(groupKey) < Number(epochTimestamp);

    return (
        <div className="ml-auto h-full whitespace-nowrap">
            {showTimestamp && (
                <LabelText
                    size={LabelTextSize.Small}
                    text={timeAgo}
                    label={isTimelockedObjects ? 'Unlocks in' : 'Expires in'}
                />
            )}
        </div>
    );
}
