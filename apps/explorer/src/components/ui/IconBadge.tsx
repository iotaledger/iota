// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
import { BadgeType } from '@iota/apps-ui-kit';
import type { ReactNode } from 'react';

interface IconBadgeProps {
    type: BadgeType;
    label: string;
    icon: ReactNode;
}

const BACKGROUND_COLORS: Record<BadgeType, string> = {
    [BadgeType.PrimarySolid]: 'badge-bg-color-primary',
    [BadgeType.Neutral]: 'badge-bg-color-neutral',
    [BadgeType.PrimarySoft]: 'badge-bg-color-primary-soft',
    [BadgeType.Success]: 'bg-success-surface',
    [BadgeType.Warning]: 'bg-warning-surface',
    [BadgeType.Error]: 'bg-error-surface',
};

const TEXT_COLORS: Record<BadgeType, string> = {
    [BadgeType.PrimarySolid]: 'badge-text-color-primary',
    [BadgeType.Neutral]: 'badge-text-color-neutral',
    [BadgeType.PrimarySoft]: 'badge-text-color-primary-soft',
    [BadgeType.Success]: 'text-on-success',
    [BadgeType.Warning]: 'text-on-warning',
    [BadgeType.Error]: 'text-on-error',
};

const BORDER_COLORS: Record<BadgeType, string> = {
    [BadgeType.PrimarySolid]: 'badge-border-color-primary',
    [BadgeType.Neutral]: 'badge-border-color-neutral',
    [BadgeType.PrimarySoft]: 'badge-border-color-soft',
    [BadgeType.Success]: 'border-success-surface',
    [BadgeType.Warning]: 'border-warning-surface',
    [BadgeType.Error]: 'border-error-surface',
};

export function IconBadge({ type, label, icon }: IconBadgeProps): React.JSX.Element {
    return (
        <div
            className={clsx(
                'inline-flex items-center gap-0.5 rounded-full border px-xs py-xxs',
                BACKGROUND_COLORS[type],
                BORDER_COLORS[type],
            )}
        >
            <span className={clsx('flex h-4 w-4 items-center justify-center', TEXT_COLORS[type])}>
                {icon}
            </span>
            <span className={clsx('text-label-md', TEXT_COLORS[type])}>{label}</span>
        </div>
    );
}
