// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BadgeType } from './badge.enums';

export const BACKGROUND_COLORS = {
    [BadgeType.PrimarySolid]: 'badge-bg-color-primary',
    [BadgeType.Neutral]: 'badge-bg-color-neutral',
    [BadgeType.PrimarySoft]: 'badge-bg-color-primary-soft',
    [BadgeType.Success]: 'bg-success-surface',
    [BadgeType.Warning]: 'bg-warning-surface',
    [BadgeType.Error]: 'bg-error-surface',
};

export const TEXT_COLORS: Record<BadgeType, string> = {
    [BadgeType.PrimarySolid]: 'badge-text-color-primary',
    [BadgeType.Neutral]: 'badge-text-color-neutral',
    [BadgeType.PrimarySoft]: 'badge-text-color-primary-soft',
    [BadgeType.Success]: 'text-on-success',
    [BadgeType.Warning]: 'text-on-warning',
    [BadgeType.Error]: 'text-on-error',
};

export const BORDER_COLORS: Record<BadgeType, string> = {
    [BadgeType.PrimarySolid]: 'badge-border-color-primary',
    [BadgeType.Neutral]: 'badge-border-color-neutral',
    [BadgeType.PrimarySoft]: 'badge-border-color-soft',
    [BadgeType.Success]: 'border-success-surface',
    [BadgeType.Warning]: 'border-warning-surface',
    [BadgeType.Error]: 'border-error-surface',
};

export const BADGE_TEXT_CLASS = 'text-label-md';
