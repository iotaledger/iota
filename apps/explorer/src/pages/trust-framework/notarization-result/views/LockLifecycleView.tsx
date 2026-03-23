// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LockLocked, LockUnlocked, Info } from '@iota/apps-ui-icons';
import {
    BadgeType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    KeyValueInfo,
    Title,
    TitleSize,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import { formatDate } from '@iota/core';
import { type LockMetadata, type TimeLock } from '@iota/notarization/web';
import { CollapsibleCard, ErrorBoundary, IconBadge } from '~/components';
import { useEffect, useState } from 'react';

interface LockLifecycleViewProps {
    locking: LockMetadata | undefined;
}

export function LockLifecycleView({ locking }: LockLifecycleViewProps): JSX.Element {
    if (locking == null) {
        return (
            <ErrorBoundary>
                <div className="flex w-full flex-col gap-sm">
                    <Title
                        title="Lock Lifecycle"
                        tooltipPosition={TooltipPosition.Left}
                        tooltipText="View the lock lifecycle governing transfer, update, and delete operations on this notarization."
                    />
                    <div className="flex flex-col">
                        <InfoBox
                            supportingText="No lock configuration found for this notarization."
                            icon={<Info />}
                            type={InfoBoxType.Default}
                            style={InfoBoxStyle.Elevated}
                        />
                    </div>
                </div>
            </ErrorBoundary>
        );
    }

    return (
        <ErrorBoundary>
            <div className="flex w-full flex-col gap-sm">
                <Title
                    title="Lock Lifecycle"
                    tooltipPosition={TooltipPosition.Left}
                    tooltipText="View the lock lifecycle governing transfer, update, and delete operations on this notarization."
                />
                <div className="flex flex-col gap-sm">
                    <LockCard operation={LockOperation.Transfer} timeLock={locking.transferLock} />
                    <LockCard operation={LockOperation.Update} timeLock={locking.updateLock} />
                    <LockCard operation={LockOperation.Delete} timeLock={locking.deleteLock} />
                </div>
            </div>
        </ErrorBoundary>
    );
}

enum LockOperation {
    Transfer = 'Transfer',
    Update = 'Update',
    Delete = 'Delete',
}

interface LockCardProps {
    operation: LockOperation;
    timeLock: TimeLock;
}

function LockCard({ operation, timeLock }: LockCardProps): JSX.Element {
    const { badgeType, badgeLabel, icon } = getLockBadgeStyle(timeLock);

    return (
        <CollapsibleCard
            collapsible
            title={`${operation} Lock`}
            titleSize={TitleSize.Small}
            supportingTitleElement={
                <div className="ml-1 flex">
                    <IconBadge label={badgeLabel} type={badgeType} icon={icon} />
                </div>
            }
        >
            <div className="flex flex-col gap-4 py-sm--rs">
                <LockType operation={operation} timeLock={timeLock} />
                <LockExpiration timeLock={timeLock} />
            </div>
        </CollapsibleCard>
    );
}

interface LockTypeProps {
    operation: LockOperation;
    timeLock: TimeLock;
}

function LockType({ operation, timeLock }: LockTypeProps): JSX.Element {
    return (
        <div className="flex flex-wrap px-md--rs">
            <KeyValueInfo
                keyText="Lock Type"
                value={timeLock.type}
                fullwidth
                tooltipPosition={TooltipPosition.Left}
                tooltipText={getLockTypeTooltip(operation, timeLock)}
            />
        </div>
    );
}

interface LockExpirationProps {
    timeLock: TimeLock;
}

function LockExpiration({ timeLock }: LockExpirationProps): JSX.Element | null {
    if (timeLock.type !== 'UnlockAt') {
        return null;
    }

    const unlockTimestampSec = Number(timeLock.args);
    const unlockDate = new Date(unlockTimestampSec * 1000);
    const formattedDate = formatDate(unlockDate, ['year', 'month', 'day', 'hour', 'minute']);
    const countdown = useCountdown(unlockDate);

    return (
        <div className="flex flex-wrap px-md--rs">
            <KeyValueInfo
                keyText="Unlocks at"
                value={`${formattedDate} (${countdown})`}
                fullwidth
                tooltipPosition={TooltipPosition.Left}
                tooltipText={`This lock expires on ${formattedDate}. After expiration the operation becomes available.`}
            />
        </div>
    );
}

function useCountdown(targetDate: Date): string {
    const [now, setNow] = useState(() => Date.now());

    useEffect(() => {
        const interval = setInterval(() => setNow(Date.now()), 60_000);
        return () => clearInterval(interval);
    }, []);

    const diffMs = targetDate.getTime() - now;
    if (diffMs <= 0) {
        return 'Expired';
    }

    const totalMinutes = Math.floor(diffMs / 60_000);
    const minutes = totalMinutes % 60;
    const totalHours = Math.floor(totalMinutes / 60);
    const hours = totalHours % 24;
    const days = Math.floor(totalHours / 24);

    const parts: string[] = [];
    if (days > 0) parts.push(`${days}d`);
    if (hours > 0) parts.push(`${hours}h`);
    if (minutes > 0) parts.push(`${minutes}m`);

    return parts.length > 0 ? parts.join(' ') + ' remaining' : 'Less than a minute';
}

function getLockTypeTooltip(operation: LockOperation, timeLock: TimeLock): string {
    const op = operation.toLowerCase();
    switch (timeLock.type) {
        case 'None':
            return `The ${op} operation is not locked and can be performed at any time.`;
        case 'UnlockAt':
            return `The ${op} operation is time-locked and will become available after the expiration date.`;
        case 'UntilDestroyed':
            return `The ${op} operation is permanently locked and cannot be performed.`;
    }
}

enum LockStatus {
    Unlocked = 'Unlocked',
    TimeLocked = 'Time Locked',
    PermanentlyLocked = 'Permanently Locked',
}

function getLockBadgeStyle(timeLock: TimeLock): {
    badgeType: BadgeType;
    badgeLabel: LockStatus;
    icon: JSX.Element;
} {
    switch (timeLock.type) {
        case 'None':
            return {
                badgeType: BadgeType.PrimarySoft,
                badgeLabel: LockStatus.Unlocked,
                icon: <LockUnlocked className="h-4 w-4" />,
            };
        case 'UnlockAt':
            if (isTimeLockExpired(timeLock)) {
                return {
                    badgeType: BadgeType.PrimarySoft,
                    badgeLabel: LockStatus.Unlocked,
                    icon: <LockUnlocked className="h-4 w-4" />,
                };
            }
            return {
                badgeType: BadgeType.Neutral,
                badgeLabel: LockStatus.TimeLocked,
                icon: <LockLocked className="h-4 w-4" />,
            };
        case 'UntilDestroyed':
            return {
                badgeType: BadgeType.Neutral,
                badgeLabel: LockStatus.PermanentlyLocked,
                icon: <LockLocked className="h-4 w-4" />,
            };
    }
}

function isTimeLockExpired(timeLock: TimeLock): boolean {
    if (timeLock.type !== 'UnlockAt') return false;
    const unlockTimestampSec = Number(timeLock.args);
    return Date.now() >= unlockTimestampSec * 1000;
}
