// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BadgeType } from './badge.enums';

export const BACKGROUND_COLORS = {
    [BadgeType.PrimarySolid]: 'bg-primary-30',
    [BadgeType.Neutral]: 'bg-neutral-92 dark:bg-neutral-12',
    [BadgeType.PrimarySoft]: 'bg-primary-90 dark:bg-primary-10',
};

export const TEXT_COLORS: Record<BadgeType, string> = {
    [BadgeType.PrimarySolid]: 'text-primary-100',
    [BadgeType.Neutral]: 'text-neutral-10 dark:text-neutral-92',
    [BadgeType.PrimarySoft]: 'text-primary-20 dark:text-primary-90',
};

export const BORDER_COLORS: Record<BadgeType, string> = {
    [BadgeType.PrimarySolid]: 'border-primary-30',
    [BadgeType.Neutral]: 'border-neutral-92 dark:border-neutral-12',
    [BadgeType.PrimarySoft]: 'border-primary-90 dark:border-primary-10',
};

export const BADGE_TEXT_CLASS = 'text-label-md';
