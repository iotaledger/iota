// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BadgeType } from './badge.enums';

export const BACKGROUND_COLORS = {
    [BadgeType.Outlined]: 'bg-transparent',
    [BadgeType.NeutralFill]: 'bg-neutral-92 dark:bg-neutral-12',
    [BadgeType.PrimaryFill]: 'bg-primary-90 dark:bg-primary-10',
};

export const TEXT_COLORS: Record<BadgeType, string> = {
    [BadgeType.Outlined]: 'text-neutral-10 dark:text-neutral-92',
    [BadgeType.NeutralFill]: 'text-neutral-10 dark:text-neutral-92',
    [BadgeType.PrimaryFill]: 'text-primary-20 dark:text-primary-90',
};

export const OUTLINED_BORDER = 'border border-neutral-70 dark:border-neutral-40';
export const BADGE_TEXT_CLASS = 'text-body-md';

export const DISABLED_OPACITY = 'disabled:opacity-30';
