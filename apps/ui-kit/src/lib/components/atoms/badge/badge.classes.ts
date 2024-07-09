// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BadgeVariant } from './badge.enums';

export const BACKGROUND_COLORS = {
    [BadgeVariant.Outlined]: 'bg-transparent',
    [BadgeVariant.NeutralFill]: 'bg-neutral-92 dark:bg-neutral-12',
    [BadgeVariant.PrimaryFill]: 'bg-primary-90 dark:bg-primary-10',
};

export const TEXT_COLORS: Record<BadgeVariant, string> = {
    [BadgeVariant.Outlined]: 'text-neutral-10 dark:text-neutral-92',
    [BadgeVariant.NeutralFill]: 'text-neutral-10 dark:text-neutral-92',
    [BadgeVariant.PrimaryFill]: 'text-primary-20 dark:text-primary-90',
};

export const OUTLINED_BORDER = 'border border-neutral-70 dark:border-neutral-40';
export const BADGE_TEXT_CLASS = 'text-body-md';
